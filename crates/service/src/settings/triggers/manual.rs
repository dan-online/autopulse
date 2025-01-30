use crate::settings::rewrite::Rewrite;
use crate::settings::timer::Timer;
use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct Manual {
    /// Rewrite path
    pub rewrite: Option<Rewrite>,
    /// Timer settings
    #[serde(default)]
    pub timer: Timer,
    /// Targets to ignore
    #[serde(default)]
    pub excludes: Vec<String>,
}

/// Manual trigger query parameters
///
/// Example:
/// - /triggers/manual?path=/path/to/file
/// - /triggers/manual?path=/path/to/file&hash=3b3fa...
#[derive(Deserialize)]
pub struct ManualQueryParams {
    /// Path to the file
    pub path: String,
    /// Optional sha256sum hash of the file
    pub hash: Option<String>,
}
