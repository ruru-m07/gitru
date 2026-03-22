use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum DiffScope {
    Worktree,
    Staged,
    Unstaged,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum PatchAction {
    Stage,
    Unstage,
    Discard,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PatchRange {
    pub start: Option<usize>,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct FileVersion {
    pub content: String,
    pub encoding: Option<String>,
    pub is_binary: bool,
    pub byte_length: usize,
}

#[derive(Debug, Serialize)]
pub struct GetDiffResponse {
    pub file_path: String,
    pub head: Option<FileVersion>,
    pub workdir: Option<FileVersion>,
    pub patch: Option<String>,
}

#[derive(Serialize)]
pub struct FileDiff {
    pub patch: String,
    pub asset_diff: Option<AssetDiff>,
    #[serde(rename = "oldFile")]
    pub old_file: Option<DiffTextFile>,
    #[serde(rename = "newFile")]
    pub new_file: Option<DiffTextFile>,
}

#[derive(Debug, Serialize, Clone)]
pub struct DiffTextFile {
    pub name: String,
    pub contents: String,
    pub byte_length: usize,
    pub encoding: String,
}

#[derive(Debug, Serialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum AssetDiffKind {
    Image,
    Binary,
}

#[derive(Debug, Serialize, Clone)]
pub struct AssetDiffEntry {
    pub absolute_path: String,
    pub mime: String,
    pub bytes: usize,
    pub logical_path: String,
    pub contents_base64: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct AssetDiff {
    pub kind: AssetDiffKind,
    pub before: Option<AssetDiffEntry>,
    pub after: Option<AssetDiffEntry>,
}
