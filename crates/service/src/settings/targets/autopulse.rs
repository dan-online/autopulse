use crate::settings::rewrite::Rewrite;
use crate::settings::{auth::Auth, targets::TargetProcess};
use autopulse_database::models::ScanEvent;
use autopulse_utils::get_url::get_url;
use reqwest::header;
use serde::Deserialize;
use tracing::{debug, error};

#[derive(Clone, Deserialize)]
pub struct Autopulse {
    /// URL to the autopulse instance
    pub url: String,
    /// Authentication credentials
    pub auth: Auth,
    /// Trigger to hit (must be type: manual) (default: manual)
    pub trigger: Option<String>,
    /// Rewrite path for the file
    pub rewrite: Option<Rewrite>,
}

impl Autopulse {
    fn get_client(&self) -> anyhow::Result<reqwest::Client> {
        let mut headers = header::HeaderMap::new();

        if self.auth.enabled {
            headers.insert("Authorization", self.auth.to_auth_encoded().parse()?);
        }

        println!("headers: {headers:?}");

        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .default_headers(headers)
            .build()
            .map_err(Into::into)
    }

    async fn scan(&self, ev: &ScanEvent) -> anyhow::Result<()> {
        let client = self.get_client()?;
        let mut url = get_url(&self.url)?.join("triggers/manual")?;

        url.query_pairs_mut()
            .append_pair("path", &ev.get_path(&self.rewrite));

        if ev.file_hash.is_some() {
            url.query_pairs_mut()
                .append_pair("hash", ev.file_hash.as_ref().unwrap());
        }

        let res = client.get(url.to_string()).send().await?;
        let status = res.status();

        if !status.is_success() {
            let body = res.text().await?;

            return Err(anyhow::anyhow!(
                "failed to scan file: {} - {}",
                status.as_u16(),
                body
            ));
        }

        Ok(())
    }
}

impl TargetProcess for Autopulse {
    async fn process(&self, evs: &[&ScanEvent]) -> anyhow::Result<Vec<String>> {
        let mut succeded = Vec::new();

        for ev in evs {
            match self.scan(ev).await {
                Ok(()) => {
                    succeded.push(ev.id.clone());
                    debug!("file scanned: {}", ev.get_path(&self.rewrite));
                }
                Err(e) => {
                    error!("error scanning file: {}", e);
                }
            }
        }

        Ok(succeded)
    }
}
