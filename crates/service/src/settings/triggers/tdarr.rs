use crate::settings::path_filter::PathFilter;
use crate::settings::rewrite::Rewrite;
use crate::settings::timer::Timer;
use crate::settings::triggers::{TriggerConfig, TriggerRequest};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Tdarr {
    /// Rewrite path
    pub rewrite: Option<Rewrite>,
    /// Timer settings
    pub timer: Option<Timer>,
    /// Targets to ignore
    #[serde(default)]
    pub excludes: Vec<String>,
    /// Path filter matched against the rewritten file path.
    #[serde(default)]
    pub filter: PathFilter,
}

impl TriggerConfig for Tdarr {
    fn rewrite(&self) -> Option<&Rewrite> {
        self.rewrite.as_ref()
    }

    fn timer(&self) -> Option<&Timer> {
        self.timer.as_ref()
    }

    fn excludes(&self) -> &Vec<String> {
        &self.excludes
    }

    fn filter(&self) -> &PathFilter {
        &self.filter
    }
}

/// JSON body: `path` (new file), `original_path` (replaced original, queued without
/// an existence check unless equal to `path`), `dir`. Values are preserved exactly;
/// blank or whitespace-only values are ignored. At least one path is required.
#[derive(Deserialize, Clone)]
pub struct TdarrRequest {
    pub path: Option<String>,
    pub original_path: Option<String>,
    pub dir: Option<String>,
}

impl TriggerRequest for TdarrRequest {
    fn from_json(json: serde_json::Value) -> anyhow::Result<Self> {
        let request: Self = serde_json::from_value(json)?;
        anyhow::ensure!(!request.paths().is_empty(), "Tdarr request has no paths");
        Ok(request)
    }

    fn paths(&self) -> Vec<(String, bool)> {
        // Tdarr renders missing template variables as "", so blank values count as absent.
        let present =
            |value: &Option<String>| value.clone().filter(|value| !value.trim().is_empty());
        let path = present(&self.path);
        let original_path =
            present(&self.original_path).filter(|original| Some(original) != path.as_ref());
        [
            (path, true),
            (original_path, false),
            (present(&self.dir), true),
        ]
        .into_iter()
        .filter_map(|(path, search)| Some((path?, search)))
        .collect()
    }
}
