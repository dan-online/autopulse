#[cfg(test)]
mod tests {
    use crate::{service::triggers::readarr::ReadarrRequest, settings::trigger::TriggerRequest};

    #[test]
    fn test_from_json_test() {
        let json = serde_json::json!({
            "eventType": "Test"
        });

        let readarr_request = ReadarrRequest::from_json(json).unwrap();

        assert!(matches!(readarr_request, ReadarrRequest::Test {}));
    }

    #[test]
    fn test_from_json_download() {
        let json = serde_json::json!({
            "eventType": "Download",
            "bookFiles": [
                { "path": "/Books/Brandon Sanderson/The Way of Kings (2010)/The Way of Kings - Brandon Sanderson.epub" }
            ],
            "author": {
                "name": "Brandon Sanderson",
                "path": "/Books/Brandon Sanderson"
            }
        });

        let readarr_request = ReadarrRequest::from_json(json).unwrap();

        if let ReadarrRequest::Download { .. } = readarr_request {
            assert_eq!(
                readarr_request.paths(),
                vec![
                    ("/Books/Brandon Sanderson/The Way of Kings (2010)/The Way of Kings - Brandon Sanderson.epub".to_string(), true)
                ]
            );
        } else {
            panic!("Unexpected variant");
        }
    }
}
