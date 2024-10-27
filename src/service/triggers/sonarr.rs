use crate::utils::{
    join_path::join_path,
    settings::{Rewrite, Timer, TriggerRequest},
};
use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct Sonarr {
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
pub struct EpisodeFile {
    pub relative_path: String,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[doc(hidden)]
pub struct Series {
    pub path: String,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[doc(hidden)]
pub struct RenamedEpisodeFile {
    pub previous_path: String,
    pub relative_path: String,
}
#[derive(Deserialize, Clone)]
#[serde(tag = "eventType")]
#[doc(hidden)]
pub enum SonarrRequest {
    #[serde(rename = "EpisodeFileDelete")]
    #[serde(rename_all = "camelCase")]
    EpisodeFileDelete {
        episode_file: EpisodeFile,
        series: Series,
    },
    #[serde(rename = "Rename")]
    #[serde(rename_all = "camelCase")]
    Rename {
        series: Series,
        renamed_episode_files: Vec<RenamedEpisodeFile>,
    },
    #[serde(rename = "SeriesDelete")]
    #[serde(rename_all = "camelCase")]
    SeriesDelete { series: Series },
    #[serde(rename = "Download")]
    #[serde(rename_all = "camelCase")]
    Download {
        episode_file: EpisodeFile,
        series: Series,
    },
    #[serde(rename = "Test")]
    Test,
}

impl TriggerRequest for SonarrRequest {
    fn from_json(json: serde_json::Value) -> anyhow::Result<Self> {
        serde_json::from_value(json).map_err(|e| anyhow::anyhow!(e))
    }

    fn paths(&self) -> Vec<(String, bool)> {
        match self {
            Self::EpisodeFileDelete {
                episode_file,
                series,
            } => {
                vec![(join_path(&series.path, &episode_file.relative_path), false)]
            }
            Self::Rename {
                series,
                renamed_episode_files,
            } => {
                let mut paths = vec![];

                for file in renamed_episode_files {
                    paths.push((file.previous_path.clone(), false));
                    paths.push((join_path(&series.path, &file.relative_path), true));
                }

                paths
            }
            Self::SeriesDelete { series } => vec![(series.path.clone(), false)],
            Self::Download {
                episode_file,
                series,
            } => {
                vec![(join_path(&series.path, &episode_file.relative_path), true)]
            }
            Self::Test => vec![],
        }
    }
}
