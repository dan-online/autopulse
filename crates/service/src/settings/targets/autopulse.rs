use super::{Request, RequestBuilderPerform};
use crate::settings::rewrite::Rewrite;
use crate::settings::{auth::Auth, targets::TargetProcess};
use autopulse_database::models::ScanEvent;
use autopulse_utils::get_url;
use reqwest::header;
use serde::{Deserialize, Serialize};
use tracing::{debug, error};

#[derive(Serialize, Clone, Deserialize)]
pub struct Autopulse {
    /// URL to the autopulse instance
    pub url: String,
    /// Authentication credentials
    pub auth: Auth,
    /// Trigger to hit (must be type: manual) (default: manual)
    pub trigger: Option<String>,
    /// Rewrite path for the file
    pub rewrite: Option<Rewrite>,
    /// HTTP request options
    #[serde(default)]
    pub request: Request,
}

impl Autopulse {
    fn get_client(&self) -> anyhow::Result<reqwest::Client> {
        let mut headers = header::HeaderMap::new();

        if self.auth.enabled {
            headers.insert("Authorization", self.auth.to_auth_encoded().parse()?);
        }

        self.request.apply_headers(&mut headers);

        self.request
            .client_builder()
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

        client.get(url).perform().await.map(|_| ())
    }
}

impl TargetProcess for Autopulse {
    async fn process(&self, evs: &[&ScanEvent]) -> anyhow::Result<Vec<String>> {
        let mut succeeded = Vec::new();

        for ev in evs {
            match self.scan(ev).await {
                Ok(()) => {
                    succeeded.push(ev.id.clone());
                    debug!("file scanned: {}", ev.get_path(&self.rewrite));
                }
                Err(e) => {
                    error!("error scanning file: {}", e);
                }
            }
        }

        Ok(succeeded)
    }
}
