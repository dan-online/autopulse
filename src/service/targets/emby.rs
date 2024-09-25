use std::{fmt::Display, io::Cursor};

use crate::{db::models::ScanEvent, utils::settings::TargetProcess};
use reqwest::header;
use serde::{Deserialize, Serialize};
use struson::{
    json_path,
    reader::{JsonReader, JsonStreamReader},
};
use tracing::{debug, error};

#[derive(Clone, Deserialize)]
pub struct Emby {
    /// URL to the Jellyfin/Emby server
    pub url: String,
    /// API token for the Jellyfin/Emby server
    pub token: String,

    /// Metadata refresh mode (default: FullRefresh)
    #[serde(default)]
    pub metadata_refresh_mode: EmbyMetadataRefreshMode,
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

#[derive(Deserialize, Clone)]
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

        headers.insert("X-Emby-Token", self.token.parse().unwrap());
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
        let libraries: Vec<Library> = res.json().await?;

        Ok(libraries)
    }

    fn get_library(&self, libraries: &[Library], path: &str) -> Option<Library> {
        libraries
            .iter()
            .find(|lib| lib.locations.iter().any(|loc| path.starts_with(loc)))
            .cloned()
    }

    async fn get_item(&self, library: &Library, path: &str) -> anyhow::Result<Option<Item>> {
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

        if res.status().is_success() {
            Ok(())
        } else {
            let body = res.text().await?;
            Err(anyhow::anyhow!("unable to send scan: {}", body))
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

        if res.status().is_success() {
            Ok(())
        } else {
            let body = res.text().await?;
            Err(anyhow::anyhow!("unable to refresh item: {}", body))
        }
    }
}

impl TargetProcess for Emby {
    async fn process<'a>(&self, evs: &[&'a ScanEvent]) -> anyhow::Result<Vec<String>> {
        let libraries = self.libraries().await?;

        let mut succeded = Vec::new();

        let mut to_refresh = Vec::new();
        let mut to_scan = Vec::new();

        for ev in evs {
            if let Some(library) = self.get_library(&libraries, &ev.file_path) {
                let item = self.get_item(&library, &ev.file_path).await?;

                if let Some(item) = item {
                    to_refresh.push((*ev, item));
                } else {
                    to_scan.push(*ev);
                }
            } else {
                error!("unable to find library for file: {}", ev.file_path);
            }
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

        self.scan(&to_scan).await?;

        succeded.extend(to_scan.iter().map(|ev| ev.id.clone()));

        Ok(succeded)
    }
}
