use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A request from the frontend for a diff of a single file.
#[derive(Debug, Clone)]
pub struct DiffRequest {
    pub file_path: String,
}

/// An enqueued job — a DiffRequest with a unique identifier.
#[derive(Debug, Clone)]
pub struct DiffJob {
    pub id: Uuid,
    pub request: DiffRequest,
}

// ──────────────────────────────────────────────────────────────────
// Serialisable types — these are emitted to the frontend via Tauri
// events and must match the TypeScript types exactly (camelCase).
// ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiffStatus {
    Processing,
    Ready,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LineKind {
    Added,
    Removed,
    Context,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenSpan {
    pub content: String,
    pub color: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HighlightedLine {
    pub line_no: u32,
    pub content: String,
    pub tokens: Vec<TokenSpan>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    pub kind: LineKind,
    pub old_lineno: Option<u32>,
    pub new_lineno: Option<u32>,
    pub content: String,
    pub old_content: Option<String>,
    pub new_content: Option<String>,
    pub old_tokens: Vec<TokenSpan>,
    pub new_tokens: Vec<TokenSpan>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hunk {
    /// The conventional @@ header string, e.g. "@@ -10,6 +10,8 @@".
    pub header: String,
    pub old_start: u32,
    pub new_start: u32,
    pub old_lines: u32,
    pub new_lines: u32,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticChange {
    pub kind: String,
    pub name: String,
    pub old_start_line: Option<u32>,
    pub old_end_line: Option<u32>,
    pub new_start_line: Option<u32>,
    pub new_end_line: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffPayload {
    pub job_id: String,
    pub file_path: String,
    pub status: DiffStatus,
    pub hunks: Vec<Hunk>,
    pub semantic_changes: Vec<SemanticChange>,
    pub old_lines: Vec<HighlightedLine>,
    pub new_lines: Vec<HighlightedLine>,
}
