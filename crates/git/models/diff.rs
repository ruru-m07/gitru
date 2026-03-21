use serde::Serialize;

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

#[derive(Debug, Serialize, Clone)]
pub struct BlameInfo {
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

impl BlameInfo {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
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
    ) -> Self {
        Self {
            commit,
            original_line,
            final_line,
            author,
            author_mail,
            author_time,
            author_tz,
            committer,
            committer_mail,
            committer_time,
            committer_tz,
            summary,
            previous,
            filename,
            content,
        }
    }
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

#[derive(Serialize)]
pub struct BlameDiff {
    #[serde(rename = "oldBlame")]
    pub old_blame: Option<Vec<BlameInfo>>,
    #[serde(rename = "newBlame")]
    pub new_blame: Option<Vec<BlameInfo>>,
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
