use autopulse_utils::LogLevel;
use serde::{Deserialize, Serialize};

#[doc(hidden)]
fn default_hostname() -> String {
    "0.0.0.0".to_string()
}

#[doc(hidden)]
const fn default_port() -> u16 {
    2875
}

#[doc(hidden)]
fn default_database_url() -> String {
    autopulse_database::conn::DatabaseType::default().default_url()
}

#[doc(hidden)]
fn default_log_level() -> LogLevel {
    LogLevel::default()
}

/// Normalize `base_path` so downstream `format!("{base}/ui/...")` is
/// always well-formed. Accepts any of `""`, `"/"`, `"autopulse"`,
/// `"/autopulse"`, `"/autopulse/"`, `"  /autopulse/  "` and yields
/// either `""` or `"/autopulse"`.
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
pub struct App {
    /// Hostname to bind to, (default: 0.0.0.0)
    #[serde(default = "default_hostname")]
    pub hostname: String,
    /// Port to bind to (default: 2875)
    #[serde(default = "default_port")]
    pub port: u16,
    /// Database URL (see [`AnyConnection`](autopulse_database::conn::AnyConnection))
    #[serde(default = "default_database_url")]
    pub database_url: String,
    /// Log level (default: info) (trace, debug, info, warn, error)
    #[serde(default = "default_log_level")]
    pub log_level: LogLevel,
    /// Whether to include api logging (default: false)
    #[serde(default)]
    pub api_logging: bool,
    /// Reverse-proxy base path (default: ""). UI routes are mounted under
    /// this prefix server-side and generated links include it, so the
    /// proxy should pass the prefix through verbatim (no strip-prefix).
    /// Input is normalized: leading slash added if missing, trailing
    /// slash stripped, `"/"` collapses to `""`.
    #[serde(default, deserialize_with = "normalize_base_path")]
    pub base_path: String,
    /// Whether to set the `Secure` flag on the UI session cookie
    /// (default: false). Enable when serving over HTTPS/TLS.
    #[serde(default)]
    pub secure_cookies: bool,
}

impl Default for App {
    fn default() -> Self {
        Self {
            hostname: default_hostname(),
            port: default_port(),
            database_url: default_database_url(),
            log_level: default_log_level(),
            api_logging: false,
            base_path: String::new(),
            secure_cookies: false,
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
