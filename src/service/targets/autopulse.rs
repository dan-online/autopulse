use crate::{
    db::models::ScanEvent,
    settings::{auth::Auth, target::TargetProcess},
};
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
}

impl Autopulse {
    fn get_client(&self) -> anyhow::Result<reqwest::Client> {
        let mut headers = header::HeaderMap::new();

        headers.insert("Authorization", self.auth.to_auth_encoded().parse()?);

        println!("headers: {:?}", headers);

        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .default_headers(headers)
            .build()
            .map_err(Into::into)
    }

    async fn scan(&self, ev: &ScanEvent) -> anyhow::Result<()> {
        let client = self.get_client()?;
        let mut url = url::Url::parse(&self.url)?.join("/triggers/manual")?;

        url.query_pairs_mut().append_pair("path", &ev.file_path);

        if ev.file_hash.is_some() {
            url.query_pairs_mut()
                .append_pair("hash", ev.file_hash.as_ref().unwrap());
        }

        let res = client.get(url.to_string()).send().await?;

        if !res.status().is_success() {
            let body = res.text().await?;
            return Err(anyhow::anyhow!("unable to scan file: {}", body));
        }

        Ok(())
    }
}

impl TargetProcess for Autopulse {
    async fn process<'a>(&self, evs: &[&'a ScanEvent]) -> anyhow::Result<Vec<String>> {
        let mut succeded = Vec::new();

        for ev in evs {
            match self.scan(ev).await {
                Ok(_) => {
                    succeded.push(ev.id.clone());
                    debug!("file scanned: {}", ev.file_path);
                }
                Err(e) => {
                    error!("error scanning file: {}", e);
                }
            }
        }

        Ok(succeded)
    }
}
