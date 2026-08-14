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
pub struct Radarr {
    /// URL to the Radarr server
    pub url: String,
    /// API token for the Radarr server
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

fn matching_movie_id(path: &str, movies: &[RadarrMovie]) -> Option<i64> {
    let event = RuntimePath::new(path);
    movies
        .iter()
        .find(|movie| event.starts_with(RuntimePath::new(&movie.path)))
        .map(|movie| movie.id)
}

impl Radarr {
    fn get_client(&self) -> anyhow::Result<reqwest::Client> {
        let mut headers = header::HeaderMap::new();

        headers.insert("X-Api-Key", self.token.parse()?);
        headers.insert("Accept", "application/json".parse()?);

        self.request
            .client_builder(headers)
            .build()
            .map_err(Into::into)
    }

    async fn get_movies(&self, evs: &[&ScanEvent]) -> anyhow::Result<Vec<i64>> {
        let client = self.get_client()?;

        let url = get_url(&self.url)?.join("api/v3/movie")?;
        let mut to_be_refreshed: HashMap<i64, Vec<String>> = HashMap::new();

        let res = client.get(url).perform().await?;

        let movies = res.json::<Vec<RadarrMovie>>().await?;

        for ev in evs {
            let ev_path = ev.get_path(&self.rewrite);

            if let Some(movie_id) = matching_movie_id(&ev_path, &movies) {
                to_be_refreshed
                    .entry(movie_id)
                    .or_default()
                    .push(ev.id.clone());
            }
        }

        // TODO: per-movie commands would let us isolate partial failures,
        // but the serial-POST cost on large imports outweighs that today.
        Ok(to_be_refreshed.into_keys().collect())
    }

    async fn refresh_movies(&self, movie_ids: Vec<i64>) -> anyhow::Result<()> {
        let client = self.get_client()?;
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
                error!("failed to refresh movies: {e}");
            }
        }

        Ok(succeeded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_windows_movie_case_insensitively_by_components() {
        let movies = vec![RadarrMovie {
            id: 24,
            path: r"\\SERVER\Movies\The Matrix".to_string(),
        }];

        assert_eq!(
            matching_movie_id(r"\\server\movies\the matrix\The Matrix (1999).mkv", &movies),
            Some(24)
        );
    }

    #[test]
    fn rejects_movie_text_prefix_and_mixed_flavor() {
        let windows = vec![RadarrMovie {
            id: 24,
            path: r"C:\Movies\Alien".to_string(),
        }];
        let unix = vec![RadarrMovie {
            id: 12,
            path: "/movies/Alien".to_string(),
        }];

        assert_eq!(
            matching_movie_id(r"C:\Movies\Aliens\Aliens.mkv", &windows),
            None
        );
        assert_eq!(matching_movie_id(r"C:\movies\Alien\Alien.mkv", &unix), None);
    }
}
