//! Only known files beneath one native-owned, marker-validated fixture root.
use super::invalid;
use crate::CollaborationError;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub const APPLICATION_ID: &str = "com.ruru.gitru.e2e.collaboration";
const MAX_MARKER: u64 = 4096;

#[derive(Deserialize)]
struct RunMarker {
    version: u32,
    application_id: String,
    run_nonce: String,
}

#[derive(Clone)]
pub(super) struct OwnedRoot {
    path: PathBuf,
    nonce: String,
    #[cfg(unix)]
    identity: (u64, u64),
}

impl OwnedRoot {
    pub fn validate(path: &Path, nonce: &str) -> Result<Self, CollaborationError> {
        if uuid::Uuid::parse_str(nonce).is_err() || path.as_os_str().is_empty() {
            return Err(invalid());
        }
        let meta = fs::symlink_metadata(path).map_err(|_| invalid())?;
        if !meta.is_dir() || meta.file_type().is_symlink() {
            return Err(invalid());
        }
        let canonical = fs::canonicalize(path).map_err(|_| invalid())?;
        if canonical != path {
            return Err(invalid());
        }
        #[cfg(unix)]
        let identity = {
            use std::os::unix::fs::MetadataExt;
            if meta.mode() & 0o077 != 0 {
                return Err(invalid());
            }
            (meta.dev(), meta.ino())
        };
        let root = Self {
            path: canonical,
            nonce: nonce.into(),
            #[cfg(unix)]
            identity,
        };
        root.check()?;
        Ok(root)
    }

    pub fn check(&self) -> Result<(), CollaborationError> {
        let meta = fs::symlink_metadata(&self.path).map_err(|_| invalid())?;
        if !meta.is_dir() || meta.file_type().is_symlink() {
            return Err(invalid());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if (meta.dev(), meta.ino()) != self.identity || meta.mode() & 0o077 != 0 {
                return Err(invalid());
            }
        }
        let bytes = self.read("run.json", MAX_MARKER)?.ok_or_else(invalid)?;
        let marker: RunMarker = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
        if marker.version != 1
            || marker.application_id != APPLICATION_ID
            || marker.run_nonce != self.nonce
        {
            return Err(invalid());
        }
        for name in [
            "collaboration.sqlite",
            "collaboration.sqlite-wal",
            "collaboration.sqlite-shm",
            "collaboration.sqlite.lock",
            "harness-state.json",
        ] {
            self.file(name)?;
        }
        let vault = self.path.join("vault");
        match fs::symlink_metadata(&vault) {
            Ok(meta) if !meta.is_dir() || meta.file_type().is_symlink() => return Err(invalid()),
            Ok(meta) => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if meta.permissions().mode() & 0o077 != 0 {
                        return Err(invalid());
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(invalid()),
        }
        Ok(())
    }

    pub fn file(&self, name: &str) -> Result<PathBuf, CollaborationError> {
        let directory = fs::symlink_metadata(&self.path).map_err(|_| invalid())?;
        if !directory.is_dir() || directory.file_type().is_symlink() {
            return Err(invalid());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if (directory.dev(), directory.ino()) != self.identity {
                return Err(invalid());
            }
        }
        // Names here originate only in native constants or SHA256 encodings.
        if name.is_empty()
            || name.contains(['/', '\\'])
            || name == "."
            || name == ".."
            || name.chars().any(char::is_control)
        {
            return Err(invalid());
        }
        let path = self.path.join(name);
        match fs::symlink_metadata(&path) {
            Ok(meta) if !meta.is_file() || meta.file_type().is_symlink() => Err(invalid()),
            Ok(_) => Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(path),
            Err(_) => Err(invalid()),
        }
    }

    pub fn read(&self, name: &str, limit: u64) -> Result<Option<Vec<u8>>, CollaborationError> {
        let path = self.file(name)?;
        let file = match File::open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(invalid()),
        };
        if file.metadata().map_err(|_| invalid())?.len() > limit {
            return Err(invalid());
        }
        let mut bytes = vec![];
        file.take(limit + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| invalid())?;
        if bytes.len() as u64 > limit {
            return Err(invalid());
        }
        Ok(Some(bytes))
    }

    pub fn write<T: Serialize>(&self, name: &str, value: &T) -> Result<(), CollaborationError> {
        let bytes = serde_json::to_vec(value).map_err(|_| invalid())?;
        self.write_bytes(name, &bytes)
    }

    pub fn write_bytes(&self, name: &str, bytes: &[u8]) -> Result<(), CollaborationError> {
        let destination = self.file(name)?;
        let temp = self.file(&format!(".ruru103-{}.tmp", uuid::Uuid::new_v4()))?;
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let result = (|| {
            let mut file = options.open(&temp).map_err(|_| invalid())?;
            file.write_all(bytes).map_err(|_| invalid())?;
            file.sync_all().map_err(|_| invalid())?;
            drop(file);
            // Recheck before replacement rather than following a newly installed
            // destination symlink. Fixture roots are private to the launch owner.
            self.file(name)?;
            fs::rename(&temp, &destination).map_err(|_| invalid())?;
            self.sync_directory()
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result
    }

    pub fn vault(&self) -> Result<Self, CollaborationError> {
        self.check()?;
        let path = self.path.join("vault");
        if !path.exists() {
            let mut builder = fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            builder.create(&path).map_err(|_| invalid())?;
            self.sync_directory()?;
        }
        let meta = fs::symlink_metadata(&path).map_err(|_| invalid())?;
        if !meta.is_dir() || meta.file_type().is_symlink() {
            return Err(invalid());
        }
        Ok(Self {
            path,
            nonce: self.nonce.clone(),
            #[cfg(unix)]
            identity: {
                use std::os::unix::fs::MetadataExt;
                (meta.dev(), meta.ino())
            },
        })
    }

    pub fn remove(&self, name: &str) -> Result<(), CollaborationError> {
        match fs::remove_file(self.file(name)?) {
            Ok(()) => self.sync_directory(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(invalid()),
        }
    }

    fn sync_directory(&self) -> Result<(), CollaborationError> {
        #[cfg(unix)]
        File::open(&self.path)
            .and_then(|file| file.sync_all())
            .map_err(|_| invalid())?;
        Ok(())
    }
}
