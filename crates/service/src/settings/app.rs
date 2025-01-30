use autopulse_utils::logs::LogLevel;
use serde::Deserialize;

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
    "postgres://autopulse:autopulse@localhost:5432/autopulse".to_string()
}

#[doc(hidden)]
fn default_log_level() -> LogLevel {
    LogLevel::default()
}

#[derive(Deserialize, Clone)]
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
}

impl Default for App {
    fn default() -> Self {
        Self {
            hostname: default_hostname(),
            port: default_port(),
            database_url: default_database_url(),
            log_level: default_log_level(),
        }
    }
}
