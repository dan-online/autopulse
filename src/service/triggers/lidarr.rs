use crate::utils::settings::{Rewrite, Timer, TriggerRequest};
use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct Lidarr {
    /// Rewrite path
    pub rewrite: Option<Rewrite>,
    /// Timer settings
    #[serde(default)]
    pub timer: Timer,
    /// Targets to ignore
    #[serde(default)]
    pub excludes: Vec<String>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[doc(hidden)]
pub struct TrackFile {
    path: String,
}

#[derive(Deserialize, Clone)]
#[serde(tag = "eventType")]
#[doc(hidden)]
pub enum LidarrRequest {
    #[serde(rename = "Download")]
    #[serde(rename_all = "camelCase")]
    Download { track_files: Vec<TrackFile> },
    #[serde(rename = "Test")]
    Test,
}

impl TriggerRequest for LidarrRequest {
    fn from_json(json: serde_json::Value) -> anyhow::Result<Self> {
        serde_json::from_value(json).map_err(|e| anyhow::anyhow!(e))
    }
    fn paths(&self) -> Vec<(String, bool)> {
        match self {
            Self::Download { track_files, .. } => track_files
                .iter()
                .map(|track_file| (track_file.path.clone(), true))
                .collect(),
            Self::Test => vec![],
        }
    }
}
