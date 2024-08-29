use reqwest::header;
use serde::{Deserialize, Serialize};

use crate::{db::models::ScanEvent, utils::settings::TargetProcess};

#[derive(Deserialize, Clone, Debug)]
pub struct Jellyfin {
    pub url: String,
    pub token: String,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "PascalCase")]
struct Library {
    #[allow(dead_code)]
    name: String,
    locations: Vec<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "PascalCase")]
struct UpdateRequest {
    path: String,
    update_type: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "PascalCase")]
struct ScanPayload {
    updates: Vec<UpdateRequest>,
}

impl Jellyfin {
    fn get_client(&self) -> anyhow::Result<reqwest::Client> {
        let mut headers = header::HeaderMap::new();

        headers.insert("X-Emby-Token", self.token.parse().unwrap());
        headers.insert("Accept", "application/json".parse().unwrap());

        reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .map_err(Into::into)
    }

    async fn libraries(&self) -> anyhow::Result<Vec<Library>> {
        let client = self.get_client()?;
        let url = url::Url::parse(&self.url)?
            .join("/Library/VirtualFolders")?
            .to_string();

        let res = client.get(&url).send().await?;
        let libraries: Vec<Library> = res.json().await?;

        Ok(libraries)
    }

    // hm, this could maybe use the Item refresh endpoint instead..., just have to find the item first
    async fn scan(&self, ev: &ScanEvent) -> anyhow::Result<()> {
        let client = self.get_client()?;
        let url = url::Url::parse(&self.url)?
            .join("/Library/Media/Updated")?
            .to_string();

        let req = UpdateRequest {
            path: ev.file_path.clone(),
            update_type: "Modified".to_string(),
        };

        let body = ScanPayload { updates: vec![req] };

        let res = client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if res.status().is_success() {
            Ok(())
        } else {
            let body = res.text().await?;
            Err(anyhow::anyhow!("Failed to send scan: {}", body))
        }
    }
}

impl TargetProcess for Jellyfin {
    async fn process(&self, ev: &ScanEvent) -> anyhow::Result<()> {
        let libraries = self.libraries().await?;

        // check if the file path is in any of the library locations
        libraries
            .iter()
            .find(|library| {
                library
                    .locations
                    .iter()
                    .any(|location| ev.file_path.starts_with(location))
            })
            .ok_or_else(|| anyhow::anyhow!("No matching library found"))?;

        // scan the file
        self.scan(ev).await?;

        Ok(())
    }
}
