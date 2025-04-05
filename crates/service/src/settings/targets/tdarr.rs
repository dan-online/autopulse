use crate::settings::rewrite::Rewrite;
use crate::settings::targets::TargetProcess;
use autopulse_database::models::ScanEvent;
use autopulse_utils::get_url;
use reqwest::header;
use serde::{Deserialize, Serialize};

use super::RequestBuilderPerform;

#[derive(Deserialize, Clone)]
pub struct Tdarr {
    /// URL to the Tdarr server
    pub url: String,
    /// Library ID for the Tdarr server
    pub db_id: String,
    /// Rewrite path for the file
    pub rewrite: Option<Rewrite>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[doc(hidden)]
struct ScanConfig {
    #[serde(rename = "dbID")]
    db_id: String,
    array_or_path: Vec<String>,
    mode: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[doc(hidden)]
struct Data {
    scan_config: ScanConfig,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[doc(hidden)]
struct Payload {
    data: Data,
}

impl Tdarr {
    fn get_client(&self) -> anyhow::Result<reqwest::Client> {
        let headers = header::HeaderMap::new();

        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .default_headers(headers)
            .build()
            .map_err(Into::into)
    }

    async fn scan(&self, evs: &[&ScanEvent]) -> anyhow::Result<()> {
        let client = self.get_client()?;

        let payload = Payload {
            data: Data {
                scan_config: ScanConfig {
                    db_id: self.db_id.clone(),
                    array_or_path: evs.iter().map(|ev| ev.get_path(&self.rewrite)).collect(),
                    mode: "scanFolderWatcher".to_string(),
                },
            },
        };

        let url = get_url(&self.url)?.join("/api/v2/scan-files")?;

        client
            .post(url)
            .header("content-type", "application/json")
            .json(&payload)
            .perform()
            .await
            .map(|_| ())
    }
}

impl TargetProcess for Tdarr {
    async fn process(&self, evs: &[&ScanEvent]) -> anyhow::Result<Vec<String>> {
        let mut succeeded = Vec::new();

        self.scan(evs).await?;

        succeeded.extend(evs.iter().map(|ev| ev.id.clone()));

        Ok(succeeded)
    }
}
