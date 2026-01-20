use serde::Serialize;
use std::{process::Command, time};

#[derive(Serialize)]
pub struct FileDiff {
    pub patch: String,
}

// TODO(ruru):
// ! sooo i have some thoughts here.
// ? i have think a lot about useing libgit2 at this point
// ? but as farrr it seems there is huge issues with index
// ? or my skill issue may be. but we are going with row
// ? git CLI for tasks like patchs and blams, seens git CLI
// ? is soo comfortable with this kind of comlex performace
// ? may be in future we slowing start adopting this kind
// ? performace blockers. let's have this row implementation
// ? for now. BRB
#[tauri::command]
pub async fn get_patch_by_file_path(repo_path: &str, file_path: &str) -> Result<FileDiff, String> {
    let start = time::Instant::now();
    let patch = git_diff_any(repo_path, file_path).map_err(|e| e.to_string())?;
    println!("{:?}", start.elapsed());

    return Ok(FileDiff { patch });
}

fn git_diff_any(repo_path: &str, file_path: &str) -> Result<String, String> {
    let out = Command::new("git")
        .current_dir(repo_path)
        .args([
            "diff",
            "--no-ext-diff",
            "--patch-with-raw",
            "--no-color",
            "HEAD",
            "--",
            file_path,
        ])
        .output()
        .map_err(|e| e.to_string())?;

    if !out.status.success() && out.status.code() != Some(1) {
        return Err(String::from_utf8_lossy(&out.stderr).to_string());
    }

    if !out.stdout.is_empty() {
        return Ok(String::from_utf8_lossy(&out.stdout).to_string());
    }

    // ? maybe untracked - why **maybe**? bcuz the whole world on it
    let abs = std::path::Path::new(repo_path).join(file_path);
    if !abs.exists() {
        return Ok(String::new());
    }

    let out = Command::new("git")
        .current_dir(repo_path)
        .args([
            "diff",
            "--no-index",
            "--patch-with-raw",
            "--no-color",
            "/dev/null",
            "--",
            file_path,
        ])
        .output()
        .map_err(|e| e.to_string())?;

    if !out.status.success() && out.status.code() != Some(1) {
        return Err(String::from_utf8_lossy(&out.stderr).to_string());
    }

    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}
