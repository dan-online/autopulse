use reqwest::header;
use serde::Deserialize;

use crate::{db::models::ScanEvent, utils::settings::TargetProcess};

#[derive(Deserialize, Clone, Debug)]
pub struct Plex {
    pub url: String,
    pub token: String,
}

#[derive(Deserialize, Clone, Debug)]
struct Location {
    path: String,
}

#[derive(Deserialize, Clone, Debug)]
struct Library {
    key: String,
    #[serde(rename = "Location")]
    location: Vec<Location>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "PascalCase")]
struct MediaContainer {
    directory: Vec<Library>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "PascalCase")]
struct LibraryResponse {
    media_container: MediaContainer,
}

impl Plex {
    fn get_client(&self) -> anyhow::Result<reqwest::Client> {
        let mut headers = header::HeaderMap::new();

        headers.insert("X-Plex-Token", self.token.parse().unwrap());
        headers.insert("Accept", "application/json".parse().unwrap());

        reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .map_err(Into::into)
    }

    async fn libraries(&self) -> anyhow::Result<LibraryResponse> {
        let client = self.get_client()?;
        let url = url::Url::parse(&self.url)?
            .join("/library/sections")?
            .to_string();

        let res = client.get(&url).send().await?;
        let libraries: LibraryResponse = res.json().await?;

        Ok(libraries)
    }

    async fn scan(&self, ev: &ScanEvent, library: &Library) -> anyhow::Result<()> {
        let client = self.get_client()?;
        let mut url = url::Url::parse(&self.url)?
            .join(&format!("/library/sections/{}/refresh", library.key))?;

        url.query_pairs_mut().append_pair("path", &ev.file_path);

        let res = client.get(url.to_string()).send().await?;

        if res.status().is_success() {
            Ok(())
        } else {
            let body = res.text().await?;
            Err(anyhow::anyhow!("unable to send scan: {}", body))
        }
    }
}

impl TargetProcess for Plex {
    async fn process(&self, ev: &ScanEvent) -> anyhow::Result<()> {
        let libraries = self.libraries().await?;

        // check if the file path is in any of the libraries and return the location
        let library: &Library = libraries
            .media_container
            .directory
            .iter()
            .find(|l| {
                l.location
                    .iter()
                    .any(|loc| ev.file_path.starts_with(&loc.path))
            })
            .ok_or_else(|| anyhow::anyhow!("file path {} not in any plex library", ev.file_path))?;

        self.scan(ev, library).await?;

        Ok(())
    }
}
