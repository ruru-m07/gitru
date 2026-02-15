use crate::models::graph::{
    GraphLogEntry, GraphRef, GraphRefKind, GraphSearchKey, GraphSearchQuery, GraphSearchTerm,
};

pub const RECORD_SEPARATOR: char = '\u{001e}';
pub const FIELD_SEPARATOR: char = '\u{001f}';

pub const LOG_FORMAT: &str = "%H%x1f%P%x1f%an%x1f%ae%x1f%at%x1f%cn%x1f%ce%x1f%ct%x1f%D%x1f%s%x1f%b%x1e";

pub fn parse_log_entries(output: &str) -> Vec<GraphLogEntry> {
    let mut entries = Vec::new();

    for record in output.split(RECORD_SEPARATOR) {
        let record = record.trim_matches(['\n', '\r', '\t', ' ']);
        if record.is_empty() {
            continue;
        }

        let fields: Vec<&str> = record.split(FIELD_SEPARATOR).collect();
        if fields.len() < 11 {
            continue;
        }

        let parents = parse_parent_ids(fields[1]);
        let refs = parse_refs(fields[8]);

        let author_time = fields[4].parse::<i64>().unwrap_or(0);
        let committer_time = fields[7].parse::<i64>().unwrap_or(0);

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
            body: fields[10].to_string(),
        });
    }

    entries
}

pub fn parse_search_query(input: &str) -> GraphSearchQuery {
    let mut terms = Vec::new();
    let tokens = tokenize_query(input);

    for token in tokens {
        let (negated, token) = match token.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, token.as_str()),
        };

        if let Some((key, value)) = token.split_once(':') {
            if let Some(search_key) = parse_search_key(key) {
                if !value.is_empty() {
                    terms.push(GraphSearchTerm {
                        key: Some(search_key),
                        value: value.to_string(),
                        negated,
                    });
                    continue;
                }
            }
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
        return GraphRef {
            name: name.trim().to_string(),
            kind: GraphRefKind::Tag,
            is_head: false,
        };
    }

    if let Some(name) = raw.strip_prefix("HEAD -> ") {
        return GraphRef {
            name: name.trim().to_string(),
            kind: GraphRefKind::Local,
            is_head: true,
        };
    }

    if raw == "refs/stash" || raw.starts_with("stash@{") {
        return GraphRef {
            name: "stash".to_string(),
            kind: GraphRefKind::Stash,
            is_head: false,
        };
    }

    GraphRef {
        name: raw.to_string(),
        kind: GraphRefKind::Other,
        is_head: false,
    }
}