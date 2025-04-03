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

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Media {
    #[serde(rename = "Part")]
    pub part: Vec<Part>,
}

#[derive(Deserialize, Clone, Debug)]
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

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub key: String,
    #[serde(rename = "Media")]
    pub media: Option<Vec<Media>>,
    #[serde(rename = "type")]
    pub t: String,
}

#[doc(hidden)]
#[derive(Deserialize, Clone, Debug)]
struct Location {
    path: String,
}

#[doc(hidden)]
#[derive(Deserialize, Clone, Debug)]
struct Library {
    title: String,
    key: String,
    #[serde(rename = "Location")]
    location: Vec<Location>,
}

#[doc(hidden)]
#[derive(Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
struct LibraryMediaContainer {
    directory: Option<Vec<Library>>,
    metadata: Option<Vec<Metadata>>,
}

#[doc(hidden)]
#[derive(Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
struct LibraryResponse {
    media_container: LibraryMediaContainer,
}

fn path_matches(part_file: &str, path: &Path) -> bool {
    if path.is_dir() {
        Path::new(part_file).parent() == Some(path)
    } else {
        Path::new(part_file) == path
    }
}

fn has_matching_media(media: &[Media], path: &Path) -> bool {
    media
        .iter()
        .any(|m| m.part.iter().any(|p| path_matches(&p.file, path)))
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

    async fn get_episodes(&self, key: &str) -> anyhow::Result<LibraryResponse> {
        let client = self.get_client()?;
        // remove last part of the key
        let key = key
            .split('/')
            .collect::<Vec<_>>()
            .into_iter()
            .take(key.split('/').count() - 1)
            .collect::<Vec<_>>()
            .join("/");

        let url = get_url(&self.url)?.join(&format!("{key}/allLeaves"))?;

        let res = client.get(url.to_string()).send().await?;

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
        Ok(lib)
    }

    async fn search_items(&self, library: &Library, path: &str) -> anyhow::Result<Vec<Metadata>> {
        // GET /search?query=<title>
        // get title from path by removing the prefix from the library and the file name
        // /mnt/media/anime/tvshows/Attack on Titan/Season 01/Attack on Titan (2013) - S01E01 - To You in 2,000 Years The Fall of Shiganshina 1 [Subs Bluray-1080p][Opus 2.0][AV1]-TatakaeFuniSubs.mkv
        // -/mnt/media/anime/tvshows/
        // Attack on Titan/Season 01/Attack on Titan (2013) - S01E01 - To You in 2,000 Years The Fall of Shiganshina 1 [Subs Bluray-1080p][Opus 2.0][AV1]-TatakaeFuniSubs.mkv
        // -Attack on Titan (2013) - S01E01 - To You in 2,000 Years The Fall of Shiganshina 1 [Subs Bluray-1080p][Opus 2.0][AV1]-TatakaeFuniSubs.mkv
        // Attack on Titan/Season 01
        // -Season 01
        // Attack on Titan (2013)
        // -(2013)
        let client = self.get_client()?;
        let mut url = get_url(&self.url)?.join(&format!("library/sections/{}/all", library.key))?;

        // Extract the relative path from the library root
        let rel_path = path
            .strip_prefix(&library.location[0].path)
            .unwrap_or(path)
            .trim_matches('/');

        // Split the path components
        let path_parts: Vec<&str> = rel_path.split('/').collect();

        // If we have a path that might point to a file
        if !path_parts.is_empty() {
            // Get the parent folder name as a search term (usually show or movie name)
            let search_term = if path_parts.len() > 1 {
                path_parts[0]
            } else {
                // If there's just one part, it might be a file or folder
                let path_obj = Path::new(path);
                if path_obj.is_dir() {
                    rel_path
                } else {
                    path_obj
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or(rel_path)
                }
            };

            trace!("searching for item with term: {}", search_term);

            // Add search query parameter
            url.query_pairs_mut().append_pair("title", search_term);
        }

        let res = client.get(url.to_string()).send().await?;

        let status = res.status();
        if !status.is_success() {
            let body = res.text().await?;
            return Err(anyhow::anyhow!(
                "Failed to search items: {} - {}",
                status.as_u16(),
                body
            ));
        }

        let lib: LibraryResponse = res.json().await?;

        let path_obj = Path::new(path);

        let mut results = vec![];

        // Process search results
        if let Some(metadata) = lib.media_container.metadata {
            for item in metadata {
                // For shows, we need to also get the episodes
                if item.t == "show" {
                    let episodes = self.get_episodes(&item.key).await?;

                    if let Some(episode_metadata) = episodes.media_container.metadata {
                        for episode in episode_metadata {
                            if let Some(media) = &episode.media {
                                if has_matching_media(media, path_obj) {
                                    results.push(episode.clone());
                                }
                            }
                        }
                    }
                } else if let Some(media) = &item.media {
                    // For movies and other content types
                    if has_matching_media(media, path_obj) {
                        results.push(item.clone());
                    }
                }
            }
        }

        trace!("found {} items matching search", results.len());
        Ok(results)
    }

    async fn _get_items(&self, library: &Library, path: &str) -> anyhow::Result<Vec<Metadata>> {
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

        let path = Path::new(path);

        let mut parts = vec![];

        // TODO: Reduce the amount of data needed to be searched
        for item in lib.media_container.metadata.unwrap_or_default() {
            match item.t.as_str() {
                "show" => {
                    let episodes = self.get_episodes(&item.key).await?;

                    for episode in episodes.media_container.metadata.unwrap_or_default() {
                        if let Some(media) = &episode.media {
                            if has_matching_media(media, path) {
                                parts.push(episode.clone());
                            }
                        }
                    }
                }
                _ => {
                    if let Some(media) = &item.media {
                        if has_matching_media(media, path) {
                            parts.push(item.clone());
                        }
                    }
                }
            }
        }

        Ok(parts)
    }

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
        .ok_or_else(|| anyhow::anyhow!("failed to convert path to string"))?;

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
                trace!("found library '{}' for {ev_path}", library.title);
                match self.scan(ev, &library).await {
                    Ok(()) => {
                        debug!("scanned '{}'", ev_path);

                        if self.analyze || self.refresh {
                            match self.search_items(&library, &ev_path).await {
                                Ok(items) => {
                                    if items.is_empty() {
                                        trace!(
                                            "failed to find items for file: '{}', leaving at scan",
                                            ev_path
                                        );
                                        succeeded.push(ev.id.clone());
                                    } else {
                                        trace!("found items for file '{}'", ev_path);

                                        let mut all_success = true;

                                        for item in items {
                                            let mut item_success = true;

                                            if self.refresh {
                                                match self.refresh_item(&item.key).await {
                                                    Ok(()) => {
                                                        debug!("refreshed metadata '{}'", item.key);
                                                    }
                                                    Err(e) => {
                                                        error!(
                                                        "failed to refresh metadata for '{}': {}",
                                                        item.key, e
                                                    );
                                                        item_success = false;
                                                    }
                                                }
                                            }

                                            if self.analyze {
                                                match self.analyze_item(&item.key).await {
                                                    Ok(()) => {
                                                        debug!("analyzed metadata '{}'", item.key);
                                                    }
                                                    Err(e) => {
                                                        error!(
                                                        "failed to analyze metadata for '{}': {}",
                                                        item.key, e
                                                    );
                                                        item_success = false;
                                                    }
                                                }
                                            }

                                            if !item_success {
                                                all_success = false;
                                            }
                                        }

                                        if all_success {
                                            succeeded.push(ev.id.clone());
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("failed to get items for '{}': {:?}", ev_path, e);
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
