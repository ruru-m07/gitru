use crate::models::{
    commit::{Author, CommitAuthors, CommitStats, FullCommitInfo},
    pickaxe::PickaxeHit,
};

const COMMIT_HEADER_LINE_COUNT: usize = 5;

#[derive(Debug, Default, Clone)]
pub struct CommitHeader {
    pub hash: String,
    pub author_name: String,
    pub author_email: String,
    pub commit_time: i64,
    pub subject: String,
}

#[derive(Debug, Default)]
struct PickaxeParser {
    header: CommitHeader,
    header_lines: Vec<String>,
    patch_lines: Vec<String>,
    current_file_path: Option<String>,
    current_file_new_path: Option<String>,
    in_patch: bool,
}

impl PickaxeParser {
    fn parse_header_line(&mut self, line: &str) {
        self.header_lines.push(line.to_string());
        if self.header_lines.len() > COMMIT_HEADER_LINE_COUNT {
            self.header_lines.remove(0);
        }

        if self.header_lines.len() < COMMIT_HEADER_LINE_COUNT {
            return;
        }

        let hash = self.header_lines[0].trim();
        if hash.len() < 7 {
            return;
        }

        self.header.hash = hash.to_string();
        self.header.author_name = self.header_lines[1].trim().to_string();
        self.header.author_email = self.header_lines[2].trim().to_string();
        self.header.commit_time = self.header_lines[3].trim().parse().unwrap_or(0);
        self.header.subject = self.header_lines[4].trim().to_string();
    }

    fn flush_patch(&mut self) -> Option<PickaxeHit> {
        if self.header.hash.is_empty() || self.current_file_path.is_none() {
            self.patch_lines.clear();
            return None;
        }

        let patch = if self.patch_lines.is_empty() {
            None
        } else {
            Some(self.patch_lines.join("\n"))
        };

        let hit = PickaxeHit {
            commit: build_commit_info(&self.header),
            commit_hash: self.header.hash.clone(),
            commit_subject: self.header.subject.clone(),
            author_name: self.header.author_name.clone(),
            author_email: self.header.author_email.clone(),
            commit_time: self.header.commit_time,
            file_path: self.current_file_path.clone().unwrap_or_default(),
            file_new_path: self.current_file_new_path.clone(),
            match_line: None,
            patch,
        };

        self.patch_lines.clear();
        Some(hit)
    }

    fn handle_diff_header(&mut self, line: &str) -> Option<PickaxeHit> {
        let flushed = self.flush_patch();
        self.in_patch = true;

        if let Some((old_path, new_path)) = parse_diff_git_paths(line) {
            self.current_file_path = Some(old_path);
            self.current_file_new_path = new_path;
        }

        self.patch_lines.push(line.to_string());
        flushed
    }

    fn push_line(&mut self, line: &str) -> Option<PickaxeHit> {
        if is_commit_hash_line(line) && !self.in_patch && self.header.hash != line.trim() {
            self.header_lines.clear();
            self.parse_header_line(line);
            return None;
        }

        if is_commit_hash_line(line) && self.in_patch {
            let flushed = self.flush_patch();
            self.in_patch = false;
            self.header_lines.clear();
            self.parse_header_line(line);
            return flushed;
        }

        if line.starts_with("diff --git ") {
            return self.handle_diff_header(line);
        }

        if self.in_patch {
            self.patch_lines.push(line.to_string());
            return None;
        }

        if line.trim().is_empty() {
            return None;
        }

        self.parse_header_line(line);
        None
    }

    fn finish(&mut self) -> Option<PickaxeHit> {
        self.flush_patch()
    }
}

pub fn parse_diff_git_paths(line: &str) -> Option<(String, Option<String>)> {
    let rest = line.strip_prefix("diff --git ")?;
    let mut parts = rest.split_whitespace();
    let old_raw = parts.next()?;
    let new_raw = parts.next()?;

    let old_path = strip_diff_path_prefix(old_raw);
    let new_path = strip_diff_path_prefix(new_raw);

    let new_path_option = if new_path == old_path {
        None
    } else {
        Some(new_path)
    };

    Some((old_path, new_path_option))
}

