use crate::models::stash::{StashEntry, StashQuickStat};
use crate::models::status::{FileStatus, FileStatusKind};

/// Parse the output of `git stash list --format=%gd%x1f%gs`.
///
/// Each line: `stash@{N}<US>message`
pub fn parse_stash_list(output: &str) -> Result<Vec<StashEntry>, String> {
    let mut result = Vec::new();

    for line in output.lines() {
        if line.is_empty() {
            continue;
        }

        let mut parts = line.splitn(2, '\x1f');
        let reference = parts.next().ok_or("missing stash reference")?.to_string();
        let message = parts.next().unwrap_or("").to_string();

        let index = parse_stash_index(&reference)?;
        let is_gitru = parse_gitru_stash_message(&message).is_some();
        let branch = parse_branch_from_message(&message);

        result.push(StashEntry {
            index,
            reference,
            message,
            branch,
            is_gitru,
        });
    }

    Ok(result)
}

/// Parse the summary line from `git stash show --stat` output.
///
/// Example last line: ` 3 files changed, 10 insertions(+), 2 deletions(-)`
pub fn parse_stash_stat(output: &str, reference: &str) -> Result<StashQuickStat, String> {
    // The stat summary is the last non-empty line
    let summary_line = output
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("");

    let mut files_changed = 0usize;
    let mut insertions = 0usize;
    let mut deletions = 0usize;

    if summary_line.eq_ignore_ascii_case("no local changes to save") {
        return Ok(StashQuickStat {
            reference: reference.to_string(),
            files_changed: 0,
            insertions: 0,
            deletions: 0,
        });
    }

    // Parse numbers from the summary line
    // Format: " N file(s) changed, N insertion(s)(+), N deletion(s)(-)"
    for part in summary_line.split(',') {
        let part = part.trim();
        if part.contains("file") && part.contains("changed") {
            if let Some(n) = extract_leading_number(part) {
                files_changed = n;
            }
        } else if part.contains("insertion") {
            if let Some(n) = extract_leading_number(part) {
                insertions = n;
            }
        } else if part.contains("deletion") {
            if let Some(n) = extract_leading_number(part) {
                deletions = n;
            }
        }
    }

    Ok(StashQuickStat {
        reference: reference.to_string(),
        files_changed,
        insertions,
        deletions,
    })
}

/// Parse the output of `git stash show --name-status -z <ref>`.
///
/// With `-z`, git outputs NUL-separated alternating fields:
///   `<status>\0<path>\0` for regular changes
///   `<status>\0<new_path>\0<old_path>\0` for renames/copies
pub fn parse_stash_file_status(output: &[u8]) -> Result<Vec<FileStatus>, String> {
    let mut result = Vec::new();
    let segments: Vec<&[u8]> = output.split(|b| *b == 0).collect();
    let mut i = 0;

    while i < segments.len() {
        let segment = segments[i];
        if segment.is_empty() {
            i += 1;
            continue;
        }

        let status_str = std::str::from_utf8(segment).map_err(|e| e.to_string())?;
        let first_char = status_str.as_bytes().first().copied().unwrap_or(b'?');

        match first_char {
            b'R' | b'C' => {
                // Rename/Copy with --name-status -z:
                // status\0old_path\0new_path\0
                let old_path = segments
                    .get(i + 1)
                    .and_then(|s| std::str::from_utf8(s).ok())
                    .unwrap_or("")
                    .to_string();
                let new_path = segments
                    .get(i + 2)
                    .and_then(|s| std::str::from_utf8(s).ok())
                    .unwrap_or("")
                    .to_string();

                result.push(FileStatus {
                    path: old_path,
                    new_path: Some(new_path),
                    status: vec![FileStatusKind::IndexRenamed],
                });
                i += 3;
            }
            b'A' | b'M' | b'D' | b'T' => {
                // Regular change: next NUL-segment is the path
                let path = segments
                    .get(i + 1)
                    .and_then(|s| std::str::from_utf8(s).ok())
                    .unwrap_or("")
                    .to_string();

                let kind = match first_char {
                    b'A' => FileStatusKind::IndexNew,
                    b'M' => FileStatusKind::IndexModified,
                    b'D' => FileStatusKind::IndexDeleted,
                    b'T' => FileStatusKind::IndexTypechange,
                    _ => unreachable!(),
                };

                result.push(FileStatus {
                    path,
                    new_path: None,
                    status: vec![kind],
                });
                i += 2;
            }
            _ => {
                // Unknown status — skip
                i += 1;
            }
        }
    }

    Ok(result)
}

/// Parse a `!!Gitru<from> -> <to>` stash message.
///
/// Returns `(from_branch, to_branch)` if the message matches the pattern.
pub fn parse_gitru_stash_message(message: &str) -> Option<(String, String)> {
    let marker = "!!Gitru<";
    let lower = message.to_ascii_lowercase();
    let lower_marker = marker.to_ascii_lowercase();
    let marker_start = lower.find(&lower_marker)?;
    let after_prefix = &message[marker_start + marker.len()..];
    let from_end = after_prefix.find('>')?;
    let from_branch = after_prefix[..from_end].trim();

    let after_from = &after_prefix[from_end + 1..];
    let after_arrow = after_from.strip_prefix(" -> <")?;
    let to_end = after_arrow.find('>')?;
    let to_branch = after_arrow[..to_end].trim();

    if from_branch.is_empty() || to_branch.is_empty() {
        return None;
    }

    Some((from_branch.to_string(), to_branch.to_string()))
}

