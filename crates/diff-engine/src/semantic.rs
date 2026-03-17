use std::collections::BTreeMap;

use tree_sitter::{Language, Node, Parser};

use crate::models::SemanticChange;

#[derive(Debug, Clone)]
struct Entity {
    text: String,
    ordinal: usize,
    start_line: u32,
    end_line: u32,
}

// ── Language detection ────────────────────────────────────────────

fn detect_language(file_path: &str) -> Option<Language> {
    let ext = std::path::Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())?;

    match ext {
        "ts" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        "tsx" => Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
        "rs" => Some(tree_sitter_rust::LANGUAGE.into()),
        _ => None,
    }
}

// ── Top-level named entity extraction ────────────────────────────

/// Node kinds that represent a named, top-level declaration we can
/// track across old/new trees.
fn is_tracked_node(kind: &str) -> bool {
    matches!(
        kind,
        // TypeScript / JavaScript
        "function_declaration"
            | "class_declaration"
            | "interface_declaration"
            | "type_alias_declaration"
            | "export_statement"
            | "lexical_declaration"
            | "variable_declarator"
            | "method_definition"
            // Rust
            | "function_item"
            | "impl_item"
            | "struct_item"
            | "enum_item"
            | "trait_item"
            | "mod_item"
    )
}

fn is_name_node(kind: &str) -> bool {
    matches!(kind, "identifier" | "type_identifier" | "name")
}

/// Walk the top-level children of a tree looking for tracked nodes
/// and return their `name` (identifier child).
fn extract_named_entities(root: Node<'_>, source: &[u8]) -> BTreeMap<String, Entity> {
    let mut entities = BTreeMap::new();
    let mut ordinal = 0;
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        collect_entity(child, source, &mut entities, &mut ordinal);
    }
    entities
}

fn collect_entity(
    node: Node<'_>,
    source: &[u8],
    out: &mut BTreeMap<String, Entity>,
    ordinal: &mut usize,
) {
    if matches!(node.kind(), "export_statement" | "lexical_declaration") {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            collect_entity(child, source, out, ordinal);
        }
        return;
    }

    if !is_tracked_node(node.kind()) {
        return;
    }

    if let Some(name) = extract_node_name(node, source) {
        let text = node
            .utf8_text(source)
            .map(|value| value.to_string())
            .unwrap_or_default();
        let start_line = node.start_position().row as u32 + 1;
        let end_line = node.end_position().row as u32 + 1;
        out.insert(
            name,
            Entity {
                text,
                ordinal: *ordinal,
                start_line,
                end_line,
            },
        );
        *ordinal += 1;
    }
}

fn extract_node_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if is_name_node(child.kind()) {
            if let Ok(text) = child.utf8_text(source) {
                return Some(text.to_string());
            }
        }

        if node.kind() == "impl_item" && child.kind() == "type_identifier" {
            if let Ok(text) = child.utf8_text(source) {
                return Some(format!("impl {text}"));
            }
        }
    }

    None
}

// ── Public API ────────────────────────────────────────────────────

pub struct SemanticAnalyzer;

