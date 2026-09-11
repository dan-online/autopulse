use crate::settings::path_filter::PathFilter;
use crate::settings::rewrite::Rewrite;
use crate::settings::timer::{EventTimers, Timer};
use crate::settings::triggers::{TriggerConfig, TriggerRequest};
use autopulse_utils::join_path;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct Sportarr {
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
    /// Event-specific timers
    pub event_timers: Option<EventTimers>,
}

impl TriggerConfig for Sportarr {
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

    fn event_timers(&self) -> Option<&EventTimers> {
        self.event_timers.as_ref()
    }
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[doc(hidden)]
pub struct EpisodeFile {
    pub relative_path: String,
}

/// `SeriesDelete` omits `path` because it removes metadata only.
/// Unlike Sonarr, Sportarr paths must be optional.
#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[doc(hidden)]
pub struct Series {
    pub path: Option<String>,
}

#[derive(Deserialize, Clone)]
#[serde(tag = "eventType")]
#[doc(hidden)]
pub enum SportarrRequest {
    #[serde(rename = "Download")]
    #[serde(rename_all = "camelCase")]
    Download {
        episode_file: Option<EpisodeFile>,
        #[serde(default)]
        episode_files: Vec<EpisodeFile>,
        #[serde(default)]
        deleted_files: Vec<EpisodeFile>,
        series: Series,
    },
    /// Rename webhooks identify the batch by `series.path`; rescan that directory.
    #[serde(rename = "Rename")]
    #[serde(rename_all = "camelCase")]
    Rename { series: Series },
    #[serde(rename = "SeriesDelete")]
    #[serde(rename_all = "camelCase")]
    SeriesDelete { series: Series },
    /// Sportarr uses plural `deletedFiles`, unlike Sonarr's `episodeFile`.
    #[serde(rename = "EpisodeFileDelete")]
    #[serde(rename_all = "camelCase")]
    EpisodeFileDelete {
        #[serde(default)]
        deleted_files: Vec<EpisodeFile>,
        series: Series,
    },
    #[serde(rename = "Test")]
    Test,
    #[serde(other)]
    Other,
}

impl TriggerRequest for SportarrRequest {
    fn from_json(json: serde_json::Value) -> anyhow::Result<Self> {
        serde_json::from_value(json).map_err(|e| anyhow::anyhow!(e))
    }

    fn paths(&self) -> Vec<(String, bool)> {
        match self {
            Self::EpisodeFileDelete {
                deleted_files,
                series,
            } => {
                let Some(series_path) = series.path.as_ref() else {
                    return vec![];
                };

                deleted_files
                    .iter()
                    .map(|f| (join_path(series_path, &f.relative_path), false))
                    .collect()
            }
            Self::Rename { series } => series
                .path
                .clone()
                .map(|path| vec![(path, true)])
                .unwrap_or_default(),
            Self::SeriesDelete { series } => series
                .path
                .clone()
                .map(|path| vec![(path, false)])
                .unwrap_or_default(),
            Self::Download {
                episode_file,
                episode_files,
                series,
                deleted_files,
            } => {
                let mut paths: Vec<(String, bool)> = vec![];

                let Some(series_path) = series.path.as_ref() else {
                    return paths;
                };

                if let Some(ef) = episode_file {
                    paths.push((join_path(series_path, &ef.relative_path), true));
                }

                for ef in episode_files {
                    paths.push((join_path(series_path, &ef.relative_path), true));
                }

                for file in deleted_files {
                    paths.push((join_path(series_path, &file.relative_path), false));
                }

                paths
            }
            Self::Test | Self::Other => vec![],
        }
    }
}
