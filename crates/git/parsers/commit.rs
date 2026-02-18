use crate::models::commit::{Author, CommitAuthors, CommitInfo, CommitStats};

pub const COMMIT_STANDARD_FORMAT: &str = concat!(
    "%H%x1f",  // ! commit hash
    "%an%x1f", // ! author name
    "%ae%x1f", // ! author email
    "%at%x1f", // ! author timestamp
    "%cn%x1f", // ! committer name
    "%ce%x1f", // ! committer email
    "%ct%x1f", // ! committer timestamp
    "%s%x1f",  // ! subject
    "%b%x1e"   // ! body + record separator
);

pub fn parse_commit_record(record: &str) -> Result<CommitInfo, String> {
    let parts: Vec<&str> = record.split('\u{001f}').collect();
    if parts.len() < 9 {
        return Err("Invalid commit record".to_string());
    }

    let summary = parts[7].to_string();
    let body = parts[8].to_string();
    let message = if body.trim().is_empty() {
        summary.clone()
    } else {
        format!("{summary}\n\n{body}")
    };

    let authors = build_commit_authors(parts[1], parts[2], parts[4], parts[5], &message);

    Ok(CommitInfo {
        id: parts[0].to_string(),
        timestamp: parts[6].parse::<i64>().unwrap_or(0),
        summary,
        body,
        authors,
    })
}

pub fn build_commit_authors(
    author_name: &str,
    author_email: &str,
    committer_name: &str,
    committer_email: &str,
    message: &str,
) -> CommitAuthors {
    let author = Author {
        name: author_name.to_string(),
        email: normalize_email(author_email),
    };

    let committer = Author {
        name: committer_name.to_string(),
        email: normalize_email(committer_email),
    };

    let co_authors = extract_co_authors(message);

    CommitAuthors {
        author,
        committer,
        co_authors,
    }
}

pub fn parse_shortstat(output: &str) -> CommitStats {
    let mut files_changed = 0;
    let mut insertions = 0;
    let mut deletions = 0;

    for token in output.split(',') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }

        if token.contains("file changed") || token.contains("files changed") {
            files_changed = token
                .split_whitespace()
                .next()
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(0);
        } else if token.contains("insertion") {
            insertions = token
                .split_whitespace()
                .next()
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(0);
        } else if token.contains("deletion") {
            deletions = token
                .split_whitespace()
                .next()
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(0);
        }
    }

    CommitStats {
        insertions,
        deletions,
        files_changed,
    }
}

pub fn normalize_email(email: &str) -> String {
    let trimmed = email.trim();
    let trimmed = trimmed.strip_prefix('<').unwrap_or(trimmed);
    let trimmed = trimmed.strip_suffix('>').unwrap_or(trimmed);
    trimmed.trim().to_string()
}

pub fn extract_co_authors(message: &str) -> Vec<Author> {
    let mut co_authors = Vec::new();

    for line in message.lines() {
        let line = line.trim();
        if line.starts_with("Co-authored-by:") || line.starts_with("Co-Authored-By:") {
            if let Some(author) = parse_author_line(line) {
                co_authors.push(author);
            }
        }
    }

    co_authors
}

pub fn parse_author_line(line: &str) -> Option<Author> {
    let line = line.split(':').nth(1)?.trim();

    if let Some(email_start) = line.rfind('<') {
        if let Some(email_end) = line.rfind('>') {
            let name = line[..email_start].trim().to_string();
            let email = line[email_start + 1..email_end].trim().to_string();

            return Some(Author { name, email });
        }
    }

    Some(Author {
        name: line.to_string(),
        email: String::new(),
    })
}
