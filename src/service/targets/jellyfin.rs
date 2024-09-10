use std::collections::HashMap;

use crate::{db::models::ScanEvent, utils::settings::TargetProcess};
use reqwest::header;
use serde::{Deserialize, Serialize};
use tracing::{debug, error};

#[derive(Clone, Debug, Deserialize)]
pub struct Jellyfin {
    pub url: String,
    pub token: String,
    #[serde(skip)]
    items_cache: HashMap<String, Item>,
    #[serde(skip)]
    last_cache: chrono::DateTime<chrono::Utc>,
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

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "PascalCase")]
struct Item {
    id: String,
    path: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "PascalCase")]
struct ItemsResponse {
    items: Vec<Item>,
}

impl Jellyfin {
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

    fn in_library(&self, libraries: &[Library], path: &str) -> bool {
        libraries
            .iter()
            .any(|lib| lib.locations.iter().any(|loc| path.starts_with(loc)))
    }

    async fn get_items(&mut self) -> anyhow::Result<&HashMap<String, Item>> {
        if chrono::Utc::now() - self.last_cache < chrono::Duration::seconds(10) {
            return Ok(&self.items_cache);
        }

        let client = self.get_client()?;
        let mut url = url::Url::parse(&self.url)?.join("/Items")?;

        url.query_pairs_mut().append_pair("Recursive", "true");
        url.query_pairs_mut().append_pair("Fields", "Path");
        url.query_pairs_mut().append_pair("EnableImages", "false");
        url.query_pairs_mut()
            .append_pair("LocationTypes", "FileSystem");
        url.query_pairs_mut()
            // TODO: Use the trigger type to reduce the data needed
            .append_pair("MediaTypes", "Video,Audio,Book");
        url.query_pairs_mut().append_pair("Filters", "IsNotFolder");
        url.query_pairs_mut()
            .append_pair("EnableTotalRecordCount", "false");

        let res = client.get(url.to_string()).send().await?;

        let res = res.json::<ItemsResponse>().await?;

        self.items_cache = res
            .items
            .iter()
            .filter_map(|item| item.path.clone().map(|path| (path, item.clone())))
            .collect();

        self.last_cache = chrono::Utc::now();

        Ok(&self.items_cache)
    }

    // sadly this is quite memory intensive, maybe a stream option is possible
    async fn find_item(&mut self, path: &str) -> anyhow::Result<Option<Item>> {
        let items = self.get_items().await?;

        Ok(items.get(path).cloned())
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

        url.query_pairs_mut()
            .append_pair("metadataRefreshMode", "FullRefresh");

        let res = client.post(url.to_string()).send().await?;

        if res.status().is_success() {
            Ok(())
        } else {
            let body = res.text().await?;
            Err(anyhow::anyhow!("unable to refresh item: {}", body))
        }
    }
}

impl TargetProcess for Jellyfin {
    async fn process<'a>(&mut self, evs: &[&'a ScanEvent]) -> anyhow::Result<Vec<String>> {
        let libraries = self.libraries().await?;

        let mut succeded = Vec::new();

        let mut to_refresh = Vec::new();
        let mut to_scan = Vec::new();

        for ev in evs {
            if !self.in_library(&libraries, &ev.file_path) {
                error!("unable to find library for file: {}", ev.file_path);

                continue;
            }

            let item = self.find_item(&ev.file_path).await?;

            if let Some(item) = item {
                to_refresh.push((*ev, item));
            } else {
                to_scan.push(*ev);
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
