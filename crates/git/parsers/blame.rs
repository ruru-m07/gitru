use crate::models::diff::BlameInfo;

#[derive(Default)]
struct BlameEntryBuilder {
    commit: String,
    original_line: usize,
    final_line: usize,
    author: String,
    author_mail: String,
    author_time: String,
    author_tz: String,
    committer: String,
    committer_mail: String,
    committer_time: String,
    committer_tz: String,
    summary: String,
    previous: String,
    filename: String,
    content: String,
}

impl BlameEntryBuilder {
    fn from_header(line: &str) -> Result<Self, String> {
        let mut parts = line.split_whitespace();
        let commit = parts
            .next()
            .ok_or_else(|| format!("Invalid blame header '{line}'"))?;
        let original_line = parts
            .next()
            .ok_or_else(|| format!("Invalid blame header '{line}'"))?
            .parse::<usize>()
            .map_err(|_| format!("Invalid original line in blame header '{line}'"))?;
        let final_line = parts
            .next()
            .ok_or_else(|| format!("Invalid blame header '{line}'"))?
            .parse::<usize>()
            .map_err(|_| format!("Invalid final line in blame header '{line}'"))?;

        Ok(Self {
            commit: commit.trim_start_matches('^').to_string(),
            original_line,
            final_line,
            ..Self::default()
        })
    }

    fn build(self) -> BlameInfo {
        BlameInfo::new(
            self.commit,
            self.original_line,
            self.final_line,
            self.author,
            self.author_mail,
            self.author_time,
            self.author_tz,
            self.committer,
            self.committer_mail,
            self.committer_time,
            self.committer_tz,
            self.summary,
            self.previous,
            self.filename,
            self.content,
        )
    }
}

pub fn parse_blame_porcelain(output: &str) -> Result<Vec<BlameInfo>, String> {
    if output.trim().is_empty() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    let mut current: Option<BlameEntryBuilder> = None;

    for line in output.lines() {
        if is_blame_header(line) {
            if let Some(prev) = current.take() {
                entries.push(prev.build());
            }
            current = Some(BlameEntryBuilder::from_header(line)?);
            continue;
        }

        if let Some(builder) = current.as_mut() {
            if let Some(content) = line.strip_prefix('\t') {
                builder.content = content.to_string();
                continue;
            }

            if let Some((key, value)) = line.split_once(' ') {
                match key {
                    "author" => builder.author = value.to_string(),
                    "author-mail" => builder.author_mail = normalize_email(value),
                    "author-time" => builder.author_time = value.to_string(),
                    "author-tz" => builder.author_tz = value.to_string(),
                    "committer" => builder.committer = value.to_string(),
                    "committer-mail" => builder.committer_mail = normalize_email(value),
                    "committer-time" => builder.committer_time = value.to_string(),
                    "committer-tz" => builder.committer_tz = value.to_string(),
                    "summary" => builder.summary = value.to_string(),
                    "previous" => builder.previous = value.to_string(),
                    "filename" => builder.filename = value.to_string(),
                    _ => {}
                }
            }
        }
    }

    if let Some(prev) = current.take() {
        entries.push(prev.build());
    }

    Ok(entries)
}

fn normalize_email(value: &str) -> String {
    value
        .trim()
        .trim_start_matches('<')
        .trim_end_matches('>')
        .to_string()
}

fn is_blame_header(line: &str) -> bool {
    let mut parts = line.split_whitespace();
    let Some(commit) = parts.next() else {
        return false;
    };
    let Some(original_line) = parts.next() else {
        return false;
    };
    let Some(final_line) = parts.next() else {
        return false;
    };

    let commit_hex = commit.strip_prefix('^').unwrap_or(commit);
    if commit_hex.len() != 40 || !commit_hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return false;
    }

    original_line.chars().all(|ch| ch.is_ascii_digit())
        && final_line.chars().all(|ch| ch.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::parse_blame_porcelain;

    #[test]
    fn parses_basic_porcelain_output() {
        let input = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 1 1 1\nauthor Test\nauthor-mail <test@example.com>\nauthor-time 1\nauthor-tz +0000\ncommitter Test\ncommitter-mail <test@example.com>\ncommitter-time 1\ncommitter-tz +0000\nsummary init\nprevious bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb src/lib.rs\nfilename src/lib.rs\n\tlet x = 1;\n";

        let parsed = parse_blame_porcelain(input).expect("should parse");
        assert_eq!(parsed.len(), 1);
    }

    #[test]
    fn parses_entries_without_previous() {
        let input = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 2 3 1\nauthor Test\nauthor-mail <test@example.com>\nauthor-time 1\nauthor-tz +0000\ncommitter Test\ncommitter-mail <test@example.com>\ncommitter-time 1\ncommitter-tz +0000\nsummary init\nfilename src/lib.rs\n\t\n";

        let parsed = parse_blame_porcelain(input).expect("should parse");
        assert_eq!(parsed.len(), 1);
    }

    #[test]
    fn strips_angle_brackets_from_email_fields() {
        let input = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 2 3 1\nauthor Test\nauthor-mail <test@example.com>\nauthor-time 1\nauthor-tz +0000\ncommitter Test\ncommitter-mail <committer@example.com>\ncommitter-time 1\ncommitter-tz +0000\nsummary init\nfilename src/lib.rs\n\tlet y = 2;\n";

        let parsed = parse_blame_porcelain(input).expect("should parse");
        let debug = format!("{:?}", parsed);

        assert!(debug.contains("author_mail: \"test@example.com\""));
        assert!(debug.contains("committer_mail: \"committer@example.com\""));
        assert!(!debug.contains("<test@example.com>"));
        assert!(!debug.contains("<committer@example.com>"));
    }

    #[test]
    fn rejects_invalid_header() {
        let input = "not-a-hash 1 1 1\nauthor Test\n\tcontent\n";
        let parsed = parse_blame_porcelain(input).expect("should not fail, no entries");
        assert!(parsed.is_empty());
    }
}
