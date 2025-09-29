use std::path::Path;

use git2::{DiffOptions, Repository};

#[tauri::command(rename_all = "snake_case")]
pub fn get_diff(repo_path: &str, file_path: &str) -> Result<String, String> {
    let repo_path = Path::new(repo_path);
    let file_path = Path::new(file_path);

    let repository =
        Repository::open(repo_path).map_err(|e| format!("Failed to open repo: {e}"))?;

    let mut opts = DiffOptions::new();
    opts.pathspec(file_path);

    let head_commit = repository
        .head()
        .ok()
        .and_then(|h| h.target())
        .and_then(|oid| repository.find_commit(oid).ok())
        .ok_or("Could not find HEAD commit")?;

    let tree = head_commit.tree().map_err(|e| e.to_string())?;
    let diff = repository
        .diff_tree_to_workdir_with_index(Some(&tree), Some(&mut opts))
        .map_err(|e| e.to_string())?;

    let mut output = String::new();

    diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
        use std::fmt::Write;

        // '+' - Added line
        // '-' - Removed line
        // ' ' - Context line (unchanged)
        // '=' - No newline at end of file
        // '>' - File header
        // '<' - File header
        // 'F' - File header
        // 'H' - Hunk header
        let origin = match line.origin() {
            '+' => "+",
            '-' => "-",
            ' ' => " ",
            _ => "",
        };

        let _ = write!(
            output,
            "{}{}",
            origin,
            std::str::from_utf8(line.content()).unwrap_or("")
        );
        true
    })
    .map_err(|e| e.to_string())?;

    Ok(output)
}
