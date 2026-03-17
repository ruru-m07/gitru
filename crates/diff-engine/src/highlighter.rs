use std::{path::Path, sync::Arc, time::Instant};

use syntect::{
    easy::HighlightLines,
    highlighting::{Color, FontStyle, Style, Theme},
    parsing::{SyntaxReference, SyntaxSet},
    util::LinesWithEndings,
};

use crate::cache::SyntaxCache;
use crate::models::{HighlightedLine, TokenSpan};

pub fn highlight_text(
    file_path: &str,
    text: &str,
    syntax_set: &SyntaxSet,
    theme: &Theme,
    syntax_cache: Option<&SyntaxCache>,
) -> Result<Vec<HighlightedLine>, String> {
    if text.is_empty() {
        return Ok(vec![]);
    }

    let syntax = if let Some(cache) = syntax_cache {
        resolve_syntax_cached(file_path, syntax_set, cache)
    } else {
        Arc::new(resolve_syntax(file_path, syntax_set).to_owned())
    };

    let mut highlighter = HighlightLines::new(&syntax, theme);
    let mut highlighted_lines = Vec::new();

    let initial_highlighter = Instant::now();

    for (index, line_with_endings) in LinesWithEndings::from(text).enumerate() {
        let content = trim_line_endings(line_with_endings).to_string();
        let ranges = highlighter
            .highlight_line(line_with_endings, syntax_set)
            .map_err(|e| e.to_string())?;

        let tokens = ranges
            .into_iter()
            .filter_map(|(style, piece)| token_from_piece(style, piece))
            .collect();

        highlighted_lines.push(HighlightedLine {
            line_no: index as u32 + 1,
            content,
            tokens,
        });
    }

    let duration = initial_highlighter.elapsed();
    log::debug!("Highlighting insider taken: {:?}", duration);

    Ok(highlighted_lines)
}

/// Resolve syntax with caching support
fn resolve_syntax_cached<'a>(
    file_path: &str,
    syntax_set: &'a SyntaxSet,
    cache: &SyntaxCache,
) -> Arc<SyntaxReference> {
    // Check cache first
    if let Some(syntax) = cache.get(file_path) {
        return syntax;
    }

    // Resolve and cache
    let syntax = resolve_syntax(file_path, syntax_set).to_owned();
    cache.insert(file_path.to_string(), Arc::new(syntax));
    cache.get(file_path).unwrap()
}

fn resolve_syntax<'a>(
    file_path: &str,
    syntax_set: &'a SyntaxSet,
) -> &'a syntect::parsing::SyntaxReference {
    let file_name = Path::new(file_path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();

    // Try exact filename match first
    if let Some(syntax) = syntax_set.find_syntax_by_token(file_name) {
        return syntax;
    }

    if let Some(extension) = Path::new(file_path)
        .extension()
        .and_then(|value| value.to_str())
    {
        // Try extension match
        if let Some(syntax) = syntax_set.find_syntax_by_extension(extension) {
            return syntax;
        }

        // TypeScript / TSX are not bundled in syntect's default set — fall back
        // to JavaScript, which shares the vast majority of its grammar and gives
        // recognisable highlighting for keywords, strings, numbers and comments.
        if matches!(extension, "ts" | "tsx") {
            if let Some(syntax) = syntax_set.find_syntax_by_extension("js") {
                return syntax;
            }
        }

        // Additional fallbacks for common languages
        match extension {
            "kt" | "kts" => {
                // Kotlin - fallback to Java if Kotlin not available
                if let Some(syntax) = syntax_set.find_syntax_by_extension("java") {
                    return syntax;
                }
            }
            "swift" => {
                // Swift - fallback to C++ for similar syntax
                if let Some(syntax) = syntax_set.find_syntax_by_extension("cpp") {
                    return syntax;
                }
            }
            "cs" => {
                // C# - fallback to Java for similar syntax
                if let Some(syntax) = syntax_set.find_syntax_by_extension("java") {
                    return syntax;
                }
            }
            "py" | "pyw" => {
                // Python is typically bundled, so this should always work
                if let Some(syntax) = syntax_set.find_syntax_by_extension("py") {
                    return syntax;
                }
            }
            "go" => {
                // Go - fallback to C if not available
                if let Some(syntax) = syntax_set.find_syntax_by_extension("c") {
                    return syntax;
                }
            }
            "rb" | "erb" => {
                // Ruby - fallback to Python
                if let Some(syntax) = syntax_set.find_syntax_by_extension("rb") {
                    return syntax;
                }
                if let Some(syntax) = syntax_set.find_syntax_by_extension("py") {
                    return syntax;
                }
            }
            _ => {}
        }
    }

    syntax_set.find_syntax_plain_text()
}

fn token_from_piece(style: Style, piece: &str) -> Option<TokenSpan> {
    let content = trim_line_endings(piece).to_string();
    if content.is_empty() {
        return None;
    }

    Some(TokenSpan {
        content,
        color: color_to_hex(style.foreground),
        bold: style.font_style.contains(FontStyle::BOLD),
        italic: style.font_style.contains(FontStyle::ITALIC),
        underline: style.font_style.contains(FontStyle::UNDERLINE),
    })
}

fn trim_line_endings(value: &str) -> &str {
    value.trim_end_matches(['\r', '\n'])
}

fn color_to_hex(color: Color) -> String {
    format!("#{:02X}{:02X}{:02X}", color.r, color.g, color.b)
}
