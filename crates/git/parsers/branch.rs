use crate::models::branch::BranchInfo;
use crate::models::commit::CommitInfo;
use crate::parsers::commit::build_commit_authors;

pub const BRANCH_STANDARD_FORMAT: &str = concat!(
    "%(refname:short)",
    "\u{001f}",
    "%(objectname)",
    "\u{001f}",
    "%(upstream:short)",
    "\u{001f}",
    "%(HEAD)",
    "\u{001f}",
    "%(authorname)",
    "\u{001f}",
    "%(authoremail)",
    "\u{001f}",
    "%(authordate:unix)",
    "\u{001f}",
    "%(committername)",
    "\u{001f}",
    "%(committeremail)",
    "\u{001f}",
    "%(committerdate:unix)",
    "\u{001f}",
    "%(subject)",
    "\u{001f}",
    "%(body)",
    "\u{001e}"
);

pub fn parse_branch_records(output: &str, is_remote: bool) -> Result<Vec<BranchInfo>, String> {
    let mut branches = Vec::new();

    for record in output.split('\u{001e}') {
        let record = record.trim_matches(['\n', '\r', '\t', ' ']);
        if record.is_empty() {
            continue;
        }

        let parts: Vec<&str> = record.split('\u{001f}').collect();
        if parts.len() < 11 {
            continue;
        }

        let name = parts[0].to_string();
        let _oid = parts[1];
        let upstream = parts[2].to_string();
        let is_head = !parts[3].is_empty();

        // Build CommitInfo from parts
        let body = parts.get(11).copied().unwrap_or("");
        let commit = CommitInfo {
            id: parts[1].to_string(),
            summary: parts[10].to_string(),
            body: body.to_string(),
            timestamp: parts[6].parse().unwrap_or(0),
            authors: build_commit_authors(parts[4], parts[5], parts[7], parts[8], body),
        };

        branches.push(BranchInfo {
            name: name.clone(),
            display_name: name,
            is_remote,
            is_head,
            commit,
            upstream: if upstream.is_empty() {
                None
            } else {
                Some(upstream)
            },
            ahead: None,
            behind: None,
        });
    }

    Ok(branches)
}
