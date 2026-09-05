use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;
use url::Url;

const MAX_EXTERNAL_URL_LENGTH: usize = 4_096;

fn validate_external_url(input: &str) -> Result<Url, String> {
    if input.is_empty() || input.len() > MAX_EXTERNAL_URL_LENGTH {
        return Err("External URL has an invalid length".to_string());
    }

    if input.chars().any(char::is_control) {
        return Err("External URL contains control characters".to_string());
    }

    if !input.starts_with("https://") {
        return Err("Only HTTPS external URLs are allowed".to_string());
    }

    let url = Url::parse(input).map_err(|_| "External URL is invalid".to_string())?;

    if url.scheme() != "https" {
        return Err("Only HTTPS external URLs are allowed".to_string());
    }

    if url.host_str().is_none() {
        return Err("External URL must include a host".to_string());
    }

    if !url.username().is_empty() || url.password().is_some() {
        return Err("External URLs with embedded credentials are not allowed".to_string());
    }

    Ok(url)
}

#[tauri::command]
pub fn open_external_url(url: String, app: AppHandle) -> Result<(), String> {
    let validated_url = validate_external_url(&url)?;

    app.opener()
        .open_url(validated_url.as_str(), None::<&str>)
        .map_err(|_| "Failed to open external URL".to_string())
}

#[cfg(test)]
mod tests {
    use super::validate_external_url;

    #[test]
    fn accepts_https_urls_without_credentials() {
        let url = validate_external_url("https://gitru.app/docs?source=desktop#start").unwrap();

        assert_eq!(url.scheme(), "https");
        assert_eq!(url.host_str(), Some("gitru.app"));
    }

    #[test]
    fn rejects_non_https_schemes() {
        for url in [
            "http://gitru.app",
            "javascript:alert(1)",
            "file:///etc/passwd",
            "mailto:hello@gitru.app",
        ] {
            assert!(validate_external_url(url).is_err(), "accepted {url}");
        }
    }

    #[test]
    fn rejects_embedded_credentials_and_control_characters() {
        assert!(validate_external_url("https://token@example.com/repo").is_err());
        assert!(validate_external_url("https://example.com/\nnext").is_err());
    }

    #[test]
    fn rejects_missing_hosts_and_oversized_urls() {
        assert!(validate_external_url("https:/docs").is_err());
        assert!(
            validate_external_url(&format!("https://example.com/{}", "a".repeat(4096))).is_err()
        );
    }
}
