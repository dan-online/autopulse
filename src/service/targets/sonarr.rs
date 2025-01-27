use crate::{db::models::ScanEvent, settings::target::TargetProcess, utils::get_url::get_url};
use reqwest::header;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::Path};
use tracing::error;

#[derive(Deserialize, Clone)]
pub struct Sonarr {
    /// URL to the Plex server
    pub url: String,
    /// API token for the Plex server
    pub token: String,
}

#[derive(Deserialize, Debug)]
struct SonarrSeries {
    id: i64,
    path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RefreshSeries {
    series_id: i64,
}

#[derive(Serialize)]
#[serde(tag = "name")]
#[serde(rename_all = "PascalCase")]
enum Command {
    RefreshSeries(RefreshSeries),
}

impl Sonarr {
    fn get_client(&self) -> anyhow::Result<reqwest::Client> {
        let mut headers = header::HeaderMap::new();

        headers.insert("X-Api-Key", self.token.parse().unwrap());
        headers.insert("Accept", "application/json".parse().unwrap());

        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .default_headers(headers)
            .build()
            .map_err(Into::into)
    }

    async fn get_series(&self, evs: &[&ScanEvent]) -> anyhow::Result<Vec<(i64, Vec<String>)>> {
        let client = self.get_client().unwrap();
        let url = get_url(&self.url)?.join("api/v3/series")?.to_string();
        let mut to_be_refreshed: HashMap<i64, Vec<String>> = HashMap::new();

        let res = client.get(&url).send().await?;
        let status = res.status();

        if !status.is_success() {
            let body = res.text().await?;

            return Err(anyhow::anyhow!(
                "failed to get series from Sonarr: {} - {}",
                status,
                body
            ));
        }

        let series = res.json::<Vec<SonarrSeries>>().await?;

        for ev in evs {
            let path = Path::new(&ev.file_path);

            for s in &series {
                let series_path = Path::new(&s.path);
                if path.starts_with(series_path) {
                    to_be_refreshed.entry(s.id).or_default().push(ev.id.clone());
                    break;
                }
            }
        }

        Ok(to_be_refreshed.into_iter().collect())
    }

    async fn refresh_series(&self, series_id: i64) -> anyhow::Result<()> {
        let client = self.get_client().unwrap();
        let url = get_url(&self.url)?.join("api/v3/command")?.to_string();
        let payload = Command::RefreshSeries(RefreshSeries { series_id });

        let res = client.post(&url).json(&payload).send().await?;
        let status = res.status();

        if !status.is_success() {
            let body = res.text().await?;

            return Err(anyhow::anyhow!(
                "failed to refresh series in Sonarr: {} - {}",
                status,
                body
            ));
        }

        Ok(())
    }
}

impl TargetProcess for Sonarr {
    async fn process(&self, evs: &[&ScanEvent]) -> anyhow::Result<Vec<String>> {
        let mut succeeded = Vec::new();

        let series = self.get_series(evs).await?;

        println!("{:?}", series);

        for (series_id, ev_ids) in series {
            match self.refresh_series(series_id).await {
                Ok(_) => {
                    succeeded.extend(ev_ids);
                }
                Err(e) => {
                    error!("failed to refresh series: {}", e);
                }
            }
        }

        Ok(succeeded)
    }
}
