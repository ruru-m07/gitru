use git2::{Commit, Repository};
use serde::Serialize;
use std::time::Instant;

#[derive(Serialize, Clone, Debug)]
pub struct Author {
    pub name: String,
    pub email: String,
}

#[derive(Serialize, Clone)]
pub struct CommitAuthors {
    pub author: Author,
    pub committer: Author,
    pub co_authors: Vec<Author>,
}

#[derive(Serialize, Clone)]
pub struct CommitInfo {
    pub id: String,
    pub summary: String,
    pub body: String,
    pub timestamp: i64,
    pub authors: CommitAuthors,
}

#[tauri::command]
pub fn history(repo_path: &str, skip: usize, limit: usize) -> Result<Vec<CommitInfo>, String> {
    let start_time = Instant::now();

    let repo = Repository::open(repo_path).map_err(|e| e.to_string())?;

    let mut revwalk = repo.revwalk().map_err(|e| e.to_string())?;

    revwalk.push_head().unwrap();

    let mut commits = Vec::with_capacity(limit);

    for oid in revwalk.skip(skip).take(limit) {
        let oid = oid.map_err(|e| e.to_string())?;
        let commit = repo.find_commit(oid).map_err(|e| e.to_string())?;

        commits.push(CommitInfo {
            id: oid.to_string(),
            summary: commit.summary().unwrap_or("").to_string(),
            body: commit.body().unwrap_or("").to_string(),
            timestamp: commit.time().seconds(),
            authors: extract_all_authors(&commit),
        });
    }

    let end_time = Instant::now();
    let duration = end_time.duration_since(start_time);
    println!("History function took: {:?}", duration);

    Ok(commits)
}

/// extracting auther, coAuther and commiter from the &Commit
/// typecaly it;s streate formward to get auther and committer
/// to get the get co-authers we can look for patter in commit.message() || commit.body()
/// if line.starts_with("Co-authored-by:") then we can procsess to get {name} <{email}>
fn extract_all_authors(commit: &Commit) -> CommitAuthors {
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
