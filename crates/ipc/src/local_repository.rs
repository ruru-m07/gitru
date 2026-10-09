//! Native registration evidence. Paths and filesystem IDs never cross IPC.
use crate::repo_manager::RepositoryInfo;
use git::{
    core::RepoServices,
    models::remotes::{RemoteObservationError, RemoteSnapshot},
    service::remotes::NativeWorktreePaths,
};
use sha2::{Digest, Sha256};
use std::path::Path;

pub struct RegisteredRemoteObservation {
    pub proof: String,
    pub remotes: RemoteSnapshot,
}

pub async fn observe_registered_repository(
    repo: &RepositoryInfo,
) -> Result<RegisteredRemoteObservation, RemoteObservationError> {
    let service = RepoServices::new(&repo.path).map_err(|_| RemoteObservationError::Unavailable)?;
    let before = service.remotes().worktree_paths().await?;
    let proof = registration_proof(repo, &before)?;
    let remotes = service.remotes().snapshot().await?;
    let after = service.remotes().worktree_paths().await?;
    if registration_proof(repo, &after)? != proof {
        return Err(RemoteObservationError::Changed);
    }
    Ok(RegisteredRemoteObservation { proof, remotes })
}

/// Recompute registration identity from already-inspected native worktree
/// coordinates. This performs filesystem identity checks only and is safe to
/// use while the Git command transaction remains held.
pub fn registration_proof(
    repo: &RepositoryInfo,
    paths: &NativeWorktreePaths,
) -> Result<String, RemoteObservationError> {
    let registered = Path::new(&repo.path)
        .canonicalize()
        .map_err(|_| RemoteObservationError::Unavailable)?;
    if registered != paths.worktree {
        return Err(RemoteObservationError::Unavailable);
    }
    let mut hash = Sha256::new();
    hash.update(repo.id.as_bytes());
    for path in [&paths.worktree, &paths.git_dir, &paths.common_dir] {
        let text = path.to_str().ok_or(RemoteObservationError::Unavailable)?;
        hash.update((text.len() as u64).to_le_bytes());
        hash.update(text.as_bytes());
        let (volume, index) = directory_identity(path)?;
        hash.update(volume.to_le_bytes());
        hash.update(index.to_le_bytes());
    }
    Ok(hash
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

#[cfg(unix)]
fn directory_identity(path: &Path) -> Result<(u64, u64), RemoteObservationError> {
    use std::os::unix::fs::MetadataExt;
    let metadata = std::fs::metadata(path).map_err(|_| RemoteObservationError::Unavailable)?;
    if !metadata.is_dir() {
        return Err(RemoteObservationError::Unavailable);
    }
    Ok((metadata.dev(), metadata.ino()))
}
#[cfg(windows)]
fn directory_identity(path: &Path) -> Result<(u64, u64), RemoteObservationError> {
    use std::{
        mem::MaybeUninit,
        os::windows::{fs::OpenOptionsExt, io::AsRawHandle},
    };
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, FILE_FLAG_BACKUP_SEMANTICS, GetFileInformationByHandle,
    };
    let handle = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)
        .map_err(|_| RemoteObservationError::Unavailable)?;
    let mut information = MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
    // The handle remains alive and the API initializes the complete value on success.
    if unsafe { GetFileInformationByHandle(handle.as_raw_handle(), information.as_mut_ptr()) } == 0
    {
        return Err(RemoteObservationError::Unavailable);
    }
    let information = unsafe { information.assume_init() };
    Ok((
        u64::from(information.dwVolumeSerialNumber),
        (u64::from(information.nFileIndexHigh) << 32) | u64::from(information.nFileIndexLow),
    ))
}
#[cfg(not(any(unix, windows)))]
fn directory_identity(_: &Path) -> Result<(u64, u64), RemoteObservationError> {
    Err(RemoteObservationError::Unavailable)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn repo(path: &Path) -> RepositoryInfo {
        RepositoryInfo {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Local".into(),
            path: path.to_str().unwrap().into(),
            origin: None,
            current_branch: None,
            ahead_behind: None,
            has_uncommitted_changes: false,
            last_updated: 0,
        }
    }
    #[test]
    fn registration_identity_survives_display_rename_but_not_directory_or_registration_reuse() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("repo");
        std::fs::create_dir(&path).unwrap();
        let git = path.join(".git");
        std::fs::create_dir(&git).unwrap();
        let paths = NativeWorktreePaths {
            worktree: path.canonicalize().unwrap(),
            git_dir: git.canonicalize().unwrap(),
            common_dir: git.canonicalize().unwrap(),
        };
        let mut registered = repo(&path);
        let original = registration_proof(&registered, &paths).unwrap();
        registered.name = "Renamed".into();
        assert_eq!(registration_proof(&registered, &paths).unwrap(), original);
        let mut another = registered.clone();
        another.id = uuid::Uuid::new_v4().to_string();
        assert_ne!(registration_proof(&another, &paths).unwrap(), original);
        std::fs::rename(&path, root.path().join("retained-original")).unwrap();
        std::fs::create_dir(&path).unwrap();
        std::fs::create_dir(path.join(".git")).unwrap();
        assert_ne!(registration_proof(&registered, &paths).unwrap(), original);
        std::fs::remove_dir_all(&path).unwrap();
        assert!(registration_proof(&registered, &paths).is_err());
    }
}
