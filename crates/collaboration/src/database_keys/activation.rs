//! Crash-recoverable activation of an already verified keyed candidate.
//! Candidate export and cryptographic verification stay behind a trusted native
//! boundary; this module owns only the durable filesystem transaction.

use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{CollaborationError, ErrorCode, storage};

type Result<T> = std::result::Result<T, CollaborationError>;
const MARKER_SUFFIX: &str = ".keyed-activation-pending.json";
const KEY_SUFFIX: &str = ".key.json";
const TRANSIENT_SUFFIXES: [&str; 3] = ["-wal", "-shm", "-journal"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyedActivationPreview {
    confirmation_id: String,
    recovery_bundle: PathBuf,
}

impl KeyedActivationPreview {
    pub fn confirmation_id(&self) -> &str {
        &self.confirmation_id
    }
    pub fn recovery_bundle(&self) -> &Path {
        &self.recovery_bundle
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyedActivationReceipt {
    pub recovery_bundle: PathBuf,
    pub plaintext_sha256: String,
    pub keyed_sha256: String,
}

/// Proof returned only by a native verifier after authenticating the key,
/// immutable database identity, schema/integrity, and authored durable data.
pub struct VerifiedKeyedCandidate {
    path: PathBuf,
}

impl VerifiedKeyedCandidate {
    /// # Safety
    ///
    /// The caller must have authenticated and closed every SQLite handle for
    /// `path`, and must have verified its adjacent ready key metadata, schema,
    /// integrity, immutable identity and authored durable evidence.
    pub unsafe fn from_native_verification(path: PathBuf) -> Self {
        Self { path }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Marker {
    format: u32,
    confirmation_id: String,
    candidate_name: String,
    bundle_name: String,
    plaintext_sha256: String,
    keyed_sha256: String,
    key_metadata_sha256: String,
}

pub struct KeyedActivationSession {
    _lease: storage::WriterLease,
    target: PathBuf,
    candidate: PathBuf,
    marker: Marker,
    preview: KeyedActivationPreview,
}

pub struct InterruptedKeyedActivation {
    _lease: storage::WriterLease,
    target: PathBuf,
    marker_path: PathBuf,
    marker: Marker,
    bundle: PathBuf,
}

impl InterruptedKeyedActivation {
    /// Opens only an exact durable marker. It never guesses from filenames.
    pub fn open(target: impl AsRef<Path>) -> Result<Self> {
        let target = canonical_target_name(target.as_ref())?;
        let lease = storage::acquire_writer_lease(&target)?;
        let marker_path = marker_path(&target);
        require_regular(&marker_path)?;
        let marker: Marker = serde_json::from_slice(&read_bounded(&marker_path, 4096)?)
            .map_err(|_| invalid("Keyed activation marker is invalid"))?;
        if marker.format != 1
            || marker.confirmation_id.parse::<Uuid>().is_err()
            || !safe_name(&marker.candidate_name)
            || !safe_name(&marker.bundle_name)
        {
            return Err(invalid("Keyed activation marker is invalid"));
        }
        let bundle = target.parent().unwrap().join(&marker.bundle_name);
        if !bundle.is_dir() {
            return Err(invalid("Keyed activation recovery bundle is missing"));
        }
        Ok(Self {
            _lease: lease,
            target,
            marker_path,
            marker,
            bundle,
        })
    }

    pub fn recovery_bundle(&self) -> &Path {
        &self.bundle
    }

    /// Restores the preserved plaintext source. Any activated keyed candidate is
    /// retained in the same recovery bundle for diagnosis; no key is deleted.
    pub fn rollback(self) -> Result<PathBuf> {
        let original = self.bundle.join("plaintext.sqlite");
        if self.target.exists() && hash(&self.target)? == self.marker.plaintext_sha256 {
            std::fs::remove_file(&self.marker_path).map_err(|_| storage_error())?;
            sync_parent(&self.marker_path)?;
            return Ok(self.bundle);
        }
        require_regular(&original)?;
        if hash(&original)? != self.marker.plaintext_sha256 {
            return Err(stale());
        }
        if self.target.exists() {
            if hash(&self.target)? != self.marker.keyed_sha256 {
                return Err(stale());
            }
            std::fs::rename(&self.target, self.bundle.join("keyed.sqlite"))
                .map_err(|_| storage_error())?;
        }
        let active_key = append(&self.target, KEY_SUFFIX);
        if active_key.exists() {
            if hash(&active_key)? != self.marker.key_metadata_sha256 {
                return Err(stale());
            }
            std::fs::rename(&active_key, self.bundle.join("keyed.key.json"))
                .map_err(|_| storage_error())?;
        }
        let staged = self
            .target
            .parent()
            .unwrap()
            .join(&self.marker.candidate_name);
        if staged.exists() {
            if hash(&staged)? != self.marker.keyed_sha256 {
                return Err(stale());
            }
            std::fs::rename(&staged, self.bundle.join("keyed.sqlite"))
                .map_err(|_| storage_error())?;
            let staged_key = append(&staged, KEY_SUFFIX);
            if hash(&staged_key)? != self.marker.key_metadata_sha256 {
                return Err(stale());
            }
            std::fs::rename(staged_key, self.bundle.join("keyed.key.json"))
                .map_err(|_| storage_error())?;
        }
        std::fs::rename(&original, &self.target).map_err(|_| storage_error())?;
        sync_parent(&self.target)?;
        if hash(&self.target)? != self.marker.plaintext_sha256 {
            return Err(stale());
        }
        std::fs::remove_file(&self.marker_path).map_err(|_| storage_error())?;
        sync_parent(&self.marker_path)?;
        Ok(self.bundle)
    }
}

impl KeyedActivationSession {
    pub fn prepare(target: impl AsRef<Path>, candidate: VerifiedKeyedCandidate) -> Result<Self> {
        let target = canonical_destination(target.as_ref())?;
        require_no_pending(&target)?;
        crate::recovery::require_no_pending_restore(&target)?;
        crate::database_keys::refuse_unkeyed_path(&target)?;
        let lease = storage::acquire_writer_lease(&target)?;
        let candidate = require_same_directory_file(&target, &candidate.path)?;
        if candidate == target {
            return Err(invalid("Candidate must be distinct from active storage"));
        }
        require_no_sidecars(&target)?;
        require_no_sidecars(&candidate)?;
        let candidate_key = append(&candidate, KEY_SUFFIX);
        require_regular(&candidate_key)?;
        // A candidate must already carry strict, ready metadata. Parsing here
        // prevents activation of malformed/pending reservations; authentication
        // remains the unsafe native verifier's responsibility.
        let journal = super::files::read(&candidate)
            .map_err(|_| invalid("Keyed candidate metadata is invalid"))?
            .ok_or_else(|| invalid("Keyed candidate metadata is missing"))?;
        if !journal.ready {
            return Err(invalid("Keyed candidate is not Ready"));
        }
        let id = Uuid::new_v4().to_string();
        let bundle_name = format!(".keyed-activation-{id}.recovery");
        let bundle = target.parent().unwrap().join(&bundle_name);
        let marker = Marker {
            format: 1,
            confirmation_id: id.clone(),
            candidate_name: file_name(&candidate)?,
            bundle_name,
            plaintext_sha256: hash(&target)?,
            keyed_sha256: hash(&candidate)?,
            key_metadata_sha256: hash(&candidate_key)?,
        };
        Ok(Self {
            _lease: lease,
            target,
            candidate,
            preview: KeyedActivationPreview {
                confirmation_id: id,
                recovery_bundle: bundle,
            },
            marker,
        })
    }

    pub fn preview(&self) -> &KeyedActivationPreview {
        &self.preview
    }

    pub fn confirm(self, confirmation_id: &str) -> Result<KeyedActivationReceipt> {
        if confirmation_id != self.marker.confirmation_id {
            return Err(invalid("Confirm the inspected keyed activation preview"));
        }
        if hash(&self.target)? != self.marker.plaintext_sha256
            || hash(&self.candidate)? != self.marker.keyed_sha256
            || hash(&append(&self.candidate, KEY_SUFFIX))? != self.marker.key_metadata_sha256
        {
            return Err(stale());
        }
        require_no_sidecars(&self.target)?;
        require_no_sidecars(&self.candidate)?;
        let marker_path = marker_path(&self.target);
        publish_marker(&marker_path, &self.marker)?;
        let bundle = self.preview.recovery_bundle.clone();
        std::fs::create_dir(&bundle).map_err(|_| storage_error())?;
        sync_parent(&bundle)?;
        std::fs::rename(&self.target, bundle.join("plaintext.sqlite"))
            .map_err(|_| storage_error())?;
        sync_parent(&self.target)?;
        std::fs::rename(&self.candidate, &self.target).map_err(|_| storage_error())?;
        std::fs::rename(
            append(&self.candidate, KEY_SUFFIX),
            append(&self.target, KEY_SUFFIX),
        )
        .map_err(|_| storage_error())?;
        sync_parent(&self.target)?;
        if hash(&self.target)? != self.marker.keyed_sha256
            || hash(&append(&self.target, KEY_SUFFIX))? != self.marker.key_metadata_sha256
        {
            return Err(stale());
        }
        write_bundle_manifest(&bundle, &self.marker)?;
        std::fs::remove_file(&marker_path).map_err(|_| storage_error())?;
        sync_parent(&marker_path)?;
        Ok(KeyedActivationReceipt {
            recovery_bundle: bundle,
            plaintext_sha256: self.marker.plaintext_sha256,
            keyed_sha256: self.marker.keyed_sha256,
        })
    }
}

pub fn require_no_pending(path: &Path) -> Result<()> {
    if marker_path(path).exists() {
        return Err(CollaborationError::new(
            ErrorCode::NotReady,
            "Keyed storage activation is interrupted; files were preserved",
        ));
    }
    Ok(())
}

fn canonical_destination(path: &Path) -> Result<PathBuf> {
    if !path.is_absolute() || path.file_name().is_none() {
        return Err(invalid("Invalid database path"));
    }
    storage::prepare_private_path(path)?;
    let parent = path
        .parent()
        .unwrap()
        .canonicalize()
        .map_err(|_| storage_error())?;
    let path = parent.join(path.file_name().unwrap());
    require_regular(&path)?;
    Ok(path)
}
fn canonical_target_name(path: &Path) -> Result<PathBuf> {
    if !path.is_absolute() || path.file_name().is_none() {
        return Err(invalid("Invalid database path"));
    }
    storage::prepare_private_path(path)?;
    let parent = path
        .parent()
        .unwrap()
        .canonicalize()
        .map_err(|_| storage_error())?;
    Ok(parent.join(path.file_name().unwrap()))
}
fn require_same_directory_file(target: &Path, path: &Path) -> Result<PathBuf> {
    require_regular(path)?;
    let canonical = path.canonicalize().map_err(|_| storage_error())?;
    if canonical.parent() != target.parent() {
        return Err(invalid("Candidate must use target directory"));
    }
    Ok(canonical)
}
fn require_regular(path: &Path) -> Result<()> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| storage_error())?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(invalid("Expected a regular private file"));
    }
    Ok(())
}
fn require_no_sidecars(path: &Path) -> Result<()> {
    if TRANSIENT_SUFFIXES
        .iter()
        .any(|suffix| append(path, suffix).exists())
    {
        return Err(CollaborationError::new(
            ErrorCode::Busy,
            "Checkpoint and close database sidecars before activation",
        ));
    }
    Ok(())
}
fn hash(path: &Path) -> Result<String> {
    let mut file = File::open(path).map_err(|_| storage_error())?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|_| storage_error())?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}
fn read_bounded(path: &Path, maximum: u64) -> Result<Vec<u8>> {
    let file = File::open(path).map_err(|_| storage_error())?;
    if file.metadata().map_err(|_| storage_error())?.len() > maximum {
        return Err(invalid("Keyed activation marker is invalid"));
    }
    let mut bytes = Vec::new();
    file.take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| storage_error())?;
    if bytes.len() as u64 > maximum {
        return Err(invalid("Keyed activation marker is invalid"));
    }
    Ok(bytes)
}
fn publish_marker(path: &Path, marker: &Marker) -> Result<()> {
    let bytes = serde_json::to_vec(marker).map_err(|_| storage_error())?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|_| storage_error())?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| storage_error())?;
    sync_parent(path)
}
fn write_bundle_manifest(bundle: &Path, marker: &Marker) -> Result<()> {
    let path = bundle.join("manifest.json");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|_| storage_error())?;
    file.write_all(&serde_json::to_vec(marker).map_err(|_| storage_error())?)
        .and_then(|_| file.sync_all())
        .map_err(|_| storage_error())?;
    sync_parent(&path)
}
fn sync_parent(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        File::open(path.parent().ok_or_else(storage_error)?)
            .and_then(|f| f.sync_all())
            .map_err(|_| storage_error())?;
    }
    Ok(())
}
fn append(path: &Path, suffix: &str) -> PathBuf {
    super::files::append(path, suffix)
}
fn marker_path(path: &Path) -> PathBuf {
    append(path, MARKER_SUFFIX)
}
fn file_name(path: &Path) -> Result<String> {
    path.file_name()
        .and_then(|v| v.to_str())
        .map(str::to_owned)
        .ok_or_else(|| invalid("Invalid candidate filename"))
}
fn safe_name(value: &str) -> bool {
    !value.is_empty() && Path::new(value).file_name().and_then(|v| v.to_str()) == Some(value)
}
fn invalid(message: &'static str) -> CollaborationError {
    CollaborationError::invalid(message)
}
fn stale() -> CollaborationError {
    CollaborationError::new(
        ErrorCode::LocalStateChanged,
        "Activation inputs changed; inspect them again",
    )
}
fn storage_error() -> CollaborationError {
    CollaborationError::storage()
}

#[cfg(test)]
mod tests;
