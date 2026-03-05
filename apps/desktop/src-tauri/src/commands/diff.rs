use git::core::get_services;
use git::models::diff::FileDiff;
use git::models::status::FileStatusKind;
use git::runner::validate_relative_path;
use git::AppState;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::process::Command;

#[tauri::command]
pub async fn get_patch_by_file_path(
    file_path: &str,
    file_new_path: Option<String>,
    status: Option<Vec<FileStatusKind>>,
    stash_reference: Option<String>,
    commit_hash: Option<String>,
    parent_index: Option<usize>,
    state: tauri::State<'_, AppState>,
) -> Result<FileDiff, String> {
    validate_relative_path(file_path)?;

    let services = get_services(state).await?;

    services
        .diff()
        .get_patch_by_file_path(
            file_path,
            file_new_path.as_deref(),
            status.as_deref(),
            stash_reference.as_deref(),
            commit_hash.as_deref(),
            parent_index,
        )
        .await
}

#[derive(Debug, Serialize)]
pub struct OdiffDiffResult {
    mask_data_url: Option<String>,
    mismatch_ratio: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct OdiffNodeResponse {
    ok: bool,
    mask_data_url: Option<String>,
    mismatch_ratio: Option<f64>,
    error: Option<String>,
}

#[tauri::command]
pub async fn compute_odiff_difference(
    before_path: String,
    after_path: String,
) -> Result<OdiffDiffResult, String> {
    if before_path.is_empty() || after_path.is_empty() {
        return Err("Missing image path for odiff comparison".to_string());
    }

    let desktop_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| "Failed to resolve desktop app directory".to_string())?;

    let ext = PathBuf::from(&after_path)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or("png".to_string());
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("Clock error: {e}"))?
        .as_nanos();
    let diff_output = std::env::temp_dir().join(format!("gitru-odiff-{ts}.{ext}"));

    let script = r#"
const fs = require('node:fs');
const { compare } = require('odiff-bin');

(async () => {
  const [beforePath, afterPath, diffPath] = process.argv.slice(1);
  try {
    const result = await compare(beforePath, afterPath, diffPath, {
      outputDiffMask: true,
      diffOverlay: true,
      antialiasing: true,
      threshold: 0.1,
      noFailOnFsErrors: true,
    });

    let mismatchRatio = null;
    let maskDataUrl = null;
    if (result.match === true) {
      mismatchRatio = 0;
    } else if (result.reason === 'pixel-diff') {
      if (typeof result.diffPercentage === 'number') {
        mismatchRatio = result.diffPercentage > 1 ? result.diffPercentage / 100 : result.diffPercentage;
      }
      if (fs.existsSync(diffPath)) {
        const buf = fs.readFileSync(diffPath);
        maskDataUrl = `data:image/png;base64,${buf.toString('base64')}`;
      }
    }

    console.log(JSON.stringify({
      ok: true,
      mask_data_url: maskDataUrl,
      mismatch_ratio: mismatchRatio,
    }));
  } catch (error) {
    const message = error && error.message ? error.message : String(error);
    console.log(JSON.stringify({ ok: false, error: message }));
    process.exit(2);
  } finally {
    try { if (fs.existsSync(diffPath)) fs.unlinkSync(diffPath); } catch {}
  }
})();
"#;

    let mut command = Command::new("node");
    command
        .arg("-e")
        .arg(script)
        .arg(&before_path)
        .arg(&after_path)
        .arg(&diff_output)
        .current_dir(desktop_dir);

    let output = command
        .output()
        .await
        .map_err(|e| format!("Failed to launch node for odiff: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .ok_or_else(|| "odiff produced no output".to_string())?;

    let parsed: OdiffNodeResponse = serde_json::from_str(line)
        .map_err(|e| format!("Failed parsing odiff response: {e}; raw: {line}"))?;

    if !parsed.ok {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "odiff compare failed: {}{}",
            parsed.error.unwrap_or_else(|| "unknown error".to_string()),
            if stderr.trim().is_empty() {
                "".to_string()
            } else {
                format!("; stderr: {}", stderr.trim())
            }
        ));
    }

    Ok(OdiffDiffResult {
        mask_data_url: parsed.mask_data_url,
        mismatch_ratio: parsed.mismatch_ratio,
    })
}
