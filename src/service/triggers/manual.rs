use crate::utils::settings::{Rewrite, Timer};
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

#[derive(Deserialize)]
pub struct ManualQueryParams {
    /// Path to the file
    pub path: String,
    /// Optional hash of the file
    pub hash: Option<String>,
}
