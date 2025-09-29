use std::cmp;
use std::fs;
use std::path::{Component, Path, PathBuf};

use base64::{Engine as _, engine::general_purpose};
use git2::{Repository, Tree};

use crate::diff::types::{FileVersion, GetDiffResponse};

#[tauri::command(rename_all = "snake_case")]
pub fn get_diff(repo_path: &str, file_path: &str) -> Result<GetDiffResponse, String> {
    let repo_path = Path::new(repo_path);
    let requested_path = Path::new(file_path);

    if requested_path.is_absolute()
        || requested_path.components().any(|c| {
            matches!(
                c,
                Component::RootDir | Component::Prefix(_) | Component::ParentDir
            )
        })
    {
        return Err("Invalid file path".to_string());
    }

    let relative_path = normalize_relative_path(requested_path);

    let repository =
        Repository::open(repo_path).map_err(|e| format!("Failed to open repo: {e}"))?;

    let head_version = read_head_version(&repository, &relative_path)?;
    let workdir_version = read_workdir_version(repo_path, &relative_path)?;

    Ok(GetDiffResponse {
        file_path: relative_path
            .to_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| file_path.to_string()),
        head: head_version,
        workdir: workdir_version,
    })
}

fn normalize_relative_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            _ => {}
        }
    }
    normalized
}

fn read_head_version(repository: &Repository, path: &Path) -> Result<Option<FileVersion>, String> {
    let head = match repository.head() {
        Ok(head) => head,
        Err(_) => return Ok(None),
    };

    let commit = match head.peel_to_commit() {
        Ok(commit) => commit,
        Err(_) => return Ok(None),
    };

    let tree = commit
        .tree()
        .map_err(|e| format!("Failed to read HEAD tree: {e}"))?;

    read_tree_entry(repository, &tree, path)
}

fn read_tree_entry(
    repository: &Repository,
    tree: &Tree,
    path: &Path,
) -> Result<Option<FileVersion>, String> {
    let entry = match tree.get_path(path) {
        Ok(entry) => entry,
        Err(_) => return Ok(None),
    };

    let object = entry
        .to_object(repository)
        .map_err(|e| format!("Failed to load blob for {:?}: {e}", path))?;

    let blob = match object.as_blob() {
        Some(blob) => blob,
        None => return Ok(None),
    };

    let bytes = blob.content();
    let is_binary = blob.is_binary();
    let (content, encoding) = if is_binary {
        (
            general_purpose::STANDARD.encode(bytes),
            Some(String::from("base64")),
        )
    } else {
        (
            String::from_utf8_lossy(bytes).to_string(),
            Some(String::from("utf8")),
        )
    };

    Ok(Some(FileVersion {
        content,
        encoding,
        is_binary,
        byte_length: bytes.len(),
    }))
}

fn read_workdir_version(repo_root: &Path, path: &Path) -> Result<Option<FileVersion>, String> {
    let absolute_path = repo_root.join(path);

    if !absolute_path.exists() || !absolute_path.is_file() {
        return Ok(None);
    }

    let bytes = fs::read(&absolute_path)
        .map_err(|e| format!("Failed to read working tree file {:?}: {e}", path))?;
    let is_binary = is_probably_binary(&bytes);

    let (content, encoding) = if is_binary {
        (
            general_purpose::STANDARD.encode(&bytes),
            Some(String::from("base64")),
        )
    } else {
        (
            String::from_utf8_lossy(&bytes).to_string(),
            Some(String::from("utf8")),
        )
    };

    Ok(Some(FileVersion {
        content,
        encoding,
        is_binary,
        byte_length: bytes.len(),
    }))
}

fn is_probably_binary(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }

    let sample_size = cmp::min(bytes.len(), 1024);
    let mut non_text_bytes = 0usize;

    for &byte in &bytes[..sample_size] {
        if byte == 0 {
            return true;
        }

        // ? Allow common whitespace characters
        if !(byte == b'\t' || byte == b'\n' || byte == b'\r' || (byte >= 0x20 && byte <= 0x7E)) {
            non_text_bytes += 1;
        }
    }

    non_text_bytes as f32 / sample_size as f32 > 0.30
}