/// Check whether a stash's target branch matches a given branch name,
/// accounting for `origin/` prefix differences.
pub fn branch_name_matches(stash_target: &str, branch: &str) -> bool {
    stash_target == branch
        || stash_target
            .strip_prefix("origin/")
            .is_some_and(|s| s == branch)
        || branch
            .strip_prefix("origin/")
            .is_some_and(|s| s == stash_target)
}

/// Validate that a stash reference matches the `stash@{N}` pattern.
pub fn validate_stash_ref(reference: &str) -> Result<(), String> {
    if reference.starts_with("stash@{") && reference.ends_with('}') {
        let inner = &reference[7..reference.len() - 1];
        if inner.parse::<usize>().is_ok() {
            return Ok(());
        }
    }
    Err(format!(
        "Invalid stash reference '{}': expected format stash@{{N}}",
        reference
    ))
}

// ── helpers ──────────────────────────────────────────────────────────

/// Extract the stash index from a reference like `stash@{3}`.
fn parse_stash_index(reference: &str) -> Result<usize, String> {
    if reference.starts_with("stash@{") && reference.ends_with('}') {
        let inner = &reference[7..reference.len() - 1];
        inner
            .parse::<usize>()
            .map_err(|_| format!("invalid stash index in '{}'", reference))
    } else {
        Err(format!("invalid stash reference '{}'", reference))
    }
}

/// Extract branch name from a standard git stash message like "On main: WIP on …".
fn parse_branch_from_message(message: &str) -> Option<String> {
    let stripped = message.strip_prefix("On ")?;
    let colon_idx = stripped.find(':')?;
    let branch = stripped[..colon_idx].trim();
    if branch.is_empty() {
        None
    } else {
        Some(branch.to_string())
    }
}

/// Extract the first number from a string.
fn extract_leading_number(s: &str) -> Option<usize> {
    let num_str: String = s
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    num_str.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_stash_list() {
        let output = "stash@{0}\x1fOn main: WIP changes\nstash@{1}\x1f!!Gitru<feature> -> <main>\n";
        let entries = parse_stash_list(output).unwrap();
        assert_eq!(entries.len(), 2);

        assert_eq!(entries[0].index, 0);
        assert_eq!(entries[0].reference, "stash@{0}");
        assert_eq!(entries[0].message, "On main: WIP changes");
        assert_eq!(entries[0].branch, Some("main".to_string()));
        assert!(!entries[0].is_gitru);

        assert_eq!(entries[1].index, 1);
        assert!(entries[1].is_gitru);
    }

    #[test]
    fn test_parse_stash_stat() {
        let output = " src/main.rs | 10 ++++----\n src/lib.rs  |  5 ++---\n 2 files changed, 7 insertions(+), 8 deletions(-)\n";
        let stat = parse_stash_stat(output, "stash@{0}").unwrap();
        assert_eq!(stat.files_changed, 2);
        assert_eq!(stat.insertions, 7);
        assert_eq!(stat.deletions, 8);
    }

    #[test]
    fn test_parse_gitru_stash_message() {
        let msg = "!!Gitru<feature-branch> -> <main>";
        let result = parse_gitru_stash_message(msg).unwrap();
        assert_eq!(result.0, "feature-branch");
        assert_eq!(result.1, "main");
    }

    #[test]
    fn test_parse_gitru_stash_message_case_insensitive() {
        // Also matches legacy !!gitru pattern (case-insensitive)
        let msg = "!!gitru<old-branch> -> <new-branch>";
        let result = parse_gitru_stash_message(msg).unwrap();
        assert_eq!(result.0, "old-branch");
        assert_eq!(result.1, "new-branch");
    }

    #[test]
    fn test_validate_stash_ref() {
        assert!(validate_stash_ref("stash@{0}").is_ok());
        assert!(validate_stash_ref("stash@{42}").is_ok());
        assert!(validate_stash_ref("stash@{abc}").is_err());
        assert!(validate_stash_ref("refs/stash").is_err());
        assert!(validate_stash_ref("stash@{-1}").is_err());
    }

    #[test]
    fn test_branch_name_matches() {
        assert!(branch_name_matches("main", "main"));
        assert!(branch_name_matches("origin/main", "main"));
        assert!(branch_name_matches("main", "origin/main"));
        assert!(!branch_name_matches("feature", "main"));
    }

    #[test]
    fn test_parse_stash_file_status() {
        // With -z, git outputs alternating NUL-separated fields: status\0path\0
        let output = b"M\0file1.rs\0A\0file2.rs\0D\0file3.rs\0";
        let files = parse_stash_file_status(output).unwrap();
        assert_eq!(files.len(), 3);
        assert_eq!(files[0].path, "file1.rs");
        assert!(matches!(files[0].status[0], FileStatusKind::IndexModified));
        assert_eq!(files[1].path, "file2.rs");
        assert!(matches!(files[1].status[0], FileStatusKind::IndexNew));
        assert_eq!(files[2].path, "file3.rs");
        assert!(matches!(files[2].status[0], FileStatusKind::IndexDeleted));
    }

    #[test]
    fn test_parse_stash_file_status_rename() {
        // Rename: R100\0old_path\0new_path\0
        let output = b"R100\0old_name.rs\0new_name.rs\0M\0other.rs\0";
        let files = parse_stash_file_status(output).unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, "old_name.rs");
        assert_eq!(files[0].new_path, Some("new_name.rs".to_string()));
        assert!(matches!(files[0].status[0], FileStatusKind::IndexRenamed));
        assert_eq!(files[1].path, "other.rs");
        assert!(matches!(files[1].status[0], FileStatusKind::IndexModified));
    }
}
