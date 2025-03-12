#[cfg(test)]
mod tests {
    use crate::{settings::triggers::radarr::RadarrRequest, settings::triggers::TriggerRequest};

    #[test]
    fn test_from_json_test() {
        let json = serde_json::json!({
            "eventType": "Test"
        });

        let radarr_request = RadarrRequest::from_json(json).unwrap();

        assert!(matches!(radarr_request, RadarrRequest::Test));
    }

    #[test]
    fn test_from_json_download() {
        let json = serde_json::json!({
            "eventType": "Download",
            "movieFile": {
                "relativePath": "Interstellar.2014.UHD.BluRay.2160p.REMUX.mkv"
            },
            "movie": {
                "folderPath": "/Movies/Interstellar (2014)"
            }
        });

        let radarr_request = RadarrRequest::from_json(json).unwrap();

        if let RadarrRequest::Download { movie_file, movie } = radarr_request.clone() {
            assert_eq!(
                movie_file.relative_path,
                "Interstellar.2014.UHD.BluRay.2160p.REMUX.mkv"
            );
            assert_eq!(movie.folder_path, "/Movies/Interstellar (2014)");
            assert_eq!(
                radarr_request.paths(),
                vec![(
                    "/Movies/Interstellar (2014)/Interstellar.2014.UHD.BluRay.2160p.REMUX.mkv"
                        .to_string(),
                    true
                )]
            );
        } else {
            panic!("Unexpected variant");
        }
    }

    #[test]
    fn test_from_json_movie_delete() {
        let json = serde_json::json!({
            "eventType": "MovieDelete",
            "movie": {
                "folderPath": "/Movies/Wonder Woman 1984 (2020)"
            }
        });

        let radarr_request = RadarrRequest::from_json(json).unwrap();

        if let RadarrRequest::MovieDelete { movie } = radarr_request.clone() {
            assert_eq!(movie.folder_path, "/Movies/Wonder Woman 1984 (2020)");
            assert_eq!(radarr_request.paths(), vec![(movie.folder_path, false)]);
        } else {
            panic!("Unexpected variant");
        }
    }

    #[test]
    fn test_from_json_movie_file_delete() {
        let json = serde_json::json!({
            "eventType": "MovieFileDelete",
            "movieFile": {
                "relativePath": "Tenet.2020.mkv"
            },
            "movie": {
                "folderPath": "/Movies/Tenet (2020)"
            }
        });

        let radarr_request = RadarrRequest::from_json(json).unwrap();

        if let RadarrRequest::MovieFileDelete { movie_file, movie } = radarr_request.clone() {
            assert_eq!(movie_file.relative_path, "Tenet.2020.mkv");
            assert_eq!(movie.folder_path, "/Movies/Tenet (2020)");

            assert_eq!(
                radarr_request.paths(),
                vec![("/Movies/Tenet (2020)/Tenet.2020.mkv".to_string(), false)]
            );
        } else {
            panic!("Unexpected variant");
        }
    }

    #[test]
    fn test_from_json_rename() {
        let json = serde_json::json!({
            "eventType": "Rename",
            "movie": {
                "folderPath": "/Movies/Deadpool (2016)"
            }
        });

        let radarr_request = RadarrRequest::from_json(json).unwrap();

        if let RadarrRequest::Rename { movie } = radarr_request.clone() {
            assert_eq!(movie.folder_path, "/Movies/Deadpool (2016)");
            assert_eq!(
                radarr_request.paths(),
                vec![("/Movies/Deadpool (2016)".to_string(), true)]
            );
        } else {
            panic!("Unexpected variant");
        }
    }
}