impl SemanticAnalyzer {
    /// Compare the structural signatures of `old` and `new` for
    /// `file_path` and return a list of high-level changes.
    /// Returns an empty list for unsupported file types or parse errors.
    pub fn analyze(old: &str, new: &str, file_path: &str) -> Vec<SemanticChange> {
        let language = match detect_language(file_path) {
            Some(l) => l,
            None => return vec![],
        };

        let mut parser = Parser::new();
        if parser.set_language(&language).is_err() {
            return vec![];
        }

        let old_tree = match parser.parse(old, None) {
            Some(t) => t,
            None => return vec![],
        };

        // Reset parser state between parses (same language, safe to reuse).
        let new_tree = match parser.parse(new, None) {
            Some(t) => t,
            None => return vec![],
        };

        let old_entities = extract_named_entities(old_tree.root_node(), old.as_bytes());
        let new_entities = extract_named_entities(new_tree.root_node(), new.as_bytes());

        let mut changes = Vec::new();

        for (name, old_entity) in &old_entities {
            match new_entities.get(name) {
                None => {
                    changes.push(SemanticChange {
                        kind: "removed".to_string(),
                        name: name.clone(),
                        old_start_line: Some(old_entity.start_line),
                        old_end_line: Some(old_entity.end_line),
                        new_start_line: None,
                        new_end_line: None,
                    });
                }
                Some(new_entity) if old_entity.text != new_entity.text => {
                    changes.push(SemanticChange {
                        kind: "modified".to_string(),
                        name: name.clone(),
                        old_start_line: Some(old_entity.start_line),
                        old_end_line: Some(old_entity.end_line),
                        new_start_line: Some(new_entity.start_line),
                        new_end_line: Some(new_entity.end_line),
                    });
                }
                Some(new_entity) if old_entity.ordinal != new_entity.ordinal => {
                    changes.push(SemanticChange {
                        kind: "moved".to_string(),
                        name: name.clone(),
                        old_start_line: Some(old_entity.start_line),
                        old_end_line: Some(old_entity.end_line),
                        new_start_line: Some(new_entity.start_line),
                        new_end_line: Some(new_entity.end_line),
                    });
                }
                Some(_) => {}
            }
        }

        for name in new_entities.keys() {
            if !old_entities.contains_key(name) {
                let new_entity = &new_entities[name];
                changes.push(SemanticChange {
                    kind: "added".to_string(),
                    name: name.clone(),
                    old_start_line: None,
                    old_end_line: None,
                    new_start_line: Some(new_entity.start_line),
                    new_end_line: Some(new_entity.end_line),
                });
            }
        }

        changes
    }
}

#[cfg(test)]
mod tests {
    use super::SemanticAnalyzer;

    #[test]
    fn detects_modified_exported_typescript_function() {
        let old = "export async function requestDiff() {\n  return 1;\n}\n";
        let new = "export async function requestDiff() {\n  return 2;\n}\n";

        let changes = SemanticAnalyzer::analyze(old, new, "commands.ts");

        assert!(
            changes
                .iter()
                .any(|change| change.kind == "modified" && change.name == "requestDiff")
        );
    }

    #[test]
    fn detects_added_rust_function() {
        let old = "fn existing() {}\n";
        let new = "fn existing() {}\nfn fresh() {}\n";

        let changes = SemanticAnalyzer::analyze(old, new, "lib.rs");

        assert!(
            changes
                .iter()
                .any(|change| change.kind == "added" && change.name == "fresh")
        );
    }

    #[test]
    fn detects_modified_exported_typescript_const_schema() {
        let old =
            "export const RequestDiffParamsSchema = z.object({\n  filePath: z.string(),\n});\n";
        let new = "export const RequestDiffParamsSchema = z.object({\n  filePath: z.string(),\n  mode: z.string().optional(),\n});\n";

        let changes = SemanticAnalyzer::analyze(old, new, "types.ts");

        assert!(changes.iter().any(|change| {
            change.kind == "modified" && change.name == "RequestDiffParamsSchema"
        }));
    }

    #[test]
    fn detects_modified_tsx_component_arrow_function() {
        let old = "export const RouteComponent = () => {\n  return <div>old</div>;\n};\n";
        let new = "export const RouteComponent = () => {\n  return <div>new</div>;\n};\n";

        let changes = SemanticAnalyzer::analyze(old, new, "index.tsx");

        assert!(
            changes
                .iter()
                .any(|change| { change.kind == "modified" && change.name == "RouteComponent" })
        );
    }

    #[test]
    fn detects_moved_typescript_function() {
        let old = "export function alpha() {}\nexport function beta() {}\n";
        let new = "export function beta() {}\nexport function alpha() {}\n";

        let changes = SemanticAnalyzer::analyze(old, new, "commands.ts");

        assert!(
            changes
                .iter()
                .any(|change| change.kind == "moved" && change.name == "alpha")
        );
        assert!(
            changes
                .iter()
                .any(|change| change.kind == "moved" && change.name == "beta")
        );
    }

    #[test]
    fn detects_moved_rust_function() {
        let old = "fn first() {}\nfn second() {}\n";
        let new = "fn second() {}\nfn first() {}\n";

        let changes = SemanticAnalyzer::analyze(old, new, "lib.rs");

        assert!(
            changes
                .iter()
                .any(|change| change.kind == "moved" && change.name == "first")
        );
        assert!(
            changes
                .iter()
                .any(|change| change.kind == "moved" && change.name == "second")
        );
    }
}
