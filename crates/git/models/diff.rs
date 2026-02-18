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

#[derive(Serialize)]
pub struct FileDiff {
    pub patch: String,
}
