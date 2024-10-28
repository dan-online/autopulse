#[cfg(test)]
mod tests {
    use crate::{service::triggers::sonarr::SonarrRequest, settings::trigger::TriggerRequest};

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
                vec![(
                    "/TV/Westworld/Season 2/Westworld.S02E01.mkv".to_string(),
                    false
                )]
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
                    (
                        "/TV/Westworld/Season 1/Westworld.S01E01.mkv".to_string(),
                        false
                    ),
                    (
                        "/TV/Westworld [imdb:tt0475784]/Season 1/Westworld.S01E01.mkv".to_string(),
                        true
                    ),
                    (
                        "/TV/Westworld/Season 1/Westworld.S01E02.mkv".to_string(),
                        false
                    ),
                    (
                        "/TV/Westworld [imdb:tt0475784]/Season 1/Westworld.S01E02.mkv".to_string(),
                        true
                    ),
                    (
                        "/TV/Westworld/Season 2/Westworld.S01E02.mkv".to_string(),
                        false
                    ),
                    (
                        "/TV/Westworld [imdb:tt0475784]/Season 2/Westworld.S02E01.mkv".to_string(),
                        true
                    )
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
            assert_eq!(
                sonarr_request.paths(),
                vec![("/TV/Westworld".to_string(), false)]
            );
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
                vec![(
                    "/TV/Westworld/Season 1/Westworld.S01E01.mkv".to_string(),
                    true
                )]
            );
        } else {
            panic!("Unexpected variant");
        }
    }
}
