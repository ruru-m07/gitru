use similar::{ChangeTag, TextDiff};

use crate::models::{DiffLine, HighlightedLine, Hunk, LineKind};

pub fn build_hunks(
    old_text: &str,
    new_text: &str,
    old_lines: &[HighlightedLine],
    new_lines: &[HighlightedLine],
    context: usize,
) -> Vec<Hunk> {
    let diff = TextDiff::from_lines(old_text, new_text);
    let mut hunks = Vec::new();

    for group in diff.grouped_ops(context) {
        let mut lines: Vec<DiffLine> = Vec::new();
        let first_op = &group[0];

        let old_start = first_op.old_range().start as u32 + 1;
        let old_len: u32 = group.iter().map(|op| op.old_range().len() as u32).sum();
        let new_start = first_op.new_range().start as u32 + 1;
        let new_len: u32 = group.iter().map(|op| op.new_range().len() as u32).sum();

        let header = format!(
            "@@ -{},{} +{},{} @@",
            old_start, old_len, new_start, new_len
        );

        for op in &group {
            for change in diff.iter_changes(op) {
                let (kind, old_lineno, new_lineno) = match change.tag() {
                    ChangeTag::Equal => (
                        LineKind::Context,
                        change.old_index().map(|value| value as u32 + 1),
                        change.new_index().map(|value| value as u32 + 1),
                    ),
                    ChangeTag::Delete => (
                        LineKind::Removed,
                        change.old_index().map(|value| value as u32 + 1),
                        None,
                    ),
                    ChangeTag::Insert => (
                        LineKind::Added,
                        None,
                        change.new_index().map(|value| value as u32 + 1),
                    ),
                };

                let old_line = old_lineno.and_then(|line_no| get_line(old_lines, line_no));
                let new_line = new_lineno.and_then(|line_no| get_line(new_lines, line_no));
                let content = change.value().trim_end_matches(['\r', '\n']).to_string();

                lines.push(DiffLine {
                    kind,
                    old_lineno,
                    new_lineno,
                    content,
                    old_content: old_line.map(|line| line.content.clone()),
                    new_content: new_line.map(|line| line.content.clone()),
                    old_tokens: old_line.map(|line| line.tokens.clone()).unwrap_or_default(),
                    new_tokens: new_line.map(|line| line.tokens.clone()).unwrap_or_default(),
                });
            }
        }

        hunks.push(Hunk {
            header,
            old_start,
            new_start,
            old_lines: old_len,
            new_lines: new_len,
            lines,
        });
    }

    hunks
}

fn get_line(lines: &[HighlightedLine], line_no: u32) -> Option<&HighlightedLine> {
    let index = usize::try_from(line_no.saturating_sub(1)).ok()?;
    lines.get(index)
}
