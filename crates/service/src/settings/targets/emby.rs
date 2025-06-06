use super::RequestBuilderPerform;
use crate::settings::rewrite::Rewrite;
use crate::settings::targets::TargetProcess;
use anyhow::Context;
use autopulse_database::models::ScanEvent;
use autopulse_utils::get_url;
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
    /// Metadata refresh mode (default: `FullRefresh`)
    #[serde(default)]
    pub metadata_refresh_mode: EmbyMetadataRefreshMode,
    /// Whether to try to refresh metadata for the item instead of scan (default: true)
    #[serde(default = "default_true")]
    pub refresh_metadata: bool,
    /// Rewrite path for the file
    pub rewrite: Option<Rewrite>,
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

        write!(f, "{mode}")
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

        headers.insert("X-Emby-Token", self.token.parse()?);
        headers.insert(
            "Authorization",
            format!("MediaBrowser Token=\"{}\"", self.token).parse()?,
        );
        headers.insert("Accept", "application/json".parse()?);

        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .default_headers(headers)
            .build()
            .map_err(Into::into)
    }

    async fn libraries(&self) -> anyhow::Result<Vec<Library>> {
        let client = self.get_client()?;
        let url = get_url(&self.url)?.join("Library/VirtualFolders")?;

        let res = client.get(url).perform().await?;

        Ok(res.json().await?)
    }

    fn get_libraries(&self, libraries: &[Library], path: &str) -> Vec<Library> {
        let ev_path = Path::new(path);
        let mut matched: Vec<Library> = vec![];

        for library in libraries {
            for location in &library.locations {
                let path = Path::new(location);

                if ev_path.starts_with(path) {
                    matched.push(library.clone());
                }
            }
        }

        matched
    }

    async fn _get_item(&self, library: &Library, path: &str) -> anyhow::Result<Option<Item>> {
        let client = self.get_client()?;
        let mut url = get_url(&self.url)?.join("Items")?;

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

        let res = client.get(url).perform().await?;

        // Possibly unneeded unless we can use streams
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

    fn fetch_items(
        &self,
        library: &Library,
    ) -> anyhow::Result<(
        UnboundedReceiver<Item>,
        tokio::task::JoinHandle<anyhow::Result<()>>,
    )> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let limit = 1000;

        let client = self.get_client()?;
        let mut url = get_url(&self.url)?.join("Items")?;

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

                let res = client.get(page_url).perform().await?;

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
        let (mut rx, handle) = self.fetch_items(library)?;

        let mut found_in_library = Vec::new();
        let mut not_found_in_library = events.clone();

        while let Some(item) = rx.recv().await {
            if let Some(ev) = events
                .iter()
                .find(|ev| item.path == Some(ev.get_path(&self.rewrite)))
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
        let url = get_url(&self.url)?.join("Library/Media/Updated")?;

        let updates = ev
            .iter()
            .map(|ev| UpdateRequest {
                path: ev.get_path(&self.rewrite),
                update_type: "Modified".to_string(),
            })
            .collect();

        let body = ScanPayload { updates };

        client
            .post(url)
            .header("Content-Type", "application/json")
            .json(&body)
            .perform()
            .await
            .map(|_| ())
    }

    async fn refresh_item(&self, item: &Item) -> anyhow::Result<()> {
        let client = self.get_client()?;
        let mut url = get_url(&self.url)?.join(&format!("Items/{}/Refresh", item.id))?;

        url.query_pairs_mut().append_pair(
            "MetadataRefreshMode",
            &self.metadata_refresh_mode.to_string(),
        );
        url.query_pairs_mut()
            .append_pair("ImageRefreshMode", &self.metadata_refresh_mode.to_string());
        url.query_pairs_mut()
            .append_pair("ReplaceAllMetadata", "true");
        url.query_pairs_mut().append_pair("Recursive", "true");

        // TODO: Possible options in future?
        url.query_pairs_mut()
            .append_pair("ReplaceAllImages", "false");
        url.query_pairs_mut()
            .append_pair("RegenerateTrickplay", "false");

        client.post(url).perform().await.map(|_| ())
    }
}

impl TargetProcess for Emby {
    async fn process(&self, evs: &[&ScanEvent]) -> anyhow::Result<Vec<String>> {
        let libraries = self
            .libraries()
            .await
            .context("failed to fetch libraries")?;

        let mut succeeded: HashMap<String, bool> = HashMap::new();

        let mut to_find = HashMap::new();
        let mut to_refresh = Vec::new();
        let mut to_scan = Vec::new();

        if self.refresh_metadata {
            for ev in evs {
                let ev_path = ev.get_path(&self.rewrite);

                let matched_libraries = self.get_libraries(&libraries, &ev_path);

                if matched_libraries.is_empty() {
                    error!("failed to find library for file: {}", ev_path);
                    continue;
                }

                for library in matched_libraries {
                    to_find.entry(library).or_insert_with(Vec::new).push(*ev);
                }
            }

            for (library, library_events) in to_find {
                let (found_in_library, not_found_in_library) = self
                    .get_items(&library, library_events.clone())
                    .await
                    .with_context(|| {
                        format!(
                            "failed to fetch items for library: {}",
                            library.name.clone()
                        )
                    })?;

                to_refresh.extend(found_in_library);
                to_scan.extend(not_found_in_library);
            }

            for (ev, item) in to_refresh {
                match self.refresh_item(&item).await {
                    Ok(()) => {
                        debug!("refreshed item: {}", item.id);
                        *succeeded.entry(ev.id.clone()).or_insert(true) &= true;
                    }
                    Err(e) => {
                        error!("failed to refresh item: {}", e);
                        succeeded.insert(ev.id.clone(), false);
                    }
                }
            }
        } else {
            to_scan.extend(evs.iter().copied());
        }

        if !to_scan.is_empty() {
            match self.scan(&to_scan).await {
                Ok(()) => {
                    for ev in &to_scan {
                        debug!("scanned file: {}", ev.file_path);

                        *succeeded.entry(ev.id.clone()).or_insert(true) &= true;
                    }
                }
                Err(e) => {
                    error!("failed to scan items: {}", e);

                    for ev in &to_scan {
                        succeeded.insert(ev.id.clone(), false);
                    }
                }
            }
        }

        Ok(succeeded
            .iter()
            .filter_map(|(k, v)| if *v { Some(k.clone()) } else { None })
            .collect())
    }
}
