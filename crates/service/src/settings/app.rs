use autopulse_utils::LogLevel;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

/// Normalize `base_path` so `format!("{base}/ui/...")` is always well-formed:
/// either `""` or `/<prefix>`, no trailing slash. Tests cover the corner cases.
fn normalize_base_path<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = String::deserialize(deserializer)?;
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    Ok(if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    })
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct App {
    /// Hostname to bind to, (default: 0.0.0.0)
    pub hostname: String,
    /// Port to bind to (default: 2875)
    pub port: u16,
    /// Database URL (see [`AnyConnection`](autopulse_database::conn::AnyConnection))
    pub database_url: String,
    /// Log level (default: info) (trace, debug, info, warn, error)
    pub log_level: LogLevel,
    /// Whether to include api logging (default: false)
    pub api_logging: bool,
    /// Reverse-proxy base path (default: ""). UI routes are mounted under
    /// this prefix server-side and generated links include it, so the
    /// proxy should pass the prefix through verbatim (no strip-prefix).
    /// Input is normalized: leading slash added if missing, trailing
    /// slash stripped, `"/"` collapses to `""`.
    #[serde(deserialize_with = "normalize_base_path")]
    pub base_path: String,
    /// Whether to set the `Secure` flag on the UI session cookie
    /// (default: false). Enable when serving over HTTPS/TLS.
    pub secure_cookies: bool,
    /// Proxy IPs whose `X-Forwarded-For` we honor for the login throttle's
    /// client identification. Empty (default) = trust nothing, use `peer_addr`.
    pub trusted_proxies: Vec<IpAddr>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            hostname: "0.0.0.0".to_string(),
            port: 2875,
            database_url: autopulse_database::conn::DatabaseType::default().default_url(),
            log_level: LogLevel::default(),
            api_logging: false,
            base_path: String::new(),
            secure_cookies: false,
            trusted_proxies: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::App;

    fn base_path_of(json: &str) -> String {
        let app: App = serde_json::from_str(json).expect("valid app json");
        app.base_path
    }

    #[test]
    fn base_path_empty_stays_empty() {
        assert_eq!(base_path_of(r#"{"base_path": ""}"#), "");
    }

    #[test]
    fn base_path_lone_slash_collapses_to_empty() {
        assert_eq!(base_path_of(r#"{"base_path": "/"}"#), "");
    }

    #[test]
    fn base_path_missing_leading_slash_gets_one() {
        assert_eq!(base_path_of(r#"{"base_path": "autopulse"}"#), "/autopulse");
    }

    #[test]
    fn base_path_trailing_slash_stripped() {
        assert_eq!(
            base_path_of(r#"{"base_path": "/autopulse/"}"#),
            "/autopulse"
        );
    }

    #[test]
    fn base_path_already_normalized_unchanged() {
        assert_eq!(base_path_of(r#"{"base_path": "/autopulse"}"#), "/autopulse");
    }

    #[test]
    fn base_path_trims_whitespace_and_trailing_slashes() {
        assert_eq!(
            base_path_of(r#"{"base_path": "  /autopulse//  "}"#),
            "/autopulse"
        );
    }

    #[test]
    fn base_path_default_is_empty() {
        let app: App = serde_json::from_str("{}").expect("valid empty app json");
        assert_eq!(app.base_path, "");
    }
}
