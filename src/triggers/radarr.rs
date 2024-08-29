use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MovieFile {
    relative_path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Movie {
    folder_path: String,
}

#[derive(Deserialize)]
#[serde(tag = "eventType")]
pub enum RadarrRequest {
    #[serde(rename = "Download")]
    #[serde(rename_all = "camelCase")]
    Download { movie_file: MovieFile, movie: Movie },
    #[serde(rename = "MovieDelete")]
    #[serde(rename_all = "camelCase")]
    MovieDelete { movie: Movie },
    #[serde(rename = "MovieFileDelete")]
    #[serde(rename_all = "camelCase")]
    MovieFileDelete { movie_file: MovieFile, movie: Movie },
    #[serde(rename = "Rename")]
    #[serde(rename_all = "camelCase")]
    Rename { movie: Movie },
    #[serde(rename = "Test")]
    Test,
}

impl RadarrRequest {
    pub fn from_json(json: serde_json::Value) -> anyhow::Result<Self> {
        serde_json::from_value(json).map_err(|e| anyhow::anyhow!(e))
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

        let radarr_request = RadarrRequest::from_json(json).unwrap();

        assert!(matches!(radarr_request, RadarrRequest::Test {}));
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

        if let RadarrRequest::Download { movie_file, movie } = radarr_request {
            assert_eq!(
                movie_file.relative_path,
                "Interstellar.2014.UHD.BluRay.2160p.REMUX.mkv"
            );
            assert_eq!(movie.folder_path, "/Movies/Interstellar (2014)");
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

        if let RadarrRequest::MovieDelete { movie } = radarr_request {
            assert_eq!(movie.folder_path, "/Movies/Wonder Woman 1984 (2020)");
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

        if let RadarrRequest::MovieFileDelete { movie_file, movie } = radarr_request {
            assert_eq!(movie_file.relative_path, "Tenet.2020.mkv");
            assert_eq!(movie.folder_path, "/Movies/Tenet (2020)");
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

        if let RadarrRequest::Rename { movie } = radarr_request {
            assert_eq!(movie.folder_path, "/Movies/Deadpool (2016)");
        } else {
            panic!("Unexpected variant");
        }
    }
}
