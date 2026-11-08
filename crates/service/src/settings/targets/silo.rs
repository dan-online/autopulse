use super::{Request, RequestBuilderPerform};
use crate::settings::path_filter::PathFilter;
use crate::settings::rewrite::Rewrite;
use crate::settings::targets::TargetProcess;
use autopulse_database::models::ScanEvent;
use autopulse_utils::get_url;
use reqwest::header;
use serde::{Deserialize, Serialize};
use tracing::{debug, error};

#[derive(Serialize, Deserialize, Clone)]
pub struct Silo {
    /// URL to the Silo server
    pub url: String,
    /// API key for the Silo server (admin key with `sa_` prefix)
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

#[derive(Serialize)]
#[doc(hidden)]
struct ScanRequest {
    path: String,
}

impl Silo {
    fn get_client(&self) -> anyhow::Result<reqwest::Client> {
        let mut headers = header::HeaderMap::new();

        headers.insert(
            "Authorization",
            format!("Bearer {}", self.token).parse()?,
        );
        headers.insert("Content-Type", "application/json".parse()?);

        self.request
            .client_builder(headers)
            .build()
            .map_err(Into::into)
    }

    async fn scan(&self, ev: &ScanEvent) -> anyhow::Result<()> {
        let client = self.get_client()?;
        let url = get_url(&self.url)?.join("api/v1/scan")?;

        let body = ScanRequest {
            path: ev.get_path(&self.rewrite),
        };

        client.post(url).json(&body).perform().await.map(|_| ())
    }
}

impl TargetProcess for Silo {
    async fn process(&self, evs: &[&ScanEvent]) -> anyhow::Result<Vec<String>> {
        let mut succeeded = Vec::new();

        for ev in evs {
            match self.scan(ev).await {
                Ok(()) => {
                    debug!("scanned file in silo: {}", ev.get_path(&self.rewrite));
                    succeeded.push(ev.id.clone());
                }
                Err(e) => {
                    error!("failed to scan silo: {}", e);
                }
            }
        }

        Ok(succeeded)
    }
}
