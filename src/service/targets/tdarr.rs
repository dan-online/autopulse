use crate::{
    db::models::ScanEvent,
    settings::{rewrite::Rewrite, target::TargetProcess},
    utils::get_url::get_url,
};
use reqwest::header;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Clone)]
pub struct Tdarr {
    /// URL to the Tdarr server
    url: String,
    /// Library ID for the Tdarr server
    db_id: String,
    /// Rewrite path for the file
    rewrite: Option<Rewrite>,
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

        let url = get_url(&self.url)?.join("/api/v2/scan-files")?.to_string();

        let res = client
            .post(&url)
            .header("content-type", "application/json")
            .json(&payload)
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
}

impl TargetProcess for Tdarr {
    async fn process(&self, evs: &[&ScanEvent]) -> anyhow::Result<Vec<String>> {
        let mut succeeded = Vec::new();

        self.scan(evs).await?;

        succeeded.extend(evs.iter().map(|ev| ev.id.clone()));

        Ok(succeeded)
    }
}
