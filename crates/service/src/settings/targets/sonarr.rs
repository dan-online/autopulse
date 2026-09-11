use crate::settings::path_filter::PathFilter;
use crate::settings::rewrite::Rewrite;
use crate::settings::targets::TargetProcess;
use autopulse_database::models::ScanEvent;
use autopulse_utils::{get_url, RuntimePath};
use reqwest::header;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::error;

use super::{Request, RequestBuilderPerform};

#[derive(Serialize, Deserialize, Clone)]
pub struct Sonarr {
    /// URL to the Sonarr server
    pub url: String,
    /// API token for the Sonarr server
    pub token: String,
    /// Rewrite path for the file
    pub rewrite: Option<Rewrite>,
    /// Path filter matched against the target-rewritten path.
    #[serde(default)]
    pub filter: PathFilter,
    /// HTTP request options
    #[serde(default)]
    pub request: Request,
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

fn matching_series_id(path: &str, series: &[SonarrSeries]) -> Option<i64> {
    let event = RuntimePath::new(path);
    series
        .iter()
        .find(|candidate| event.starts_with(RuntimePath::new(&candidate.path)))
        .map(|candidate| candidate.id)
}

impl Sonarr {
    fn get_client(&self) -> anyhow::Result<reqwest::Client> {
        let mut headers = header::HeaderMap::new();

        headers.insert("X-Api-Key", self.token.parse()?);
        headers.insert("Accept", "application/json".parse()?);

        self.request
            .client_builder(headers)
            .build()
            .map_err(Into::into)
    }

    async fn get_series(&self, evs: &[&ScanEvent]) -> anyhow::Result<Vec<(i64, Vec<String>)>> {
        let client = self.get_client()?;
        let url = get_url(&self.url)?.join("api/v3/series")?;
        let mut to_be_refreshed: HashMap<i64, Vec<String>> = HashMap::new();

        let res = client.get(url).perform().await?;

        let series = res.json::<Vec<SonarrSeries>>().await?;

        for ev in evs {
            let ev_path = ev.get_path(&self.rewrite);

            if let Some(series_id) = matching_series_id(&ev_path, &series) {
                to_be_refreshed
                    .entry(series_id)
                    .or_default()
                    .push(ev.id.clone());
            }
        }

        Ok(to_be_refreshed.into_iter().collect())
    }

    async fn refresh_series(&self, series_id: i64) -> anyhow::Result<()> {
        let client = self.get_client()?;
        let url = get_url(&self.url)?.join("api/v3/command")?;
        let payload = Command::RefreshSeries(RefreshSeries { series_id });

        client.post(url).json(&payload).perform().await.map(|_| ())
    }
}

impl TargetProcess for Sonarr {
    async fn process(&self, evs: &[&ScanEvent]) -> anyhow::Result<Vec<String>> {
        let mut succeeded = Vec::new();

        let series = self.get_series(evs).await?;

        for (series_id, ev_ids) in series {
            match self.refresh_series(series_id).await {
                Ok(()) => {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_windows_series_case_insensitively_by_components() {
        let series = vec![SonarrSeries {
            id: 42,
            path: r"D:\TV\Breaking Bad".to_string(),
        }];

        assert_eq!(
            matching_series_id(r"d:\tv\breaking bad\Season 1\S01E01.mkv", &series),
            Some(42)
        );
    }

    #[test]
    fn rejects_series_text_prefix_and_mixed_flavor() {
        let windows = vec![SonarrSeries {
            id: 42,
            path: r"D:\TV\Show".to_string(),
        }];
        let unix = vec![SonarrSeries {
            id: 7,
            path: "/tv/Show".to_string(),
        }];

        assert_eq!(
            matching_series_id(r"D:\TV\Showcase\Episode.mkv", &windows),
            None
        );
        assert_eq!(matching_series_id(r"D:\tv\Show\Episode.mkv", &unix), None);
    }
}
