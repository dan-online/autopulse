use crate::settings::rewrite::Rewrite;
use crate::settings::timer::{EventTimers, Timer};
use crate::settings::triggers::TriggerRequest;
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
    /// Event-specific timers
    #[serde(default)]
    pub event_timers: EventTimers,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[doc(hidden)]
pub struct BookFile {
    path: String,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[doc(hidden)]
pub struct RenamedBookFile {
    path: String,
    previous_path: String,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[doc(hidden)]
pub struct Author {
    path: String,
}

// Based on https://github.com/Readarr/Readarr/blob/develop/src/NzbDrone.Core/Notifications/Webhook/WebhookBase.cs
#[derive(Deserialize, Clone)]
#[serde(tag = "eventType")]
#[doc(hidden)]
pub enum ReadarrRequest {
    #[serde(rename = "Download")]
    #[serde(rename_all = "camelCase")]
    Download { book_files: Vec<BookFile> },
    #[serde(rename = "Rename")]
    #[serde(rename_all = "camelCase")]
    Rename {
        renamed_book_files: Vec<RenamedBookFile>,
    },
    #[serde(rename = "AuthorDelete")]
    #[serde(rename_all = "camelCase")]
    AuthorDelete { author: Author },
    #[serde(rename = "BookDelete")]
    #[serde(rename_all = "camelCase")]
    BookDelete { author: Author },
    #[serde(rename = "BookFileDelete")]
    #[serde(rename_all = "camelCase")]
    BookFileDelete { book_file: BookFile },
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
            Self::Rename { renamed_book_files } => {
                let mut paths = vec![];

                for file in renamed_book_files {
                    paths.push((file.previous_path.clone(), false));
                    paths.push((file.path.clone(), true));
                }

                paths
            }
            Self::AuthorDelete { author } | Self::BookDelete { author } => {
                vec![(author.path.clone(), false)]
            }
            Self::BookFileDelete { book_file } => {
                vec![(book_file.path.clone(), false)]
            }
            Self::Test => vec![],
        }
    }
}
