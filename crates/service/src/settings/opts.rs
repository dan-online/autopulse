use autopulse_utils::Rotation;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Clone, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LogRotation {
    Daily,
    Minutely,
    Hourly,
    #[default]
    Never,
}

// impl Into<Rotation> for LogRotation {
//     fn into(self) -> Rotation {
//         match self {
//             LogRotation::Daily => Rotation::DAILY,
//             LogRotation::Minute => Rotation::MINUTELY,
//             LogRotation::Hour => Rotation::HOURLY,
//             LogRotation::Never => Rotation::NEVER,
//         }
//     }
// }

// from AutopulseRotation -> Rotation
impl From<&LogRotation> for Rotation {
    fn from(rotation: &LogRotation) -> Self {
        match rotation {
            LogRotation::Daily => Self::DAILY,
            LogRotation::Minutely => Self::MINUTELY,
            LogRotation::Hourly => Self::HOURLY,
            LogRotation::Never => Self::NEVER,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Opts {
    /// Check if the path exists before processing (default: false)
    pub check_path: bool,

    /// Maximum retries before giving up (default: 5)
    pub max_retries: i32,

    /// Default timer wait time (default: 60)
    pub default_timer_wait: u64,

    /// Cleanup not_found events older than x days (default: 10)
    pub cleanup_days: u64,

    /// Log file path
    pub log_file: Option<PathBuf>,

    /// Whether to rollover the log file (default: never)
    pub log_file_rollover: LogRotation,

    /// Number of retries for webhook HTTP requests (default: 3)
    pub webhook_retries: u8,

    /// HTTP timeout in seconds for webhook requests (default: 10)
    pub webhook_timeout: u64,

    /// Interval in seconds between webhook batch sends (default: 10)
    pub webhook_interval: u64,
}

impl Default for Opts {
    fn default() -> Self {
        Self {
            check_path: false,
            max_retries: 5,
            default_timer_wait: 60,
            cleanup_days: 10,
            log_file: None,
            log_file_rollover: LogRotation::default(),
            webhook_retries: 3,
            webhook_timeout: 10,
            webhook_interval: 10,
        }
    }
}
