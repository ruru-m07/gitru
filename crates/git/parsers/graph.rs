use crate::models::commit::CommitStats;
use crate::models::graph::{
    GraphLogEntry, GraphRef, GraphRefKind, GraphSearchKey, GraphSearchQuery, GraphSearchTerm,
};

pub const RECORD_SEPARATOR: char = '\u{001e}';
pub const FIELD_SEPARATOR: char = '\u{001f}';

// Record separator leads each entry so that --shortstat output (which follows the
// commit body) stays inside the same split-part as its commit, making a single
// git log call sufficient for both metadata and diff stats.
pub const LOG_FORMAT: &str =
    "%x1e%H%x1f%P%x1f%an%x1f%ae%x1f%at%x1f%cn%x1f%ce%x1f%ct%x1f%D%x1f%s%x1f%b";

pub fn parse_log_entries(output: &str) -> Vec<GraphLogEntry> {
    let mut entries = Vec::new();

    for record in output.split(RECORD_SEPARATOR) {
        let record = record.trim_matches(['\n', '\r', '\t', ' ']);
        if record.is_empty() {
            continue;
        }

        // splitn keeps everything past the 10th separator in fields[10] so that
        // the body (and any trailing shortstat block) is not split on \x1f.
        let fields: Vec<&str> = record.splitn(11, FIELD_SEPARATOR).collect();
        if fields.len() < 11 {
            continue;
        }

        let parents = parse_parent_ids(fields[1]);
        let refs = parse_refs(fields[8]);

        let author_time = fields[4].parse::<i64>().unwrap_or(0);
        let committer_time = fields[7].parse::<i64>().unwrap_or(0);

        let (body, stats) = split_body_and_stats(fields[10]);

        entries.push(GraphLogEntry {
            id: fields[0].to_string(),
            parents,
            author_name: fields[2].to_string(),
            author_email: fields[3].to_string(),
            author_time,
            committer_name: fields[5].to_string(),
            committer_email: fields[6].to_string(),
            committer_time,
            refs,
            summary: fields[9].to_string(),
            body,
            stats,
        });
    }

    entries
}

/// Separates the commit body from the optional shortstat block appended by
/// `git log --shortstat`. Shortstat lines look like:
///   "3 files changed, 10 insertions(+), 2 deletions(-)"
/// They always start with a digit and contain the word "changed".
/// Returns the body (trailing newlines stripped) and parsed stats.
fn split_body_and_stats(raw: &str) -> (String, CommitStats) {
    let lines: Vec<&str> = raw.lines().collect();
    if let Some((idx, _)) = lines
        .iter()
        .enumerate()
        .rev()
        .find(|(_, line)| is_stat_line(line.trim()))
    {
        let body = lines[..idx]
            .join("\n")
            .trim_end_matches(['\n', '\r', ' '])
            .to_string();
        let stats = parse_stat_line(lines[idx].trim());
        (body, stats)
    } else {
        (
            raw.trim_end_matches(['\n', '\r', ' ']).to_string(),
            CommitStats::default(),
        )
    }
}

fn is_stat_line(t: &str) -> bool {
    t.starts_with(|c: char| c.is_ascii_digit()) && t.contains("changed")
}

pub fn parse_stat_line(line: &str) -> CommitStats {
    let mut stats = CommitStats::default();
    for part in line.split(',') {
        let part = part.trim();
        let mut iter = part.split_whitespace();
        let count = match iter.next().and_then(|v| v.parse::<usize>().ok()) {
            Some(v) => v,
            None => continue,
        };
        let label = iter.collect::<Vec<_>>().join(" ");
        if label.starts_with("file") {
            stats.files_changed = count;
        } else if label.starts_with("insertion") {
            stats.insertions = count;
        } else if label.starts_with("deletion") {
            stats.deletions = count;
        }
    }
    stats
}

