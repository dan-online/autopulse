use crate::{db::models::ScanEvent, settings::target::TargetProcess};
use anyhow::Context;
use reqwest::header;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Display, io::Cursor, path::Path};
use struson::{
    json_path,
    reader::{JsonReader, JsonStreamReader},
};
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::{debug, error};

#[doc(hidden)]
const fn default_true() -> bool {
    true
}

#[derive(Clone, Deserialize)]
pub struct Emby {
    /// URL to the Jellyfin/Emby server
    pub url: String,
    /// API token for the Jellyfin/Emby server
    pub token: String,

    /// Metadata refresh mode (default: FullRefresh)
    #[serde(default)]
    pub metadata_refresh_mode: EmbyMetadataRefreshMode,

    /// Whether to try to refresh metadata for the item instead of scan (default: true)
    #[serde(default = "default_true")]
    pub refresh_metadata: bool,
}

/// Metadata refresh mode for Jellyfin/Emby
#[derive(Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbyMetadataRefreshMode {
    /// `none`
    None,
    /// `validation_only`
    ValidationOnly,
    /// `default`
    Default,
    /// `full_refresh`
    FullRefresh,
}

impl Display for EmbyMetadataRefreshMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mode = match self {
            Self::None => "None",
            Self::ValidationOnly => "ValidationOnly",
            Self::Default => "Default",
            Self::FullRefresh => "FullRefresh",
        };

        write!(f, "{}", mode)
    }
}

impl Default for EmbyMetadataRefreshMode {
    fn default() -> Self {
        Self::FullRefresh
    }
}

#[derive(Deserialize, Clone, Eq, PartialEq, Hash)]
#[serde(rename_all = "PascalCase")]
#[doc(hidden)]
struct Library {
    #[allow(dead_code)]
    name: String,
    locations: Vec<String>,
    item_id: String,
    collection_type: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "PascalCase")]
#[doc(hidden)]
struct UpdateRequest {
    path: String,
    update_type: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "PascalCase")]
#[doc(hidden)]
struct ScanPayload {
    updates: Vec<UpdateRequest>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
#[doc(hidden)]
struct Item {
    id: String,
    path: Option<String>,
}

impl Emby {
    fn get_client(&self) -> anyhow::Result<reqwest::Client> {
        let mut headers = header::HeaderMap::new();

        headers.insert(
            "Authorzation",
            format!("MediaBrowser Token=\"{}\"", self.token)
                .parse()
                .unwrap(),
        );
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
            .join("/Library/VirtualFolders")?
            .to_string();

        let res = client.get(&url).send().await?;
        let status = res.status();

        if status.is_success() {
            Ok(res.json().await?)
        } else {
            let body = res.text().await?;

            Err(anyhow::anyhow!(
                "failed to fetch libraries: {} - {}",
                status.as_u16(),
                body
            ))
        }
    }

    fn get_library(&self, libraries: &[Library], path: &str) -> Option<Library> {
        let ev_path = Path::new(path);

        for library in libraries {
            for location in &library.locations {
                let path = Path::new(location);

                if ev_path.starts_with(path) {
                    return Some(library.clone());
                }
            }
        }

        None
    }

    async fn _get_item(&self, library: &Library, path: &str) -> anyhow::Result<Option<Item>> {
        let client = self.get_client()?;
        let mut url = url::Url::parse(&self.url)?.join("/Items")?;

        url.query_pairs_mut().append_pair("Recursive", "true");
        url.query_pairs_mut().append_pair("Fields", "Path");
        url.query_pairs_mut().append_pair("EnableImages", "false");
        if let Some(collection_type) = &library.collection_type {
            url.query_pairs_mut().append_pair(
                "IncludeItemTypes",
                match collection_type.as_str() {
                    "tvshows" => "Episode",
                    "books" => "Book",
                    "music" => "Audio",
                    "movie" => "VideoFile,Movie",
                    _ => "",
                },
            );
        }
        url.query_pairs_mut()
            .append_pair("ParentId", &library.item_id);
        url.query_pairs_mut()
            .append_pair("EnableTotalRecordCount", "false");

        let res = client.get(url.to_string()).send().await?;

        // Possibly uneeded unless we can use streams
        let bytes = res.bytes().await?;

        let mut json_reader = JsonStreamReader::new(Cursor::new(bytes));

        json_reader.seek_to(&json_path!["Items"])?;
        json_reader.begin_array()?;

        while json_reader.has_next()? {
            let item: Item = json_reader.deserialize_next()?;

            if item.path == Some(path.to_owned()) {
                return Ok(Some(item));
            }
        }

        Ok(None)
    }

    async fn fetch_items(
        &self,
        library: &Library,
    ) -> anyhow::Result<(
        UnboundedReceiver<Item>,
        tokio::task::JoinHandle<anyhow::Result<()>>,
    )> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let limit = 1000;

        let client = self.get_client()?;
        let mut url = url::Url::parse(&self.url)?.join("/Items")?;

