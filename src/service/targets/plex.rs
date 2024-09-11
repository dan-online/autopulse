use reqwest::header;
use serde::Deserialize;
use tracing::error;

use crate::{db::models::ScanEvent, utils::settings::TargetProcess};

#[derive(Deserialize, Clone)]
pub struct Plex {
    pub url: String,
    pub token: String,
}

#[derive(Deserialize, Clone)]
struct Location {
    path: String,
}

#[derive(Deserialize, Clone)]
struct Library {
    key: String,
    #[serde(rename = "Location")]
    location: Vec<Location>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
struct MediaContainer {
    directory: Vec<Library>,
}

#[derive(Deserialize, Clone)]
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
            .timeout(std::time::Duration::from_secs(10))
            .default_headers(headers)
            .build()
            .map_err(Into::into)
    }

    async fn libraries(&self) -> anyhow::Result<Vec<Library>> {
        let client = self.get_client()?;
        let url = url::Url::parse(&self.url)?
            .join("/library/sections")?
            .to_string();

        let res = client.get(&url).send().await?;
        let libraries: LibraryResponse = res.json().await?;

        Ok(libraries.media_container.directory)
    }

    fn in_library(
        &self,
        libraries: &Vec<Library>,
        ev: &ScanEvent,
    ) -> anyhow::Result<Option<Library>> {
        for library in libraries {
            for location in &library.location {
                if ev.file_path.starts_with(&location.path) {
                    return Ok(Some(library.clone()));
                }
            }
        }

        Ok(None)
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
    async fn process<'a>(&mut self, evs: &[&'a ScanEvent]) -> anyhow::Result<Vec<String>> {
        let libraries = self.libraries().await?;

        let mut succeeded = Vec::new();

        for ev in evs {
            if let Some(library) = self.in_library(&libraries, ev)? {
                match self.scan(ev, &library).await {
                    Ok(_) => succeeded.push(ev.id.clone()),
                    Err(e) => {
                        error!("failed to scan file '{}': {}", ev.file_path, e);
                    }
                };
            } else {
                error!("unable to find library for file: {}", ev.file_path);
            }
        }

        Ok(succeeded)
    }
}
