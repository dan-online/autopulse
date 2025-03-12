use crate::settings::rewrite::Rewrite;
use crate::settings::targets::TargetProcess;
use anyhow::Context;
use autopulse_database::models::ScanEvent;
use autopulse_utils::get_url;
use reqwest::header;
use serde::Deserialize;
use std::path::Path;
use tracing::{debug, error, trace};

#[derive(Deserialize, Clone)]
pub struct Plex {
    /// URL to the Plex server
    pub url: String,
    /// API token for the Plex server
    pub token: String,
    /// Whether to refresh metadata of the file (default: false)
    #[serde(default)]
    pub refresh: bool,
    /// Whether to analyze the file (default: false)
    #[serde(default)]
    pub analyze: bool,
    /// Rewrite path for the file
    pub rewrite: Option<Rewrite>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Media {
    #[serde(rename = "Part")]
    pub part: Vec<Part>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Part {
    // pub id: i64,
    pub key: String,
    // pub duration: Option<i64>,
    pub file: String,
    // pub size: i64,
    // pub audio_profile: Option<String>,
    // pub container: Option<String>,
    // pub video_profile: Option<String>,
    // pub has_thumbnail: Option<String>,
    // pub has64bit_offsets: Option<bool>,
    // pub optimized_for_streaming: Option<bool>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub key: String,
    #[serde(rename = "Media")]
    pub media: Option<Vec<Media>>,
}

#[doc(hidden)]
#[derive(Deserialize, Clone, Debug)]
struct Location {
    path: String,
}

#[doc(hidden)]
#[derive(Deserialize, Clone, Debug)]
struct Library {
    key: String,
    #[serde(rename = "Location")]
    location: Vec<Location>,
}

#[doc(hidden)]
#[derive(Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
struct MediaContainer {
    directory: Option<Vec<Library>>,
    metadata: Option<Vec<Metadata>>,
}

#[doc(hidden)]
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
        let url = get_url(&self.url)?.join("library/sections")?.to_string();

        let res = client.get(&url).send().await?;
        let status = res.status();

        if status.is_success() {
            let libraries: LibraryResponse = res.json().await?;

            Ok(libraries.media_container.directory.unwrap())
        } else {
            let body = res.text().await?;

            Err(anyhow::anyhow!(
                "failed to get libraries: {} - {}",
                status.as_u16(),
                body
            ))
        }
    }

    fn get_library(&self, libraries: &[Library], path: &str) -> Option<Library> {
        let ev_path = Path::new(path);

        for library in libraries {
            for location in &library.location {
                let path = Path::new(&location.path);

                if ev_path.starts_with(path) {
                    return Some(library.clone());
                }
            }
        }

        None
    }

    // TODO: X-Plex-Media-Container-Size
    // TODO: Change to get_items
    async fn get_item(&self, library: &Library, path: &str) -> anyhow::Result<Option<Metadata>> {
        let client = self.get_client()?;
        let url = get_url(&self.url)?
            .join(&format!("library/sections/{}/all", library.key))?
            .to_string();

        let res = client.get(&url).send().await?;
        let status = res.status();

        if !status.is_success() {
            let body = res.text().await?;

            return Err(anyhow::anyhow!(
                "failed to get library: {} - {}",
                status.as_u16(),
                body
            ));
        }

        let lib: LibraryResponse = res.json().await?;

        for item in lib.media_container.metadata.unwrap_or_default() {
            if item
                .media
                .clone()
                .unwrap_or_default()
                .iter()
                .any(|media| media.part.iter().any(|part| part.file == path))
            {
                return Ok(Some(item));
            }
        }

        Ok(None)
    }

    // async fn refresh_library(&self, library: &str) -> anyhow::Result<()> {
    //     let client = self.get_client()?;
    //     let mut url =
    //         get_url(&self.url)?.join(&format!("/library/sections/{}/refresh", library))?;

    //     url.query_pairs_mut().append_pair("force", "1");

    //     let res = client.get(url.to_string()).send().await?;

    //     if res.status().is_success() {
    //         Ok(())
    //     } else {
    //         let body = res.text().await?;
    //         Err(anyhow::anyhow!("failed to send refresh: {}", body))
    //     }
    // }

    // async fn analyze_library(&self, library: &str) -> anyhow::Result<()> {
    //     let client = self.get_client()?;
    //     let url =
    //         get_url(&self.url)?.join(&format!("/library/sections/{}/analyze", library))?;

