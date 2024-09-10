use serde::Deserialize;

use crate::utils::settings::TriggerRequest;

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TrackFile {
    path: String,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(tag = "eventType")]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_json_test() {
        let json = serde_json::json!({
            "eventType": "Test"
        });

        let lidarr_request = LidarrRequest::from_json(json).unwrap();

        assert!(matches!(lidarr_request, LidarrRequest::Test {}));
    }

    #[test]
    fn test_from_json_download() {
        let json = serde_json::json!({
            "eventType": "Download",
            "trackFiles": [
                { "path": "/Music/blink‐182/California (2016)/CD 01/01 - Cynical.mp3" },
                { "path": "/Music/blink‐182/California (2016)/CD 01/02 - Bored to Death.mp3" },
                { "path": "/Music/blink‐182/California (2016)/CD 02/01 - Parking Lot.mp3" },
                { "path": "/Music/blink‐182/California (2016)/CD 02/02 - Misery.mp3" }
            ],
            "artist": {
                "name": "blink-182",
                "path": "/Music/blink-182"
            }
        });

        let lidarr_request = LidarrRequest::from_json(json).unwrap();

        if let LidarrRequest::Download { .. } = lidarr_request {
            assert_eq!(
                lidarr_request.paths(),
                vec![
                    (
                        "/Music/blink‐182/California (2016)/CD 01/01 - Cynical.mp3".to_string(),
                        true
                    ),
                    (
                        "/Music/blink‐182/California (2016)/CD 01/02 - Bored to Death.mp3"
                            .to_string(),
                        true
                    ),
                    (
                        "/Music/blink‐182/California (2016)/CD 02/01 - Parking Lot.mp3".to_string(),
                        true
                    ),
                    (
                        "/Music/blink‐182/California (2016)/CD 02/02 - Misery.mp3".to_string(),
                        true
                    ),
                ]
            );
        } else {
            panic!("Unexpected variant");
        }
    }
}
