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

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_remote_url SSH tests ───────────────────────────────────

    #[test]
    fn parse_ssh_github_url() {
        let (protocol, host, owner, repo, provider) =
            parse_remote_url("git@github.com:user/repo.git");

        assert_eq!(protocol, "ssh");
        assert_eq!(host, Some("github.com".to_string()));
        assert_eq!(owner, Some("user".to_string()));
        assert_eq!(repo, Some("repo".to_string()));
        assert_eq!(provider, Some("github".to_string()));
    }

    #[test]
    fn parse_ssh_gitlab_url() {
        let (protocol, host, owner, repo, provider) =
            parse_remote_url("git@gitlab.com:organization/project.git");

        assert_eq!(protocol, "ssh");
        assert_eq!(host, Some("gitlab.com".to_string()));
        assert_eq!(owner, Some("organization".to_string()));
        assert_eq!(repo, Some("project".to_string()));
        assert_eq!(provider, Some("gitlab".to_string()));
    }

    #[test]
    fn parse_ssh_bitbucket_url() {
        let (protocol, host, owner, repo, provider) =
            parse_remote_url("git@bitbucket.org:team/repo.git");

        assert_eq!(protocol, "ssh");
        assert_eq!(host, Some("bitbucket.org".to_string()));
        assert_eq!(owner, Some("team".to_string()));
        assert_eq!(repo, Some("repo".to_string()));
        assert_eq!(provider, Some("bitbucket".to_string()));
    }

    #[test]
    fn parse_ssh_without_git_extension() {
        let (protocol, host, owner, repo, provider) = parse_remote_url("git@github.com:user/repo");

        assert_eq!(protocol, "ssh");
        assert_eq!(host, Some("github.com".to_string()));
        assert_eq!(owner, Some("user".to_string()));
        assert_eq!(repo, Some("repo".to_string()));
        assert_eq!(provider, Some("github".to_string()));
    }

    #[test]
    fn parse_ssh_self_hosted() {
        let (protocol, host, owner, repo, provider) =
            parse_remote_url("git@git.company.com:team/project.git");

        assert_eq!(protocol, "ssh");
        assert_eq!(host, Some("git.company.com".to_string()));
        assert_eq!(owner, Some("team".to_string()));
        assert_eq!(repo, Some("project".to_string()));
        assert_eq!(provider, Some("unknown".to_string()));
    }

    // ── parse_remote_url HTTPS tests ─────────────────────────────────

    #[test]
    fn parse_https_github_url() {
        let (protocol, host, owner, repo, provider) =
            parse_remote_url("https://github.com/user/repo.git");

        assert_eq!(protocol, "https");
        assert_eq!(host, Some("github.com".to_string()));
        assert_eq!(owner, Some("user".to_string()));
        assert_eq!(repo, Some("repo".to_string()));
        assert_eq!(provider, Some("github".to_string()));
    }

    #[test]
    fn parse_http_url() {
        let (protocol, host, owner, repo, provider) =
            parse_remote_url("http://github.com/user/repo.git");

        assert_eq!(protocol, "https"); // Note: both http and https return "https"
        assert_eq!(host, Some("github.com".to_string()));
        assert_eq!(owner, Some("user".to_string()));
        assert_eq!(repo, Some("repo".to_string()));
        assert_eq!(provider, Some("github".to_string()));
    }

    #[test]
    fn parse_https_without_git_extension() {
        let (protocol, host, owner, repo, provider) =
            parse_remote_url("https://github.com/user/repo");

        assert_eq!(protocol, "https");
        assert_eq!(host, Some("github.com".to_string()));
        assert_eq!(owner, Some("user".to_string()));
        assert_eq!(repo, Some("repo".to_string()));
        assert_eq!(provider, Some("github".to_string()));
    }

    #[test]
    fn parse_https_self_hosted_gitlab() {
        let (protocol, host, owner, repo, provider) =
            parse_remote_url("https://gitlab.company.com/team/project.git");

        assert_eq!(protocol, "https");
        assert_eq!(host, Some("gitlab.company.com".to_string()));
        assert_eq!(owner, Some("team".to_string()));
        assert_eq!(repo, Some("project".to_string()));
        assert_eq!(provider, Some("gitlab".to_string()));
    }

    // ── parse_remote_url edge cases ──────────────────────────────────

    #[test]
    fn parse_unknown_url_format() {
        let (protocol, host, owner, repo, provider) = parse_remote_url("svn://example.com/repo");

        assert_eq!(protocol, "unknown");
        assert!(host.is_none());
        assert!(owner.is_none());
        assert!(repo.is_none());
        assert!(provider.is_none());
    }

    #[test]
    fn parse_empty_url() {
        let (protocol, host, owner, repo, provider) = parse_remote_url("");

        assert_eq!(protocol, "unknown");
        assert!(host.is_none());
        assert!(owner.is_none());
        assert!(repo.is_none());
        assert!(provider.is_none());
    }

    #[test]
    fn parse_url_with_port() {
        // This might not parse perfectly but shouldn't crash
        let result = parse_remote_url("https://github.com:443/user/repo.git");
        // Should at least not panic
        assert!(!result.0.is_empty());
    }

    // ── detect_provider tests ────────────────────────────────────────

    #[test]
    fn detect_github_provider() {
        assert_eq!(
            detect_provider(&"github.com".to_string()),
            Some("github".to_string())
        );
        assert_eq!(
            detect_provider(&"api.github.com".to_string()),
            Some("github".to_string())
        );
    }

    #[test]
    fn detect_gitlab_provider() {
        assert_eq!(
            detect_provider(&"gitlab.com".to_string()),
            Some("gitlab".to_string())
        );
        assert_eq!(
            detect_provider(&"gitlab.company.com".to_string()),
            Some("gitlab".to_string())
        );
    }

    #[test]
    fn detect_bitbucket_provider() {
        assert_eq!(
            detect_provider(&"bitbucket.org".to_string()),
            Some("bitbucket".to_string())
        );
    }

    #[test]
    fn detect_unknown_provider() {
        assert_eq!(
            detect_provider(&"git.example.com".to_string()),
            Some("unknown".to_string())
        );
        assert_eq!(
            detect_provider(&"sourcehut.org".to_string()),
            Some("unknown".to_string())
        );
    }
}
