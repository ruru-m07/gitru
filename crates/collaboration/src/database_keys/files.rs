use super::*;
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    time::SystemTime,
};
const MAX_METADATA: u64 = 1024;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Journal {
    version: u32,
    database_id: String,
    generation: u64,
    pub ready: bool,
}
impl Journal {
    pub fn reserved() -> Self {
        Self {
            version: 1,
            database_id: uuid::Uuid::new_v4().to_string(),
            generation: 1,
            ready: false,
        }
    }
    pub fn identity(&self) -> DatabaseKeyIdentity {
        DatabaseKeyIdentity {
            database_id: self.database_id.clone(),
            generation: self.generation,
        }
    }
}
pub(super) fn append(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}
pub(super) fn canonical_path(path: &Path) -> Result<PathBuf, DatabaseKeyError> {
    if !path.is_absolute() || path.file_name().is_none() {
        return Err(DatabaseKeyError::InvalidMetadata);
    }
    crate::storage::prepare_private_path(path).map_err(|_| DatabaseKeyError::Storage)?;
    let parent = path
        .parent()
        .ok_or(DatabaseKeyError::InvalidMetadata)?
        .canonicalize()
        .map_err(|_| DatabaseKeyError::Storage)?;
    Ok(parent.join(path.file_name().ok_or(DatabaseKeyError::InvalidMetadata)?))
}

