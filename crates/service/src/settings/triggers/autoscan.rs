use crate::settings::rewrite::Rewrite;
use crate::settings::timer::Timer;
use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct Autoscan {
    /// Rewrite path
    pub rewrite: Option<Rewrite>,
    /// Timer settings
    #[serde(default)]
    pub timer: Timer,
    /// Targets to ignore
    #[serde(default)]
    pub excludes: Vec<String>,
}

/// Autoscan trigger query parameters
///
/// Example:
/// - /triggers/autoscan?dir=/path/to/dir
#[derive(Deserialize)]
pub struct AutoscanQueryParams {
    /// Path to the directory
    pub dir: String,
}