fn is_commit_hash_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.len() == 40 && trimmed.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn build_commit_info(header: &CommitHeader) -> FullCommitInfo {
    FullCommitInfo {
        id: header.hash.clone(),
        timestamp: header.commit_time,
        summary: header.subject.clone(),
        body: String::new(),
        authors: CommitAuthors {
            author: Author {
                name: header.author_name.clone(),
                email: header.author_email.clone(),
            },
            committer: Author {
                name: header.author_name.clone(),
                email: header.author_email.clone(),
            },
            co_authors: vec![],
        },
        stats: CommitStats::default(),
        files: vec![],
    }
}

fn strip_diff_path_prefix(value: &str) -> String {
    value
        .strip_prefix("a/")
        .or_else(|| value.strip_prefix("b/"))
        .unwrap_or(value)
        .to_string()
}

pub struct PickaxeStreamParser {
    parser: PickaxeParser,
}

impl PickaxeStreamParser {
    pub fn new() -> Self {
        Self {
            parser: PickaxeParser::default(),
        }
    }

    pub fn push_line(&mut self, line: &str) -> Vec<PickaxeHit> {
        let mut hits = Vec::new();

        if let Some(hit) = self.parser.push_line(line) {
            hits.push(hit);
        }

        hits
    }

    pub fn finish(&mut self) -> Vec<PickaxeHit> {
        let mut hits = Vec::new();
        if let Some(hit) = self.parser.finish() {
            hits.push(hit);
        }
        hits
    }
}

pub fn parse_grep_hit_line(
    line: &str,
    commit_hash: &str,
    header: &CommitHeader,
) -> Option<PickaxeHit> {
    let (file_path, line_number) = parse_grep_output_line(line)?;
    Some(PickaxeHit {
        commit: build_commit_info(header),
        commit_hash: commit_hash.to_string(),
        commit_subject: header.subject.clone(),
        author_name: header.author_name.clone(),
        author_email: header.author_email.clone(),
        commit_time: header.commit_time,
        file_path,
        file_new_path: None,
        match_line: Some(line_number),
        patch: None,
    })
}

fn parse_grep_output_line(line: &str) -> Option<(String, u32)> {
    let mut parts = line.splitn(3, ':');
    let file_path = parts.next()?.to_string();
    let line_number = parts.next()?.parse().ok()?;
    Some((file_path, line_number))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_diff_git_paths_extracts_renamed_file() {
        let paths = parse_diff_git_paths("diff --git a/old.ts b/new.ts").unwrap();
        assert_eq!(paths.0, "old.ts");
        assert_eq!(paths.1, Some("new.ts".to_string()));
    }

    #[test]
    fn pickaxe_parser_emits_hit_for_commit_block() {
        let sample = r"abc123def4567890123456789012345678901234
Alice
alice@example.com
1700000000
Add calculateTotal

diff --git a/src/math.ts b/src/math.ts
index 1111111..2222222 100644
--- a/src/math.ts
+++ b/src/math.ts
@@ -1,3 +1,7 @@
+function calculateTotal() {
+  return 0;
+}
";

        let mut parser = PickaxeStreamParser::new();
        let mut hits = Vec::new();
        for line in sample.lines() {
            hits.extend(parser.push_line(line));
        }
        hits.extend(parser.finish());

        assert_eq!(hits.len(), 1);
        assert_eq!(
            hits[0].commit_hash,
            "abc123def4567890123456789012345678901234"
        );
        assert_eq!(hits[0].commit.id, hits[0].commit_hash);
        assert_eq!(hits[0].commit.summary, "Add calculateTotal");
        assert_eq!(hits[0].commit.authors.author.name, "Alice");
        assert_eq!(hits[0].file_path, "src/math.ts");
        assert!(hits[0].patch.as_ref().unwrap().contains("calculateTotal"));
    }
}
