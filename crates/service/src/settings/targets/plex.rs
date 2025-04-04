use crate::settings::rewrite::Rewrite;
use crate::settings::targets::TargetProcess;
use anyhow::Context;
use autopulse_database::models::ScanEvent;
use autopulse_utils::{get_url, what_is, PathType};
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
    let part_file_path = Path::new(part_file);
    let what_is_path = what_is(path);

    if what_is_path == PathType::Directory {
        part_file_path.starts_with(path)
    } else {
        part_file_path == path
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
        let mut matches: Vec<(usize, &Library)> = vec![];

        for library in libraries {
            for location in &library.location {
                let loc_path = Path::new(&location.path);
                if ev_path.starts_with(loc_path) {
                    matches.push((loc_path.components().count(), library));
                }
            }
        }

        matches.sort_by(|(len_a, _), (len_b, _)| len_b.cmp(len_a));

        // Return the most specific match
        matches
            .into_iter()
            .next()
            .map(|(_, library)| library.clone())
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

    fn get_search_term(&self, path: &str) -> anyhow::Result<String> {
        let path_obj = Path::new(path);
        let parts = path_obj.components().collect::<Vec<_>>();

        let mut chosen_part = path_obj
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("failed to convert path to string"))?
            .to_string()
            .replace("/", " ");

        for part in parts.iter().rev() {
            let part_str = part.as_os_str().to_string_lossy();

            if part_str.contains(".") || part_str.contains("Season") || part_str.is_empty() {
                continue;
            }

            chosen_part = part_str.to_string();
            break;
        }

        let chosen_part = chosen_part
            .split_whitespace()
            .filter(|&s| {
                ["(", ")", "[", "]", "{", "}"]
                    .iter()
                    .all(|&c| !s.contains(c))
            })
            .collect::<Vec<_>>()
            .join(" ");

        Ok(chosen_part)
    }

    async fn search_items(&self, _library: &Library, path: &str) -> anyhow::Result<Vec<Metadata>> {
        let client = self.get_client()?;
        // let mut url = get_url(&self.url)?.join(&format!("library/sections/{}/all", library.key))?;
        let mut url = get_url(&self.url)?.join("search")?;

        let mut results = vec![];

        let rel_path = path.to_string();

        trace!("searching for item with relative path: {}", rel_path);

        let search_term = self.get_search_term(&rel_path)?;

        trace!("searching for item with term: {}", search_term);

        url.query_pairs_mut()
            // .append_pair("title", search_term.as_str());
            .append_pair("query", search_term.as_str());

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

        if let Some(mut metadata) = lib.media_container.metadata.clone() {
            // sort episodes then movies to the front, then the rest
            metadata.sort_by(|a, b| {
                if a.t == "episode" && b.t != "episode" {
                    std::cmp::Ordering::Less
                } else if a.t != "episode" && b.t == "episode" {
                    std::cmp::Ordering::Greater
                } else if a.t == "movie" && b.t != "movie" && b.t != "episode" {
                    std::cmp::Ordering::Less
                } else if a.t != "movie" && a.t != "episode" && b.t == "movie" {
                    std::cmp::Ordering::Greater
                } else {
                    std::cmp::Ordering::Equal
                }
            });

            for item in metadata {
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

        trace!(
            "found {} out of {} items matching search",
            results.len(),
            lib.media_container.metadata.unwrap_or_default().len()
        );

        // if show + episode then remove duplicates
        results.dedup_by_key(|item| item.key.clone());

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

        let file_dir = (if matches!(what_is(ev_path), PathType::File) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_search_term() {
        let plex = Plex {
            url: String::new(),
            token: String::new(),
            refresh: false,
            analyze: false,
            rewrite: None,
        };

        // Test with a path that has a file name and season directory
        let path = "/media/TV Shows/Breaking Bad/Season 1/S01E01.mkv";
        assert_eq!(plex.get_search_term(path).unwrap(), "Breaking Bad");

        // Test with a path that has parentheses and brackets
        let path = "/media/Movies/The Matrix (1999) [1080p]/matrix.mkv";
        assert_eq!(plex.get_search_term(path).unwrap(), "The Matrix");

        // Test with a simple path
        let path = "/media/Movies/Inception/inception.mkv";
        assert_eq!(plex.get_search_term(path).unwrap(), "Inception");

        // Test with a directory path
        let path = "/media/TV Shows/Game of Thrones/Season 2";
        assert_eq!(plex.get_search_term(path).unwrap(), "Game of Thrones");

        // Test with no directory path
        let path = "/media/TV Shows/Game of Thrones";
        assert_eq!(plex.get_search_term(path).unwrap(), "Game of Thrones");

        // Test with multiple levels of season directories
        let path = "/media/TV Shows/Doctor Who/Season 10/Season 10 Part 2/S10E12.mkv";
        assert_eq!(plex.get_search_term(path).unwrap(), "Doctor Who");
    }

    #[test]
    fn test_get_library() {
        let plex = Plex {
            url: String::new(),
            token: String::new(),
            refresh: false,
            analyze: false,
            rewrite: None,
        };

        let libraries = [Library {
            title: "Movies".to_string(),
            key: "library_key_movies".to_string(),
            location: vec![Location {
                path: "/media/movies".to_string(),
            }],
        }];

        let path = "/media/movies/Inception.mkv";
        let library = plex.get_library(&libraries, path).unwrap();
        assert!(library.key == "library_key_movies");

        let nested_libraries = [
            Library {
                title: "Movies".to_string(),
                key: "library_key_movies".to_string(),
                location: vec![Location {
                    path: "/media/movies".to_string(),
                }],
            },
            Library {
                title: "Movies".to_string(),
                key: "library_key_movies_4k".to_string(),
                location: vec![Location {
                    path: "/media/movies/4k".to_string(),
                }],
            },
        ];

        let path = "/media/movies/4k/Inception.mkv";

        let library = plex.get_library(&nested_libraries, path).unwrap();
        assert!(library.key == "library_key_movies_4k");
    }
}
