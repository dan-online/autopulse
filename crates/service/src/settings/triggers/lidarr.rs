use crate::settings::rewrite::Rewrite;
use crate::settings::timer::EventTimers;
use crate::settings::{timer::Timer, triggers::TriggerRequest};
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
    /// Event-specific timers
    #[serde(default)]
    pub event_timers: EventTimers,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[doc(hidden)]
pub struct TrackFile {
    path: String,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[doc(hidden)]
pub struct RenamedTrackFile {
    path: String,
    previous_path: String,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[doc(hidden)]
pub struct Artist {
    path: String,
}

// Based on https://github.com/Lidarr/Lidarr/blob/develop/src/NzbDrone.Core/Notifications/Webhook/WebhookBase.cs
#[derive(Deserialize, Clone)]
#[serde(tag = "eventType")]
#[doc(hidden)]
pub enum LidarrRequest {
    #[serde(rename = "Download")]
    #[serde(rename_all = "camelCase")]
    Download { track_files: Vec<TrackFile> },
    #[serde(rename = "Rename")]
    #[serde(rename_all = "camelCase")]
    Rename {
        renamed_track_files: Vec<RenamedTrackFile>,
    },
    #[serde(rename = "ArtistDelete")]
    #[serde(rename_all = "camelCase")]
    ArtistDelete { artist: Artist },
    #[serde(rename = "AlbumDelete")]
    #[serde(rename_all = "camelCase")]
    AlbumDelete { artist: Artist },
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
            Self::Rename {
                renamed_track_files,
            } => {
                let mut paths = vec![];

                for file in renamed_track_files {
                    paths.push((file.previous_path.clone(), false));
                    paths.push((file.path.clone(), true));
                }

                paths
            }
            Self::ArtistDelete { artist } | Self::AlbumDelete { artist } => {
                vec![(artist.path.clone(), false)]
            }
            Self::Test => vec![],
        }
    }
}