        url.query_pairs_mut().append_pair("Recursive", "true");
        url.query_pairs_mut().append_pair("Fields", "Path");
        url.query_pairs_mut().append_pair("EnableImages", "false");
        url.query_pairs_mut()
            .append_pair("ParentId", &library.item_id);
        url.query_pairs_mut()
            .append_pair("EnableTotalRecordCount", "false");
        url.query_pairs_mut()
            .append_pair("Limit", &limit.to_string());
        if let Some(collection_type) = &library.collection_type {
            url.query_pairs_mut().append_pair(
                "IncludeItemTypes",
                match collection_type.as_str() {
                    "tvshows" => "Episode",
                    "books" => "Book",
                    "music" => "Audio",
                    "movie" => "VideoFile,Movie",
                    _ => "",
                },
            );
        }

        let handle = tokio::spawn(async move {
            let mut page = 0;

            loop {
                let mut page_url = url.clone();
                page_url
                    .query_pairs_mut()
                    .append_pair("StartIndex", &(page * limit).to_string());

                let res = client.get(page_url.to_string()).send().await?;

                let bytes = res.bytes().await?;

                let mut json_reader = JsonStreamReader::new(Cursor::new(bytes));

                json_reader.seek_to(&json_path!["Items"])?;
                json_reader.begin_array()?;

                let mut found_items_count = 0;

                while json_reader.has_next()? {
                    let item: Item = json_reader.deserialize_next()?;

                    tx.send(item)?;

                    found_items_count += 1;
                }

                if found_items_count < limit {
                    break;
                }

                page += 1;
            }

            drop(tx);

            Ok(())
        });

        Ok((rx, handle))
    }

    async fn get_items<'a>(
        &self,
        library: &Library,
        events: Vec<&'a ScanEvent>,
    ) -> anyhow::Result<(Vec<(&'a ScanEvent, Item)>, Vec<&'a ScanEvent>)> {
        let (mut rx, handle) = self.fetch_items(library).await?;

        let mut found_in_library = Vec::new();
        let mut not_found_in_library = events.to_vec();

        while let Some(item) = rx.recv().await {
            if let Some(ev) = events
                .iter()
                .find(|ev| item.path == Some(ev.file_path.clone()))
            {
                found_in_library.push((*ev, item.clone()));
                not_found_in_library.retain(|&e| e.id != ev.id);

                if not_found_in_library.is_empty() {
                    break;
                }
            }
        }

        handle.abort();

        Ok((found_in_library, not_found_in_library))
    }

    // not as effective as refreshing the item, but good enough
    async fn scan(&self, ev: &[&ScanEvent]) -> anyhow::Result<()> {
        let client = self.get_client()?;
        let url = url::Url::parse(&self.url)?
            .join("/Library/Media/Updated")?
            .to_string();

        let updates = ev
            .iter()
            .map(|ev| UpdateRequest {
                path: ev.file_path.clone(),
                update_type: "Modified".to_string(),
            })
            .collect();

        let body = ScanPayload { updates };

        let res = client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;
        let status = res.status();

        if status.is_success() {
            Ok(())
        } else {
            let body = res.text().await?;

            Err(anyhow::anyhow!(
                "failed to send scan: {} - {}",
                status.as_u16(),
                body
            ))
        }
    }

    async fn refresh_item(&self, item: &Item) -> anyhow::Result<()> {
        let client = self.get_client()?;
        let mut url = url::Url::parse(&self.url)?.join(&format!("/Items/{}/Refresh", item.id))?;

        url.query_pairs_mut().append_pair(
            "metadataRefreshMode",
            &self.metadata_refresh_mode.to_string(),
        );

        let res = client.post(url.to_string()).send().await?;
        let status = res.status();

        if status.is_success() {
            Ok(())
        } else {
            let body = res.text().await?;

            Err(anyhow::anyhow!(
                "failed to refresh item: {} - {}",
                status.as_u16(),
                body
            ))
        }
    }
}

impl TargetProcess for Emby {
    async fn process(&self, evs: &[&ScanEvent]) -> anyhow::Result<Vec<String>> {
        let libraries = self
            .libraries()
            .await
            .context("failed to fetch libraries")?;

        let mut succeded = Vec::new();

        let mut to_find = HashMap::new();
        let mut to_refresh = Vec::new();
        let mut to_scan = Vec::new();

        if self.refresh_metadata {
            for ev in evs {
                if let Some(library) = self.get_library(&libraries, &ev.file_path) {
                    to_find.entry(library).or_insert_with(Vec::new).push(*ev);
                } else {
                    error!("failed to find library for file: {}", ev.file_path);
                }
            }

            for (library, library_events) in to_find {
                let (found_in_library, not_found_in_library) = self
                    .get_items(&library, library_events.clone())
                    .await
                    .with_context(|| {
                        format!(
                            "failed to fetch items for library: {}",
                            library.name.to_owned()
                        )
                    })?;

                to_refresh.extend(found_in_library);
                to_scan.extend(not_found_in_library);
            }

            for (ev, item) in to_refresh {
                match self.refresh_item(&item).await {
                    Ok(_) => {
                        debug!("refreshed item: {}", item.path.unwrap());
                        succeded.push(ev.id.clone());
                    }
                    Err(e) => {
                        error!("failed to refresh item: {}", e);
                    }
                }
            }
        } else {
            to_scan.extend(evs.iter().copied());
        }

        if !to_scan.is_empty() {
            self.scan(&to_scan).await.context("failed to scan files")?;

            for file in to_scan.iter() {
                debug!("scanned file: {}", file.file_path);
            }
        }

        succeded.extend(to_scan.iter().map(|ev| ev.id.clone()));

        Ok(succeded)
    }
}
