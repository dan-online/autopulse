use crate::{db::models::ScanEvent, settings::target::TargetProcess};
use reqwest::header;
use serde::Deserialize;
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
    pub id: i64,
    pub key: String,
    pub duration: Option<i64>,
    pub file: String,
    pub size: i64,
    pub audio_profile: Option<String>,
    pub container: Option<String>,
    pub video_profile: Option<String>,
    pub has_thumbnail: Option<String>,
    pub has64bit_offsets: Option<bool>,
    pub optimized_for_streaming: Option<bool>,
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
        let url = url::Url::parse(&self.url)?
            .join("/library/sections")?
            .to_string();

        let res = client.get(&url).send().await?;
        let libraries: LibraryResponse = res.json().await?;

        Ok(libraries.media_container.directory.unwrap())
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

    async fn get_item(&self, library: &Library, path: &str) -> anyhow::Result<Option<Metadata>> {
        let client = self.get_client()?;
        let url = url::Url::parse(&self.url)?
            .join(&format!("/library/sections/{}/all", library.key))?
            .to_string();

        let res = client.get(&url).send().await?;
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
    //         url::Url::parse(&self.url)?.join(&format!("/library/sections/{}/refresh", library))?;

    //     url.query_pairs_mut().append_pair("force", "1");

    //     let res = client.get(url.to_string()).send().await?;

    //     if res.status().is_success() {
    //         Ok(())
    //     } else {
    //         let body = res.text().await?;
    //         Err(anyhow::anyhow!("unable to send refresh: {}", body))
    //     }
    // }

    // async fn analyze_library(&self, library: &str) -> anyhow::Result<()> {
    //     let client = self.get_client()?;
    //     let url =
    //         url::Url::parse(&self.url)?.join(&format!("/library/sections/{}/analyze", library))?;

    //     let res = client.put(url.to_string()).send().await?;

    //     if res.status().is_success() {
    //         Ok(())
    //     } else {
    //         let body = res.text().await?;
    //         Err(anyhow::anyhow!("unable to send analyze: {}", body))
    //     }
    // }

    async fn refresh_item(&self, key: &str) -> anyhow::Result<()> {
        let client = self.get_client()?;
        let url = url::Url::parse(&self.url)?.join(&format!("{}/refresh", key))?;

        let res = client.put(url.to_string()).send().await?;

        if res.status().is_success() {
            Ok(())
        } else {
            let body = res.text().await?;
            Err(anyhow::anyhow!("unable to send analyze: {}", body))
        }
    }

    async fn analyze_item(&self, key: &str) -> anyhow::Result<()> {
        let client = self.get_client()?;
        let url = url::Url::parse(&self.url)?.join(&format!("{}/analyze", key))?;

        let res = client.put(url.to_string()).send().await?;

        if res.status().is_success() {
            Ok(())
        } else {
            let body = res.text().await?;
            Err(anyhow::anyhow!("unable to send analyze: {}", body))
        }
    }

    async fn scan(&self, ev: &ScanEvent, library: &Library) -> anyhow::Result<()> {
        let client = self.get_client()?;
        let mut url = url::Url::parse(&self.url)?
            .join(&format!("/library/sections/{}/refresh", library.key))?;

        let file_dir = std::path::Path::new(&ev.file_path)
            .parent()
            .ok_or_else(|| anyhow::anyhow!("unable to get parent directory"))?
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("unable to convert path to string"))?;

        url.query_pairs_mut().append_pair("path", file_dir);

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
    async fn process(&self, evs: &[&ScanEvent]) -> anyhow::Result<Vec<String>> {
        let libraries = self.libraries().await?;

        let mut succeeded = Vec::new();

        for ev in evs {
            if let Some(library) = self.in_library(&libraries, ev)? {
                match self.scan(ev, &library).await {
                    Ok(_) => {
                        debug!("scanned file '{}'", ev.file_path);

                        let is_dir = std::path::Path::new(&ev.file_path).is_dir();

                        // Only analyze and refresh metadata for files
                        if !is_dir && (self.analyze || self.refresh) {
                            match self.get_item(&library, &ev.file_path).await {
                                Ok(Some(item)) => {
                                    trace!("found item for file '{}'", ev.file_path);

                                    let mut success = true;

                                    if self.analyze {
                                        match self.analyze_item(&item.key).await {
                                            Ok(_) => {
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
                                            Ok(_) => {
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
                                        "unable to find item for file: {}, leaving at scan",
                                        ev.file_path
                                    );
                                    succeeded.push(ev.id.clone());
                                }
                                Err(e) => {
                                    error!(
                                        "failed to get item for file '{}': {:?}",
                                        ev.file_path, e
                                    );
                                }
                            };
                        } else {
                            succeeded.push(ev.id.clone());
                        }
                    }
                    Err(e) => {
                        error!("failed to scan file '{}': {}", ev.file_path, e);
                    }
                }
            } else {
                error!("unable to find library for file: {}", ev.file_path);
            }
        }

        Ok(succeeded)
    }
}
