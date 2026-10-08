//! Explicit, crash-recoverable key rotation and destructive reset boundaries.
//!
//! Neither operation deletes a vault entry. Rotation publishes a separately
//! authenticated next-generation database; reset quarantines all database bytes
//! before a later, independently authorized first-run may create new storage.

use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{DatabaseKey, DatabaseKeyError, DatabaseKeyIdentity, DatabaseKeyVault, files};

const MARKER_SUFFIX: &str = ".key-lifecycle-pending.json";
const KEY_SUFFIX: &str = ".key.json";
const SIDECARS: [&str; 3] = ["-wal", "-shm", "-journal"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Operation {
    Rotate,
    Reset,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Marker {
    version: u32,
    operation: Operation,
    confirmation_id: String,
    quarantine_name: String,
    candidate_name: Option<String>,
    source_sha256: String,
    source_metadata_sha256: String,
    candidate_sha256: Option<String>,
    candidate_metadata_sha256: Option<String>,
}

pub struct DatabaseKeyRotationReservation {
    _lease: crate::storage::WriterLease,
    target: PathBuf,
    source: files::Journal,
    next: files::Journal,
    next_key: DatabaseKey,
}

impl DatabaseKeyRotationReservation {
    /// Reserves a distinct next-generation key. The current key must still be
    /// readable, and neither current nor next-generation entries are replaced.
    pub fn prepare(path: &Path, vault: &dyn DatabaseKeyVault) -> Result<Self, DatabaseKeyError> {
        let target = files::canonical_path(path)?;
        require_no_pending(&target)?;
        super::activation::require_no_pending(&target)
            .map_err(|_| DatabaseKeyError::InterruptedRestore)?;
        crate::recovery::require_no_pending_restore(&target)
            .map_err(|_| DatabaseKeyError::InterruptedRestore)?;
        files::require_no_pending(&target)?;
        let lease = crate::storage::acquire_writer_lease(&target).map_err(|error| {
            if error.code == crate::ErrorCode::Busy {
                DatabaseKeyError::Busy
            } else {
                DatabaseKeyError::Storage
            }
        })?;
        require_closed(&target)?;
        let source = files::read(&target)?.ok_or(DatabaseKeyError::MissingKeyMetadata)?;
        if !source.ready {
            return Err(DatabaseKeyError::InterruptedMetadata);
        }
        let source_identity = source.identity();
        vault
            .load(&source_identity)?
            .ok_or(DatabaseKeyError::MissingKey)?;
        let next = source.successor()?;
        let next_identity = next.identity();
        let next_key = match vault.load(&next_identity)? {
            // A crash or cancelled confirmation may leave a reserved next key.
            // Reuse that exact generation; never replace it.
            Some(saved) => saved,
            None => {
                let generated = DatabaseKey::generate()?;
                let stored = vault.store_new(&next_identity, &generated);
                match vault.load(&next_identity) {
                    Ok(Some(saved)) if generated.same(&saved) => saved,
                    Ok(Some(_)) => return Err(DatabaseKeyError::VaultKeyMismatch),
                    Ok(None) => {
                        return Err(stored
                            .err()
                            .unwrap_or(DatabaseKeyError::VaultWriteUncertain));
                    }
                    Err(error) => return Err(error),
                }
            }
        };
        Ok(Self {
            _lease: lease,
            target,
            source,
            next,
            next_key,
        })
    }

    pub fn identity(&self) -> DatabaseKeyIdentity {
        self.next.identity()
    }

    pub fn target(&self) -> &Path {
        &self.target
    }

    /// Native candidate creation only; never serialize or log these bytes.
    pub fn key(&self) -> &DatabaseKey {
        &self.next_key
    }

    /// Publish ready metadata only after a trusted native importer has created,
    /// authenticated, integrity-checked, checkpointed and closed this exact
    /// next-generation candidate. Publication alone grants no activation proof.
    pub fn publish_imported_candidate_metadata(
        &self,
        candidate: &Path,
    ) -> Result<(), DatabaseKeyError> {
        let candidate = same_directory_regular(&self.target, candidate)?;
        require_closed(&candidate)?;
        if files::read(&candidate)?.is_some() {
            return Err(DatabaseKeyError::InvalidMetadata);
        }
        files::publish(&candidate, &self.next, false)?;
        if files::read(&candidate)?.as_ref() != Some(&self.next) {
            return Err(DatabaseKeyError::StaleFilesystem);
        }
        Ok(())
    }

    pub fn prepare_verified_candidate(
        self,
        candidate: VerifiedRotatedDatabase,
    ) -> Result<DatabaseKeyRotation, DatabaseKeyError> {
        if candidate.identity != self.next.identity() {
            return Err(DatabaseKeyError::WrongKeyOrCorrupt);
        }
        let candidate = same_directory_regular(&self.target, &candidate.path)?;
        require_closed(&candidate)?;
        let candidate_journal =
            files::read(&candidate)?.ok_or(DatabaseKeyError::MissingKeyMetadata)?;
        if candidate_journal != self.next || !candidate_journal.ready {
            return Err(DatabaseKeyError::WrongKeyOrCorrupt);
        }
        if files::read(&self.target)?.as_ref() != Some(&self.source) {
            return Err(DatabaseKeyError::StaleFilesystem);
        }
        DatabaseKeyRotation::new(self._lease, self.target, candidate)
    }
}

pub struct VerifiedRotatedDatabase {
    path: PathBuf,
    identity: DatabaseKeyIdentity,
}

impl VerifiedRotatedDatabase {
    /// # Safety
    /// The caller must authenticate the candidate with `identity`, verify its
    /// schema/integrity/authored data, checkpoint it, and close every handle.
    pub unsafe fn from_native_verification(path: PathBuf, identity: DatabaseKeyIdentity) -> Self {
        Self { path, identity }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseKeyLifecyclePreview {
    confirmation_id: String,
    quarantine: PathBuf,
}
impl DatabaseKeyLifecyclePreview {
    pub fn confirmation_id(&self) -> &str {
        &self.confirmation_id
    }
    pub fn quarantine(&self) -> &Path {
        &self.quarantine
    }
}

pub struct DatabaseKeyRotation {
    _lease: crate::storage::WriterLease,
    target: PathBuf,
    candidate: PathBuf,
    marker: Marker,
    preview: DatabaseKeyLifecyclePreview,
}
impl DatabaseKeyRotation {
    fn new(
        lease: crate::storage::WriterLease,
        target: PathBuf,
        candidate: PathBuf,
    ) -> Result<Self, DatabaseKeyError> {
        let id = Uuid::new_v4().to_string();
        let quarantine_name = format!(".database-key-rotation-{id}.recovery");
        let marker = Marker {
            version: 1,
            operation: Operation::Rotate,
            confirmation_id: id.clone(),
            quarantine_name: quarantine_name.clone(),
            candidate_name: Some(file_name(&candidate)?),
            source_sha256: hash(&target)?,
            source_metadata_sha256: hash(&files::append(&target, KEY_SUFFIX))?,
            candidate_sha256: Some(hash(&candidate)?),
            candidate_metadata_sha256: Some(hash(&files::append(&candidate, KEY_SUFFIX))?),
        };
        Ok(Self {
            _lease: lease,
            target: target.clone(),
            candidate,
            preview: DatabaseKeyLifecyclePreview {
                confirmation_id: id,
                quarantine: target.parent().unwrap().join(quarantine_name),
            },
            marker,
        })
    }
    pub fn preview(&self) -> &DatabaseKeyLifecyclePreview {
        &self.preview
    }
    pub fn confirm(self, confirmation_id: &str) -> Result<PathBuf, DatabaseKeyError> {
        if confirmation_id != self.marker.confirmation_id {
            return Err(DatabaseKeyError::ConfirmationRequired);
        }
        verify_rotation_inputs(&self.target, &self.candidate, &self.marker)?;
        publish_marker(&self.target, &self.marker)?;
        std::fs::create_dir(&self.preview.quarantine).map_err(|_| DatabaseKeyError::Storage)?;
        sync_parent(&self.preview.quarantine)?;
        move_file(
            &self.target,
            &self.preview.quarantine.join("database.sqlite"),
        )?;
        move_file(
            &files::append(&self.target, KEY_SUFFIX),
            &self.preview.quarantine.join("database.key.json"),
        )?;
        move_file(&self.candidate, &self.target)?;
        move_file(
            &files::append(&self.candidate, KEY_SUFFIX),
            &files::append(&self.target, KEY_SUFFIX),
        )?;
        verify_hash(
            &self.target,
            self.marker.candidate_sha256.as_deref().unwrap(),
        )?;
        verify_hash(
            &files::append(&self.target, KEY_SUFFIX),
            self.marker.candidate_metadata_sha256.as_deref().unwrap(),
        )?;
        finish_marker(&self.target)?;
        Ok(self.preview.quarantine)
    }
}

pub struct DatabaseReset {
    _lease: crate::storage::WriterLease,
    target: PathBuf,
    marker: Marker,
    preview: DatabaseKeyLifecyclePreview,
}
impl DatabaseReset {
    pub fn prepare(path: &Path) -> Result<Self, DatabaseKeyError> {
        let target = files::canonical_path(path)?;
        require_no_pending(&target)?;
        super::activation::require_no_pending(&target)
            .map_err(|_| DatabaseKeyError::InterruptedRestore)?;
        crate::recovery::require_no_pending_restore(&target)
            .map_err(|_| DatabaseKeyError::InterruptedRestore)?;
        files::require_no_pending(&target)?;
        let lease = crate::storage::acquire_writer_lease(&target).map_err(|error| {
            if error.code == crate::ErrorCode::Busy {
                DatabaseKeyError::Busy
            } else {
                DatabaseKeyError::Storage
            }
        })?;
        require_closed(&target)?;
        let journal = files::read(&target)?.ok_or(DatabaseKeyError::MissingKeyMetadata)?;
        if !journal.ready {
            return Err(DatabaseKeyError::InterruptedMetadata);
        }
        let id = Uuid::new_v4().to_string();
        let quarantine_name = format!(".database-key-reset-{id}.quarantine");
        let marker = Marker {
            version: 1,
            operation: Operation::Reset,
            confirmation_id: id.clone(),
            quarantine_name: quarantine_name.clone(),
            candidate_name: None,
            source_sha256: hash(&target)?,
            source_metadata_sha256: hash(&files::append(&target, KEY_SUFFIX))?,
            candidate_sha256: None,
            candidate_metadata_sha256: None,
        };
        Ok(Self {
            _lease: lease,
            target: target.clone(),
            preview: DatabaseKeyLifecyclePreview {
                confirmation_id: id,
                quarantine: target.parent().unwrap().join(quarantine_name),
            },
            marker,
        })
    }
    pub fn preview(&self) -> &DatabaseKeyLifecyclePreview {
        &self.preview
    }
    pub fn confirm(self, confirmation_id: &str) -> Result<PathBuf, DatabaseKeyError> {
        if confirmation_id != self.marker.confirmation_id {
            return Err(DatabaseKeyError::ConfirmationRequired);
        }
        verify_hash(&self.target, &self.marker.source_sha256)?;
        verify_hash(
            &files::append(&self.target, KEY_SUFFIX),
            &self.marker.source_metadata_sha256,
        )?;
        require_closed(&self.target)?;
        publish_marker(&self.target, &self.marker)?;
        std::fs::create_dir(&self.preview.quarantine).map_err(|_| DatabaseKeyError::Storage)?;
        sync_parent(&self.preview.quarantine)?;
        move_file(
            &self.target,
            &self.preview.quarantine.join("database.sqlite"),
        )?;
        move_file(
            &files::append(&self.target, KEY_SUFFIX),
            &self.preview.quarantine.join("database.key.json"),
        )?;
        finish_marker(&self.target)?;
        Ok(self.preview.quarantine)
    }
}

/// Opens an interrupted transaction without guessing. Rollback restores the
/// exact old database and metadata; candidate/new-generation bytes are retained.
pub struct InterruptedDatabaseKeyLifecycle {
    _lease: crate::storage::WriterLease,
    target: PathBuf,
    marker: Marker,
    quarantine: PathBuf,
}
impl InterruptedDatabaseKeyLifecycle {
    pub fn open(path: &Path) -> Result<Self, DatabaseKeyError> {
        let target = files::canonical_path(path)?;
        let lease = crate::storage::acquire_writer_lease(&target).map_err(|error| {
            if error.code == crate::ErrorCode::Busy {
                DatabaseKeyError::Busy
            } else {
                DatabaseKeyError::Storage
            }
        })?;
        let marker = read_marker(&target)?;
        let quarantine = target.parent().unwrap().join(&marker.quarantine_name);
        Ok(Self {
            _lease: lease,
            target,
            marker,
            quarantine,
        })
    }
    pub fn rollback(self) -> Result<PathBuf, DatabaseKeyError> {
        let old = self.quarantine.join("database.sqlite");
        let old_meta = self.quarantine.join("database.key.json");
        if self.target.exists() {
            let retained = self.quarantine.join("abandoned-new-generation.sqlite");
            move_file(&self.target, &retained)?;
            let active_meta = files::append(&self.target, KEY_SUFFIX);
            if active_meta.exists() {
                move_file(
                    &active_meta,
                    &self.quarantine.join("abandoned-new-generation.key.json"),
                )?;
            }
        }
        verify_hash(&old, &self.marker.source_sha256)?;
        verify_hash(&old_meta, &self.marker.source_metadata_sha256)?;
        move_file(&old, &self.target)?;
        move_file(&old_meta, &files::append(&self.target, KEY_SUFFIX))?;
        finish_marker(&self.target)?;
        Ok(self.quarantine)
    }
}

pub fn require_no_pending(path: &Path) -> Result<(), DatabaseKeyError> {
    if files::append(path, MARKER_SUFFIX).exists()
        || files::append(&files::append(path, MARKER_SUFFIX), ".pending").exists()
    {
        Err(DatabaseKeyError::InterruptedRestore)
    } else {
        Ok(())
    }
}

fn verify_rotation_inputs(
    target: &Path,
    candidate: &Path,
    marker: &Marker,
) -> Result<(), DatabaseKeyError> {
    require_closed(target)?;
    require_closed(candidate)?;
    verify_hash(target, &marker.source_sha256)?;
    verify_hash(
        &files::append(target, KEY_SUFFIX),
        &marker.source_metadata_sha256,
    )?;
    verify_hash(
        candidate,
        marker
            .candidate_sha256
            .as_deref()
            .ok_or(DatabaseKeyError::InvalidMetadata)?,
    )?;
    verify_hash(
        &files::append(candidate, KEY_SUFFIX),
        marker
            .candidate_metadata_sha256
            .as_deref()
            .ok_or(DatabaseKeyError::InvalidMetadata)?,
    )
}
fn require_closed(path: &Path) -> Result<(), DatabaseKeyError> {
    if !path.is_file()
        || SIDECARS
            .iter()
            .any(|suffix| files::append(path, suffix).exists())
    {
        return Err(DatabaseKeyError::Busy);
    }
    Ok(())
}
fn same_directory_regular(target: &Path, candidate: &Path) -> Result<PathBuf, DatabaseKeyError> {
    if !candidate.is_file() {
        return Err(DatabaseKeyError::InvalidMetadata);
    }
    let candidate = candidate
        .canonicalize()
        .map_err(|_| DatabaseKeyError::Storage)?;
    if candidate.parent() != target.parent() || candidate == target {
        return Err(DatabaseKeyError::InvalidMetadata);
    }
    Ok(candidate)
}
fn file_name(path: &Path) -> Result<String, DatabaseKeyError> {
    path.file_name()
        .and_then(|v| v.to_str())
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
        .ok_or(DatabaseKeyError::InvalidMetadata)
}
fn marker_path(path: &Path) -> PathBuf {
    files::append(path, MARKER_SUFFIX)
}
fn publish_marker(path: &Path, marker: &Marker) -> Result<(), DatabaseKeyError> {
    let final_path = marker_path(path);
    let pending = files::append(&final_path, ".pending");
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&pending)
        .map_err(|_| DatabaseKeyError::Storage)?;
    file.write_all(&serde_json::to_vec(marker).map_err(|_| DatabaseKeyError::Storage)?)
        .map_err(|_| DatabaseKeyError::Storage)?;
    file.sync_all().map_err(|_| DatabaseKeyError::Storage)?;
    drop(file);
    std::fs::rename(&pending, &final_path).map_err(|_| DatabaseKeyError::Storage)?;
    sync_parent(&final_path)
}
fn read_marker(path: &Path) -> Result<Marker, DatabaseKeyError> {
    let file = File::open(marker_path(path)).map_err(|_| DatabaseKeyError::InterruptedRestore)?;
    if file
        .metadata()
        .map_err(|_| DatabaseKeyError::Storage)?
        .len()
        > 4096
    {
        return Err(DatabaseKeyError::InvalidMetadata);
    }
    let mut bytes = Vec::new();
    file.take(4097)
        .read_to_end(&mut bytes)
        .map_err(|_| DatabaseKeyError::Storage)?;
    let marker: Marker =
        serde_json::from_slice(&bytes).map_err(|_| DatabaseKeyError::InvalidMetadata)?;
    if marker.version != 1
        || marker.confirmation_id.parse::<Uuid>().is_err()
        || Path::new(&marker.quarantine_name)
            .file_name()
            .and_then(|v| v.to_str())
            != Some(&marker.quarantine_name)
    {
        return Err(DatabaseKeyError::InvalidMetadata);
    }
    Ok(marker)
}
fn finish_marker(path: &Path) -> Result<(), DatabaseKeyError> {
    std::fs::remove_file(marker_path(path)).map_err(|_| DatabaseKeyError::Storage)?;
    sync_parent(path)
}
fn move_file(from: &Path, to: &Path) -> Result<(), DatabaseKeyError> {
    std::fs::rename(from, to).map_err(|_| DatabaseKeyError::Storage)?;
    sync_parent(to)
}
fn verify_hash(path: &Path, expected: &str) -> Result<(), DatabaseKeyError> {
    if hash(path)? == expected {
        Ok(())
    } else {
        Err(DatabaseKeyError::StaleFilesystem)
    }
}
fn hash(path: &Path) -> Result<String, DatabaseKeyError> {
    let mut file = File::open(path).map_err(|_| DatabaseKeyError::Storage)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| DatabaseKeyError::Storage)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}
fn sync_parent(path: &Path) -> Result<(), DatabaseKeyError> {
    #[cfg(unix)]
    File::open(path.parent().ok_or(DatabaseKeyError::Storage)?)
        .and_then(|f| f.sync_all())
        .map_err(|_| DatabaseKeyError::Storage)?;
    Ok(())
}

#[cfg(test)]
mod tests;
