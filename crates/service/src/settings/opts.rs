use serde::Deserialize;
use std::path::PathBuf;

#[doc(hidden)]
const fn default_check_path() -> bool {
    false
}

#[doc(hidden)]
const fn default_max_retries() -> i32 {
    5
}

#[doc(hidden)]
const fn default_default_timer_wait() -> u64 {
    60
}

#[doc(hidden)]
const fn default_cleanup_days() -> u64 {
    10
}

#[derive(Deserialize, Clone)]
pub struct Opts {
    /// Check if the path exists before processing (default: false)
    #[serde(default = "default_check_path")]
    pub check_path: bool,

    /// Maximum retries before giving up (default: 5)
    #[serde(default = "default_max_retries")]
    pub max_retries: i32,

    /// Default timer wait time (default: 60)
    #[serde(default = "default_default_timer_wait")]
    pub default_timer_wait: u64,

    /// Cleanup events older than x days (default: 10)
    #[serde(default = "default_cleanup_days")]
    pub cleanup_days: u64,

    /// Log file path
    pub log_file: Option<PathBuf>,
}

impl Default for Opts {
    fn default() -> Self {
        Self {
            check_path: default_check_path(),
            max_retries: default_max_retries(),
            default_timer_wait: default_default_timer_wait(),
            cleanup_days: default_cleanup_days(),
            log_file: None,
        }
    }
}
