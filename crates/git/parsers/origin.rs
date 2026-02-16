pub fn parse_remote_url(
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

pub fn detect_provider(host: &String) -> Option<String> {
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
