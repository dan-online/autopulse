use crate::utils::settings::{Rewrite, Timer, TriggerRequest};
use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct Readarr {
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
pub struct BookFile {
    path: String,
}

#[derive(Deserialize, Clone)]
#[serde(tag = "eventType")]
#[doc(hidden)]
pub enum ReadarrRequest {
    #[serde(rename = "Download")]
    #[serde(rename_all = "camelCase")]
    Download { book_files: Vec<BookFile> },
    #[serde(rename = "Test")]
    Test,
}

impl TriggerRequest for ReadarrRequest {
    fn from_json(json: serde_json::Value) -> anyhow::Result<Self> {
        serde_json::from_value(json).map_err(|e| anyhow::anyhow!(e))
    }
    fn paths(&self) -> Vec<(String, bool)> {
        match self {
            Self::Download { book_files } => book_files
                .iter()
                .map(|book_file| (book_file.path.clone(), true))
                .collect(),
            Self::Test => vec![],
        }
    }
}
