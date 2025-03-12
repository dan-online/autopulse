#![cfg(test)]
mod tests {
    use crate::{settings::triggers::lidarr::LidarrRequest, settings::triggers::TriggerRequest};

    #[test]
    fn test_from_json_test() {
        let json = serde_json::json!({
            "eventType": "Test"
        });

        let lidarr_request = LidarrRequest::from_json(json).unwrap();

        assert!(matches!(lidarr_request, LidarrRequest::Test));
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

    #[test]
    fn test_from_json_rename() {
        let json = serde_json::json!({
            "eventType": "Rename",
            "renamedTrackFiles": [
                {
                    "path": "/Music/blink‐182/California (2016)/CD 01/01 - Cynical.mp3",
                    "previousPath": "/Music/blink‐182/California (2016)/CD 01/01 - Cynical (Demo).mp3"
                },
                {
                    "path": "/Music/blink‐182/California (2016)/CD 01/02 - Bored to Death.mp3",
                    "previousPath": "/Music/blink‐182/California (2016)/CD 01/02 - Bored to Death (Demo).mp3"
                },
                {
                    "path": "/Music/blink‐182/California (2016)/CD 02/01 - Parking Lot.mp3",
                    "previousPath": "/Music/blink‐182/California (2016)/CD 02/01 - Parking Lot (Demo).mp3"
                },
                {
                    "path": "/Music/blink‐182/California (2016)/CD 02/02 - Misery.mp3",
                    "previousPath": "/Music/blink‐182/California (2016)/CD 02/02 - Misery (Demo).mp3"
                }
            ]
        });

        let lidarr_request = LidarrRequest::from_json(json).unwrap();

        if let LidarrRequest::Rename { .. } = lidarr_request {
            assert_eq!(
                lidarr_request.paths(),
                vec![
                    (
                        "/Music/blink‐182/California (2016)/CD 01/01 - Cynical (Demo).mp3"
                            .to_string(),
                        false
                    ),
                    (
                        "/Music/blink‐182/California (2016)/CD 01/01 - Cynical.mp3".to_string(),
                        true
                    ),
                    (
                        "/Music/blink‐182/California (2016)/CD 01/02 - Bored to Death (Demo).mp3"
                            .to_string(),
                        false
                    ),
                    (
                        "/Music/blink‐182/California (2016)/CD 01/02 - Bored to Death.mp3"
                            .to_string(),
                        true
                    ),
                    (
                        "/Music/blink‐182/California (2016)/CD 02/01 - Parking Lot (Demo).mp3"
                            .to_string(),
                        false
                    ),
                    (
                        "/Music/blink‐182/California (2016)/CD 02/01 - Parking Lot.mp3".to_string(),
                        true
                    ),
                    (
                        "/Music/blink‐182/California (2016)/CD 02/02 - Misery (Demo).mp3"
                            .to_string(),
                        false
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

    #[test]
    fn test_from_json_artist_delete() {
        let json = serde_json::json!({
            "eventType": "ArtistDelete",
            "artist": {
                "path": "/Music/blink-182"
            }
        });

        let lidarr_request = LidarrRequest::from_json(json).unwrap();

        if let LidarrRequest::ArtistDelete { .. } = lidarr_request {
            assert_eq!(
                lidarr_request.paths(),
                vec![("/Music/blink-182".to_string(), false)]
            );
        } else {
            panic!("Unexpected variant");
        }
    }

    #[test]
    fn test_from_json_album_delete() {
        let json = serde_json::json!({
            "eventType": "AlbumDelete",
            "artist": {
                "path": "/Music/blink-182"
            }
        });

        let lidarr_request = LidarrRequest::from_json(json).unwrap();

        if let LidarrRequest::AlbumDelete { .. } = lidarr_request {
            assert_eq!(
                lidarr_request.paths(),
                vec![("/Music/blink-182".to_string(), false)]
            );
        } else {
            panic!("Unexpected variant");
        }
    }
}