    //     let res = client.put(url.to_string()).send().await?;

    //     if res.status().is_success() {
    //         Ok(())
    //     } else {
    //         let body = res.text().await?;
    //         Err(anyhow::anyhow!("failed to send analyze: {}", body))
    //     }
    // }

    async fn refresh_item(&self, key: &str) -> anyhow::Result<()> {
        let client = self.get_client()?;
        let url = get_url(&self.url)?.join(&format!("{key}/refresh"))?;

        let res = client.put(url.to_string()).send().await?;

        if res.status().is_success() {
            Ok(())
        } else {
            let body = res.text().await?;
            Err(anyhow::anyhow!("failed to send analyze: {}", body))
        }
    }

    async fn analyze_item(&self, key: &str) -> anyhow::Result<()> {
        let client = self.get_client()?;
        let url = get_url(&self.url)?.join(&format!("{key}/analyze"))?;

        let res = client.put(url.to_string()).send().await?;

        if res.status().is_success() {
            Ok(())
        } else {
            let body = res.text().await?;
            Err(anyhow::anyhow!("failed to send analyze: {}", body))
        }
    }

    async fn scan(&self, ev: &ScanEvent, library: &Library) -> anyhow::Result<()> {
        let client = self.get_client()?;
        let mut url =
            get_url(&self.url)?.join(&format!("library/sections/{}/refresh", library.key))?;

        let ev_path = ev.get_path(&self.rewrite);
        let ev_path = Path::new(&ev_path);

        let file_dir = (if ev_path.is_dir() {
            ev_path
        } else {
            ev_path
                .parent()
                .ok_or_else(|| anyhow::anyhow!("failed to get parent directory"))?
        })
        .to_str()
        .ok_or(anyhow::anyhow!("failed to convert path to string"))?;

        url.query_pairs_mut().append_pair("path", file_dir);

        let res = client.get(url.to_string()).send().await?;

        if res.status().is_success() {
            Ok(())
        } else {
            let body = res.text().await?;
            Err(anyhow::anyhow!("failed to send scan: {}", body))
        }
    }
}

impl TargetProcess for Plex {
    async fn process(&self, evs: &[&ScanEvent]) -> anyhow::Result<Vec<String>> {
        let libraries = self.libraries().await.context("failed to get libraries")?;

        let mut succeeded = Vec::new();

        for ev in evs {
            let ev_path = ev.get_path(&self.rewrite);

            if let Some(library) = self.get_library(&libraries, &ev_path) {
                match self.scan(ev, &library).await {
                    Ok(()) => {
                        debug!("scanned file '{}'", ev_path);

                        let is_dir = Path::new(&ev_path).is_dir();

                        // Only analyze and refresh metadata for files
                        if self.analyze || self.refresh {
                            match self.get_item(&library, &ev_path).await {
                                Ok(Some(item)) => {
                                    trace!("found item for file '{}'", ev_path);

                                    let mut success = true;

                                    if self.analyze {
                                        match self.analyze_item(&item.key).await {
                                            Ok(()) => {
                                                debug!("analyzed metadata '{}'", item.key);
                                            }
                                            Err(e) => {
                                                error!(
                                                    "failed to analyze library '{}': {}",
                                                    library.key, e
                                                );

                                                success = false;
                                            }
                                        }
                                    }

                                    if self.refresh {
                                        match self.refresh_item(&item.key).await {
                                            Ok(()) => {
                                                debug!("refreshed metadata '{}'", item.key);
                                            }
                                            Err(e) => {
                                                error!(
                                                    "failed to refresh library '{}': {}",
                                                    library.key, e
                                                );

                                                success = false;
                                            }
                                        }
                                    }

                                    if success {
                                        succeeded.push(ev.id.clone());
                                    }
                                }
                                Ok(None) => {
                                    trace!(
                                        "failed to find item for file: {}, leaving at scan",
                                        ev_path
                                    );
                                    succeeded.push(ev.id.clone());
                                }
                                Err(e) => {
                                    error!("failed to get item for file '{}': {:?}", ev_path, e);
                                }
                            };
                        } else {
                            succeeded.push(ev.id.clone());
                        }
                    }
                    Err(e) => {
                        error!("failed to scan file '{}': {}", ev_path, e);
                    }
                }
            } else {
                error!("failed to find library for file: {}", ev_path);
            }
        }

        Ok(succeeded)
    }
}
