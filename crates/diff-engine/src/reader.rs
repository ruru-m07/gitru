use std::path::Path;

use git2::Oid;
use tokio::process::Command;

/// Reads file content relevant to diffing: the staged (index) version
/// as "old" and the working-tree file as "new".
pub struct FileContentReader;

/// Content of a single side of a diff, plus the git blob OID (used as
/// a stable cache key in Phase 6). None means the file didn't exist on
/// that side (new file / deleted file).
pub struct FileContent {
    pub text: String,
    /// Blob OID from the git index; None for working-tree content.
    pub oid: Option<Oid>,
}

impl FileContentReader {
    /// Read the staged (index) version of `file_path` inside `repo_path`.
    ///
    /// Returns:
    /// - `Ok(Some(fc))` — file is tracked and readable UTF-8 text.
    /// - `Ok(None)`     — file is not in the index (untracked / new file
    ///                    that hasn't been staged yet).
    /// - `Err(_)`       — IO / git error.
    /// ---------------------------------------------
    // let repo = Repository::open(repo_path).map_err(|e| e.to_string())?;
    // let index = repo.index().map_err(|e| e.to_string())?;

    // let entry = match index.get_path(Path::new(file_path), 0) {
    //     Some(e) => e,
    //     // File is not tracked in the index — treat as new file.
    //     None => return Ok(None),
    // };

    // let oid = entry.id;
    // let blob = repo.find_blob(oid).map_err(|e| e.to_string())?;

    // // Skip binary blobs gracefully.
    // if blob.is_binary() {
    //     return Ok(None);
    // }

    // let text = std::str::from_utf8(blob.content())
    //     .map(|s| s.to_string())
    //     .map_err(|e| format!("file {file_path} is not valid UTF-8: {e}"))?;

    // Ok(Some(FileContent {
    //     text,
    //     oid: Some(oid),
    // }))
    #[logger::logger]
    pub async fn read_index_content(
        repo_path: &str,
        file_path: &str,
    ) -> Result<Option<FileContent>, String> {
        let output = Command::new("git")
            .args(["show", &format!(":{}", file_path)])
            .current_dir(repo_path)
            .output()
            .await
            .map_err(|e| e.to_string())?;

        if !output.status.success() {
            return Ok(None);
        }

        let text = String::from_utf8(output.stdout)
            .map_err(|e| format!("file {file_path} is not valid UTF-8: {e}"))?;

        Ok(Some(FileContent {
            text,
            oid: None, // can't get it without extra cost
        }))
    }

    /// Read the working-tree (on-disk) version of `file_path`.
    ///
    /// Returns `Ok(None)` if the file does not exist on disk (deleted).
    #[logger::logger]
    pub fn read_working_tree_content(
        repo_path: &str,
        file_path: &str,
    ) -> Result<Option<FileContent>, String> {
        let full_path = Path::new(repo_path).join(file_path);

        if !full_path.exists() {
            return Ok(None);
        }

        let text = std::fs::read_to_string(&full_path)
            .map_err(|e| format!("failed to read {file_path}: {e}"))?;

        Ok(Some(FileContent { text, oid: None }))
    }
}
