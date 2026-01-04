use serde::Serialize;

use crate::utils::open_repository;

#[derive(Serialize)]
pub struct RepositoryOrigin {
    pub remote_name: String,
    pub remote_url: String,

    pub host: Option<String>,     // github.com
    pub provider: Option<String>, // github | gitlab | bitbucket | unknown
    pub owner: Option<String>,    // user or org
    pub repo: Option<String>,     // repo name

    pub protocol: String,
}

#[tauri::command]
pub fn repository_origin(repo_path: &str) -> Result<RepositoryOrigin, String> {
    let repo = open_repository(repo_path).map_err(|e| e.to_string())?;

    let remote = repo
        .find_remote("origin")
        .map_err(|_| "No origin remote found".to_string())?;

    let url = remote.url().ok_or("Origin remote has no URL")?.to_string();
    let (protocol, host, owner, repo_name, provider) = parse_remote_url(&url);

    Ok(RepositoryOrigin {
        remote_name: "origin".into(),
        remote_url: url,
        host,
        provider,
        owner,
        repo: repo_name,
        protocol,
    })
}

fn parse_remote_url(
    url: &str,
) -> (
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    // SSH: git@github.com:user/repo.git
    if let Some(rest) = url.strip_prefix("git@") {
        let mut parts = rest.split(':');
        if let (Some(host), Some(path_str)) = (parts.next(), parts.next()) {
            let host = host.to_string();
            let path = path_str.trim_end_matches(".git");

            let mut path_parts = path.split('/');
            let owner = path_parts.next().map(|s| s.to_string());
            let repo = path_parts.next().map(|s| s.to_string());

            return (
                "ssh".into(),
                Some(host.clone()),
                owner,
                repo,
                detect_provider(&host),
            );
        }
    }

    // HTTPS: https://github.com/user/repo.git
    if let Some(rest) = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
    {
        let mut parts = rest.splitn(2, '/');
        if let (Some(host), Some(path_str)) = (parts.next(), parts.next()) {
            let host = host.to_string();
            let path = path_str.trim_end_matches(".git");

            let mut path_parts = path.split('/');
            let owner = path_parts.next().map(|s| s.to_string());
            let repo = path_parts.next().map(|s| s.to_string());

            return (
                "https".into(),
                Some(host.clone()),
                owner,
                repo,
                detect_provider(&host),
            );
        }
    }

    ("unknown".into(), None, None, None, None)
}

fn detect_provider(host: &String) -> Option<String> {
    if host.contains("github") {
        Some("github".into())
    } else if host.contains("gitlab") {
        Some("gitlab".into())
    } else if host.contains("bitbucket") {
        Some("bitbucket".into())
    } else {
        Some("unknown".into())
    }
}
