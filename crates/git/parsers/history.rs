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
