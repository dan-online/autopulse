use serde::Deserialize;

use crate::utils::settings::TriggerRequest;

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BookFile {
    path: String,
}

#[derive(Deserialize, Clone)]
#[serde(tag = "eventType")]
pub enum ReadarrRequest {
    #[serde(rename = "Download")]
    #[serde(rename_all = "camelCase")]
    Download { book_files: Vec<BookFile> },
    #[serde(rename = "Test")]
    Test,
}

impl TriggerRequest for ReadarrRequest {
    fn from_json(json: serde_json::Value) -> anyhow::Result<Self> {
        serde_json::from_value(json).map_err(|e| anyhow::anyhow!(e))
    }
    fn paths(&self) -> Vec<(String, bool)> {
        match self {
            Self::Download { book_files } => book_files
                .iter()
                .map(|book_file| (book_file.path.clone(), true))
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
