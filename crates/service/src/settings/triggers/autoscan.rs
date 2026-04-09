use crate::settings::rewrite::Rewrite;
use crate::settings::timer::Timer;
use crate::settings::triggers::TriggerConfig;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct Autoscan {
    /// Rewrite path
    pub rewrite: Option<Rewrite>,
    /// Timer settings
    pub timer: Option<Timer>,
    /// Targets to ignore
    #[serde(default)]
    pub excludes: Vec<String>,
}

impl TriggerConfig for Autoscan {
    fn rewrite(&self) -> Option<&Rewrite> {
        self.rewrite.as_ref()
    }

    fn timer(&self) -> Option<&Timer> {
        self.timer.as_ref()
    }

    fn excludes(&self) -> &Vec<String> {
        &self.excludes
    }
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
