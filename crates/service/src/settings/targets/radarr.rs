use crate::settings::rewrite::Rewrite;
use crate::settings::targets::TargetProcess;
use autopulse_database::models::ScanEvent;
use autopulse_utils::get_url;
use reqwest::header;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::Path};
use tracing::error;

use super::RequestBuilderPerform;

#[derive(Deserialize, Clone)]
pub struct Radarr {
    /// URL to the Plex server
    pub url: String,
    /// API token for the Plex server
    pub token: String,
    /// Rewrite path for the file
    pub rewrite: Option<Rewrite>,
}

#[derive(Deserialize, Debug)]
struct RadarrMovie {
    id: i64,
    path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RefreshMovie {
    movie_ids: Vec<i64>,
}

#[derive(Serialize)]
#[serde(tag = "name")]
#[serde(rename_all = "PascalCase")]
enum Command {
    RefreshMovie(RefreshMovie),
}

impl Radarr {
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

    async fn get_movies(&self, evs: &[&ScanEvent]) -> anyhow::Result<Vec<i64>> {
        let client = self.get_client().unwrap();

        let url = get_url(&self.url)?.join("api/v3/movie")?;
        let mut to_be_refreshed: HashMap<i64, Vec<String>> = HashMap::new();

        let res = client.get(url).perform().await?;

        let movies = res.json::<Vec<RadarrMovie>>().await?;

        for ev in evs {
            let ev_path = ev.get_path(&self.rewrite);
            let ev_path = Path::new(&ev_path);

            for movie in &movies {
                let movie_path = Path::new(&movie.path);
                if ev_path.starts_with(movie_path) {
                    to_be_refreshed
                        .entry(movie.id)
                        .or_default()
                        .push(ev.id.clone());
                    break;
                }
            }
        }

        // In future instead of batching the refresh command, just send individual refresh commands
        // per movie and then only partially fail events that failed to refresh
        Ok(to_be_refreshed.into_keys().collect())
    }

    async fn refresh_movies(&self, movie_ids: Vec<i64>) -> anyhow::Result<()> {
        let client = self.get_client().unwrap();
        let url = get_url(&self.url)?.join("api/v3/command")?;
        let payload = Command::RefreshMovie(RefreshMovie { movie_ids });

        client.post(url).json(&payload).perform().await.map(|_| ())
    }
}

impl TargetProcess for Radarr {
    async fn process(&self, evs: &[&ScanEvent]) -> anyhow::Result<Vec<String>> {
        let mut succeeded = Vec::new();

        let movies = self.get_movies(evs).await?;

        match self.refresh_movies(movies).await {
            Ok(()) => {
                succeeded.extend(evs.iter().map(|ev| ev.id.clone()));
            }
            Err(e) => {
                error!("failed to refresh series: {}", e);
            }
        }

        Ok(succeeded)
    }
}
