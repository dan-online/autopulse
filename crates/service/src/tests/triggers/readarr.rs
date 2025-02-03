#[cfg(test)]
mod tests {
    use crate::{settings::triggers::readarr::ReadarrRequest, settings::triggers::TriggerRequest};

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

    #[test]
    fn test_from_json_rename() {
        let json = serde_json::json!({
            "eventType": "Rename",
            "renamedBookFiles": [
                {
                    "previousPath": "/Books/Brandon Sanderson/The Way of Kings (2010)/The Way of Kings - Brandon Sanderson.epub",
                    "path": "/Books/Brandon Sanderson/The Way of Kings (2010)/The Way of Kings - Brandon Sanderson.epub"
                }
            ]
        });

        let readarr_request = ReadarrRequest::from_json(json).unwrap();

        if let ReadarrRequest::Rename { .. } = readarr_request {
            assert_eq!(
                readarr_request.paths(),
                vec![
                    ("/Books/Brandon Sanderson/The Way of Kings (2010)/The Way of Kings - Brandon Sanderson.epub".to_string(), false),
                    ("/Books/Brandon Sanderson/The Way of Kings (2010)/The Way of Kings - Brandon Sanderson.epub".to_string(), true)
                ]
            );
        } else {
            panic!("Unexpected variant");
        }
    }

    #[test]
    fn test_from_json_author_delete() {
        let json = serde_json::json!({
            "eventType": "AuthorDelete",
            "author": {
                "path": "/Books/Brandon Sanderson"
            }
        });

        let readarr_request = ReadarrRequest::from_json(json).unwrap();

        if let ReadarrRequest::AuthorDelete { .. } = readarr_request {
            assert_eq!(
                readarr_request.paths(),
                vec![("/Books/Brandon Sanderson".to_string(), false)]
            );
        } else {
            panic!("Unexpected variant");
        }
    }

    #[test]
    fn test_from_json_book_delete() {
        let json = serde_json::json!({
            "eventType": "BookDelete",
            "author": {
                "path": "/Books/Brandon Sanderson"
            }
        });

        let readarr_request = ReadarrRequest::from_json(json).unwrap();

        if let ReadarrRequest::BookDelete { .. } = readarr_request {
            assert_eq!(
                readarr_request.paths(),
                vec![("/Books/Brandon Sanderson".to_string(), false)]
            );
        } else {
            panic!("Unexpected variant");
        }
    }

    #[test]
    fn test_from_json_book_file_delete() {
        let json = serde_json::json!({
            "eventType": "BookFileDelete",
            "bookFile": {
                "path": "/Books/Brandon Sanderson/The Way of Kings (2010)/The Way of Kings - Brandon Sanderson.epub"
            }
        });

        let readarr_request = ReadarrRequest::from_json(json).unwrap();

        if let ReadarrRequest::BookFileDelete { .. } = readarr_request {
            assert_eq!(
                readarr_request.paths(),
                vec![("/Books/Brandon Sanderson/The Way of Kings (2010)/The Way of Kings - Brandon Sanderson.epub".to_string(), false)]
            );
        } else {
            panic!("Unexpected variant");
        }
    }
}
