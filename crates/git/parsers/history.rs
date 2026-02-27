use crate::{models::commit::CommitInfo, parsers::commit::parse_commit_record};

pub fn parse_history_records(output: &str) -> Result<Vec<CommitInfo>, String> {
    let mut commits = Vec::with_capacity(100);

    for record in output.split('\u{001e}') {
        let record = record.trim_matches(['\n', '\r', '\t', ' ']);
        if record.is_empty() {
            continue;
        }
        if let Ok(commit) = parse_commit_record(record) {
            commits.push(commit);
        }
    }
    Ok(commits)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Edge case tests ──────────────────────────────────────────────

    #[test]
    fn parse_empty_history() {
        let result = parse_history_records("").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn parse_whitespace_only() {
        let result = parse_history_records("  \n\t\n  ").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn parse_malformed_record_skipped() {
        // Malformed records should be skipped, not cause errors
        let result = parse_history_records("malformed\u{001e}also_bad").unwrap();
        assert!(result.is_empty());
    }
}
