use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct RepositoryOrigin {
    pub remote_name: String,
    pub remote_url: String,

    pub host: Option<String>,     // github.com
    pub provider: Option<String>, // github | gitlab | bitbucket | unknown
    pub owner: Option<String>,    // user or org
    pub repo: Option<String>,     // repo name

    pub protocol: String,
}
