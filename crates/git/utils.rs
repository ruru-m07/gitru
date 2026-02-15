use crate::types_legacy::{Author, CommitAuthors};
use git2::{Commit, Repository};
use std::time;

pub fn open_repository(path: &str) -> Result<Repository, String> {
    let start = time::Instant::now();

    match Repository::open(path) {
        Ok(repo) => {
            log::info!("Opening Repository {} in {:?}", path, start.elapsed());
            Ok(repo)
        }
        Err(e) if e.code() == git2::ErrorCode::NotFound => {
            log::error!("Repository Not Found! {} - {}", path, e);
            Err(format!("Location does not exist: {}", path))
        }
        Err(e) => {
            log::error!("Failed to Open Repository: {} - {}", path, e);
            Err(format!("{}", e))
        }
    }
}

/// Extracting author, co-author and committer from the &Commit
/// Typically it's straightforward to get author and committer
/// To get the co-authors we can look for pattern in commit.message() || commit.body()
/// if line.starts_with("Co-authored-by:") then we can process to get {name} <{email}>
pub fn extract_all_authors(commit: &Commit) -> CommitAuthors {
    let author = Author {
        name: commit.author().name().unwrap_or("").to_string(),
        email: commit.author().email().unwrap_or("").to_string(),
    };

    let committer = Author {
        name: commit.committer().name().unwrap_or("").to_string(),
        email: commit.committer().email().unwrap_or("").to_string(),
    };

    let co_authors = extract_co_authors(commit);

    CommitAuthors {
        author,
        committer,
        co_authors,
    }
}

fn extract_co_authors(commit: &Commit) -> Vec<Author> {
    let mut co_authors = Vec::new();

    if let Some(message) = commit.message() {
        for line in message.lines() {
            let line = line.trim();
            if line.starts_with("Co-authored-by:") || line.starts_with("Co-Authored-By:") {
                if let Some(author) = parse_author_line(line) {
                    co_authors.push(author);
                }
            }
        }
    }

    co_authors
}

fn parse_author_line(line: &str) -> Option<Author> {
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