pub fn parse_search_query(input: &str) -> GraphSearchQuery {
    let mut terms = Vec::new();
    let tokens = tokenize_query(input);

    for token in tokens {
        let (negated, token) = match token.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, token.as_str()),
        };

        if let Some((key, value)) = token.split_once(':')
            && let Some(search_key) = parse_search_key(key)
            && !value.is_empty()
        {
            terms.push(GraphSearchTerm {
                key: Some(search_key),
                value: value.to_string(),
                negated,
            });
            continue;
        }

        if !token.is_empty() {
            terms.push(GraphSearchTerm {
                key: None,
                value: token.to_string(),
                negated,
            });
        }
    }

    GraphSearchQuery { terms }
}

fn tokenize_query(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for ch in input.chars() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
            }
            ' ' | '\t' | '\n' | '\r' if !in_quotes => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
            _ => current.push(ch),
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

fn parse_search_key(key: &str) -> Option<GraphSearchKey> {
    match key.to_ascii_lowercase().as_str() {
        "author" | "a" => Some(GraphSearchKey::Author),
        "committer" | "c" => Some(GraphSearchKey::Committer),
        "message" | "msg" | "m" => Some(GraphSearchKey::Message),
        "ref" | "r" => Some(GraphSearchKey::Ref),
        "id" | "hash" => Some(GraphSearchKey::Id),
        _ => None,
    }
}

fn parse_parent_ids(raw: &str) -> Vec<String> {
    raw.split_whitespace()
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .collect()
}

fn parse_refs(raw: &str) -> Vec<GraphRef> {
    if raw.trim().is_empty() {
        return Vec::new();
    }

    raw.split(',')
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(parse_ref)
        .collect()
}

fn parse_ref(raw: &str) -> GraphRef {
    if let Some(name) = raw.strip_prefix("tag: ") {
        let name = name.trim();
        return GraphRef {
            name: name.to_string(),
            display_name: display_name_for_ref(name),
            kind: GraphRefKind::Tag,
            is_head: false,
        };
    }

    if let Some(name) = raw.strip_prefix("HEAD -> ") {
        let name = name.trim();
        return GraphRef {
            name: name.to_string(),
            display_name: display_name_for_ref(name),
            kind: GraphRefKind::Local,
            is_head: true,
        };
    }

    if raw == "refs/stash" || raw.starts_with("stash@{") {
        return GraphRef {
            name: "stash".to_string(),
            display_name: "stash".to_string(),
            kind: GraphRefKind::Stash,
            is_head: false,
        };
    }

    // Classify directly from the full ref path — no secondary subprocess needed.
    let kind = if raw.starts_with("refs/remotes/") {
        GraphRefKind::Remote
    } else if raw.starts_with("refs/heads/") {
        GraphRefKind::Local
    } else if raw.starts_with("refs/tags/") {
        GraphRefKind::Tag
    } else {
        GraphRefKind::Other
    };

    GraphRef {
        name: raw.to_string(),
        display_name: display_name_for_ref(raw),
        kind,
        is_head: false,
    }
}

fn display_name_for_ref(name: &str) -> String {
    for prefix in ["refs/heads/", "refs/remotes/", "refs/tags/", "refs/stash"] {
        if let Some(stripped) = name.strip_prefix(prefix) {
            return stripped.trim_start_matches('/').to_string();
        }
    }

    name.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_head_ref_marks_current_branch() {
        let reference = parse_ref("HEAD -> refs/heads/main");

        assert_eq!(reference.name, "refs/heads/main");
        assert_eq!(reference.display_name, "main");
        assert_eq!(reference.kind, GraphRefKind::Local);
        assert!(reference.is_head);
    }

    #[test]
    fn display_names_are_normalized_for_common_refs() {
        let local = parse_ref("refs/heads/feature/foo");
        let remote = parse_ref("refs/remotes/origin/main");
        let tag = parse_ref("refs/tags/v1.2.3");
        let stash = parse_ref("stash@{0}");

        assert_eq!(local.display_name, "feature/foo");
        assert_eq!(remote.display_name, "origin/main");
        assert_eq!(tag.display_name, "v1.2.3");
        assert_eq!(stash.display_name, "stash");
    }
}