/// A cooperating owner holds the writer lease; these stamps additionally refuse
/// ordinary out-of-band replacement or modification across a held native call.
/// They do not claim protection from a process that can restore file timestamps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Stamp {
    identity: (u64, u64),
    length: u64,
    modified: SystemTime,
}
fn stamp(file: &File) -> Result<Stamp, DatabaseKeyError> {
    let metadata = file.metadata().map_err(|_| DatabaseKeyError::Storage)?;
    if !metadata.is_file() {
        return Err(DatabaseKeyError::InvalidMetadata);
    }
    #[cfg(unix)]
    let identity = {
        use std::os::unix::fs::MetadataExt;
        (metadata.dev(), metadata.ino())
    };
    #[cfg(windows)]
    let identity = {
        use std::os::windows::{fs::MetadataExt, io::AsRawHandle};
        use windows_sys::Win32::Storage::FileSystem::{
            BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_REPARSE_POINT, GetFileInformationByHandle,
        };
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(DatabaseKeyError::InvalidMetadata);
        }
        let mut information: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        // The live File owns this valid handle for the duration of the call.
        if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut information) } == 0 {
            return Err(DatabaseKeyError::Storage);
        }
        (
            u64::from(information.dwVolumeSerialNumber),
            (u64::from(information.nFileIndexHigh) << 32) | u64::from(information.nFileIndexLow),
        )
    };
    Ok(Stamp {
        identity,
        length: metadata.len(),
        modified: metadata.modified().map_err(|_| DatabaseKeyError::Storage)?,
    })
}
fn open_regular(path: &Path) -> Result<Option<File>, DatabaseKeyError> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.is_file() && !meta.file_type().is_symlink() => (),
        Ok(_) => return Err(DatabaseKeyError::InvalidMetadata),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(DatabaseKeyError::Storage),
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // NONBLOCK also avoids waiting on a FIFO swapped in after the precheck.
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options
        .open(path)
        .map_err(|_| DatabaseKeyError::StaleFilesystem)?;
    stamp(&file)?;
    Ok(Some(file))
}
fn observe_file(path: &Path) -> Result<Option<Stamp>, DatabaseKeyError> {
    open_regular(path)?.as_ref().map(stamp).transpose()
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Files {
    pub database: Option<Stamp>,
    sidecars: [Option<Stamp>; 3],
    key_metadata: Option<Stamp>,
}
impl Files {
    pub fn any(&self) -> bool {
        self.database.is_some() || self.sidecars.iter().any(Option::is_some)
    }
}
pub(super) fn observe(path: &Path) -> Result<Files, DatabaseKeyError> {
    Ok(Files {
        database: observe_file(path)?,
        sidecars: [
            observe_file(&append(path, "-wal"))?,
            observe_file(&append(path, "-shm"))?,
            observe_file(&append(path, "-journal"))?,
        ],
        key_metadata: observe_file(&append(path, ".key.json"))?,
    })
}
pub(super) fn unmanaged_error(
    path: &Path,
    files: &Files,
) -> Result<DatabaseKeyError, DatabaseKeyError> {
    if let Some(expected) = &files.database {
        let mut file = open_regular(path)?.ok_or(DatabaseKeyError::StaleFilesystem)?;
        if &stamp(&file)? != expected {
            return Err(DatabaseKeyError::StaleFilesystem);
        }
        let mut header = [0; 16];
        let plaintext = file.read_exact(&mut header).is_ok() && header == *b"SQLite format 3\0";
        if &stamp(&file)? != expected || observe_file(path)?.as_ref() != Some(expected) {
            return Err(DatabaseKeyError::StaleFilesystem);
        }
        if plaintext {
            return Ok(DatabaseKeyError::PlaintextMigrationRequired);
        }
    }
    Ok(DatabaseKeyError::MissingKeyMetadata)
}
pub(super) fn require_no_pending(path: &Path) -> Result<(), DatabaseKeyError> {
    if observe_file(&append(path, ".key.json.pending"))?.is_some() {
        return Err(DatabaseKeyError::InterruptedMetadata);
    }
    Ok(())
}
pub(super) fn read(path: &Path) -> Result<Option<Journal>, DatabaseKeyError> {
    let path = append(path, ".key.json");
    let Some(mut file) = open_regular(&path)? else {
        return Ok(None);
    };
    let before = stamp(&file)?;
    if before.length > MAX_METADATA {
        return Err(DatabaseKeyError::InvalidMetadata);
    }
    let mut bytes = vec![];
    (&mut file)
        .take(MAX_METADATA + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| DatabaseKeyError::Storage)?;
    if bytes.len() as u64 > MAX_METADATA {
        return Err(DatabaseKeyError::InvalidMetadata);
    }
    if stamp(&file)? != before || observe_file(&path)?.as_ref() != Some(&before) {
        return Err(DatabaseKeyError::StaleFilesystem);
    }
    let journal: Journal =
        serde_json::from_slice(&bytes).map_err(|_| DatabaseKeyError::InvalidMetadata)?;
    if journal.version != 1
        || journal.generation != 1
        || uuid::Uuid::parse_str(&journal.database_id)
            .ok()
            .is_none_or(|id| id.get_version_num() != 4 || id.to_string() != journal.database_id)
    {
        return Err(DatabaseKeyError::InvalidMetadata);
    }
    Ok(Some(journal))
}

#[cfg(test)]
thread_local! { pub(super) static FAIL_PUBLICATION: std::cell::Cell<Option<&'static str>> = const { std::cell::Cell::new(None) }; }
fn boundary(_name: &'static str) -> Result<(), DatabaseKeyError> {
    #[cfg(test)]
    if FAIL_PUBLICATION.with(|point| point.get() == Some(_name)) {
        return Err(DatabaseKeyError::Storage);
    }
    Ok(())
}
fn replace_synced(pending: &Path, destination: &Path) -> Result<(), DatabaseKeyError> {
    #[cfg(unix)]
    std::fs::rename(pending, destination).map_err(|_| DatabaseKeyError::Storage)?;
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{
            MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
        };
        let wide = |path: &Path| -> Result<Vec<u16>, DatabaseKeyError> {
            let mut value = path.as_os_str().encode_wide().collect::<Vec<_>>();
            if value.contains(&0) {
                return Err(DatabaseKeyError::InvalidMetadata);
            }
            value.push(0);
            Ok(value)
        };
        let source = wide(pending)?;
        let target = wide(destination)?;
        // Both nul-terminated buffers live through the call. No copy-across-volume
        // fallback; failed replacement retains the old reservation and pending file.
        if unsafe {
            MoveFileExW(
                source.as_ptr(),
                target.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        } == 0
        {
            return Err(DatabaseKeyError::Storage);
        }
    }
    Ok(())
}
pub(super) fn publish(
    path: &Path,
    journal: &Journal,
    replace: bool,
) -> Result<(), DatabaseKeyError> {
    let destination = append(path, ".key.json");
    let pending = append(path, ".key.json.pending");
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut output = options
        .open(&pending)
        .map_err(|_| DatabaseKeyError::Storage)?;
    let bytes = serde_json::to_vec(journal).map_err(|_| DatabaseKeyError::Storage)?;
    output
        .write_all(&bytes)
        .map_err(|_| DatabaseKeyError::Storage)?;
    boundary("after_write")?;
    output.sync_all().map_err(|_| DatabaseKeyError::Storage)?;
    boundary("after_file_sync")?;
    drop(output);
    boundary("before_publish")?;
    if replace {
        replace_synced(&pending, &destination)?;
    } else {
        std::fs::hard_link(&pending, &destination).map_err(|_| DatabaseKeyError::Storage)?;
        boundary("after_link")?;
        std::fs::remove_file(&pending).map_err(|_| DatabaseKeyError::Storage)?;
    }
    boundary("before_directory_sync")?;
    #[cfg(unix)]
    {
        File::open(path.parent().ok_or(DatabaseKeyError::Storage)?)
            .and_then(|file| file.sync_all())
            .map_err(|_| DatabaseKeyError::Storage)?;
    }
    Ok(())
}
