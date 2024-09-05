use serde::Deserialize;

use crate::utils::{join_path::join_path, settings::TriggerRequest};

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeFile {
    pub relative_path: String,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Series {
    path: String,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RenamedEpisodeFile {
    previous_path: String,
    relative_path: String,
}
#[derive(Deserialize, Clone, Debug)]
#[serde(tag = "eventType")]
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

    fn paths(&self) -> Vec<String> {
        match self {
            Self::EpisodeFileDelete {
                episode_file,
                series,
            } => {
                vec![join_path(&series.path, &episode_file.relative_path)]
            }
            Self::Rename {
                series,
                renamed_episode_files,
            } => {
                let mut paths = vec![];

                for file in renamed_episode_files {
                    paths.push(file.previous_path.clone());
                    paths.push(join_path(&series.path, &file.relative_path));
                }

                paths
            }
            Self::SeriesDelete { series } => vec![series.path.clone()],
            Self::Download {
                episode_file,
                series,
            } => {
                vec![join_path(&series.path, &episode_file.relative_path)]
            }
            Self::Test => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_json_test() {
        let json = serde_json::json!({
            "eventType": "Test"
        });

        let sonarr_request = SonarrRequest::from_json(json).unwrap();

        assert!(matches!(sonarr_request, SonarrRequest::Test {}));
    }

    #[test]
    fn test_from_json_episode_file_delete() {
        let json = serde_json::json!({
            "eventType": "EpisodeFileDelete",
            "episodeFile": {
                "relativePath": "Season 2/Westworld.S02E01.mkv"
            },
            "series": {
                "path": "/TV/Westworld"
            }
        });

        let sonarr_request = SonarrRequest::from_json(json).unwrap();

        if let SonarrRequest::EpisodeFileDelete {
            episode_file,
            series,
        } = sonarr_request.clone()
        {
            assert_eq!(episode_file.relative_path, "Season 2/Westworld.S02E01.mkv");
            assert_eq!(series.path, "/TV/Westworld");
            assert_eq!(
                sonarr_request.paths(),
                vec!["/TV/Westworld/Season 2/Westworld.S02E01.mkv"]
            );
        } else {
            panic!("Unexpected variant");
        }
    }

    #[test]
    fn test_from_json_rename() {
        let json = serde_json::json!({
            "eventType": "Rename",
            "series": {
                "path": "/TV/Westworld [imdb:tt0475784]"
            },
            "renamedEpisodeFiles": [
                {
                    "previousPath": "/TV/Westworld/Season 1/Westworld.S01E01.mkv",
                    "relativePath": "Season 1/Westworld.S01E01.mkv"
                },
                {
                    "previousPath": "/TV/Westworld/Season 1/Westworld.S01E02.mkv",
                    "relativePath": "Season 1/Westworld.S01E02.mkv"
                },
                {
                    "previousPath": "/TV/Westworld/Season 2/Westworld.S01E02.mkv",
                    "relativePath": "Season 2/Westworld.S02E01.mkv"
                }
            ]
        });

        let sonarr_request = SonarrRequest::from_json(json).unwrap();

        if let SonarrRequest::Rename {
            series,
            renamed_episode_files,
        } = sonarr_request.clone()
        {
            assert_eq!(series.path, "/TV/Westworld [imdb:tt0475784]");
            assert_eq!(renamed_episode_files.len(), 3);
            assert_eq!(
                sonarr_request.paths(),
                vec![
                    "/TV/Westworld/Season 1/Westworld.S01E01.mkv",
                    "/TV/Westworld [imdb:tt0475784]/Season 1/Westworld.S01E01.mkv",
                    "/TV/Westworld/Season 1/Westworld.S01E02.mkv",
                    "/TV/Westworld [imdb:tt0475784]/Season 1/Westworld.S01E02.mkv",
                    "/TV/Westworld/Season 2/Westworld.S01E02.mkv",
                    "/TV/Westworld [imdb:tt0475784]/Season 2/Westworld.S02E01.mkv"
                ]
            );
        } else {
            panic!("Unexpected variant");
        }
    }

    #[test]
    fn test_from_json_series_delete() {
        let json = serde_json::json!({
            "eventType": "SeriesDelete",
            "series": {
                "path": "/TV/Westworld"
            }
        });

        let sonarr_request = SonarrRequest::from_json(json).unwrap();

        if let SonarrRequest::SeriesDelete { series } = sonarr_request.clone() {
            assert_eq!(series.path, "/TV/Westworld");
            assert_eq!(sonarr_request.paths(), vec!["/TV/Westworld"]);
        } else {
            panic!("Unexpected variant");
        }
    }

    #[test]
    fn test_from_json_download() {
        let json = serde_json::json!({
            "eventType": "Download",
            "episodeFile": {
                "relativePath": "Season 1/Westworld.S01E01.mkv"
            },
            "series": {
                "path": "/TV/Westworld"
            }
        });

        let sonarr_request = SonarrRequest::from_json(json).unwrap();

        if let SonarrRequest::Download {
            episode_file,
            series,
        } = sonarr_request.clone()
        {
            assert_eq!(episode_file.relative_path, "Season 1/Westworld.S01E01.mkv");
            assert_eq!(series.path, "/TV/Westworld");
            assert_eq!(
                sonarr_request.paths(),
                vec!["/TV/Westworld/Season 1/Westworld.S01E01.mkv"]
            );
        } else {
            panic!("Unexpected variant");
        }
    }
}
