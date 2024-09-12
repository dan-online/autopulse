use reqwest::header;
use serde::Deserialize;
use tracing::error;
use crate::{db::models::ScanEvent, utils::settings::TargetProcess};

#[derive(Deserialize, Clone)]
pub struct FileFlows {
    /// URL to the FileFlows server
    pub url: String,
}

impl FileFlows {
    fn get_client(&self) -> anyhow::Result<reqwest::Client> {
        let headers = header::HeaderMap::new();

        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .default_headers(headers)
            .build()
            .map_err(Into::into)
    }

    async fn scan(&self, ev: &ScanEvent) -> anyhow::Result<()> {
        let client = self.get_client()?;

        let mut url = url::Url::parse(&self.url)?.join("/api/library-file/process-file")?;

        url.query_pairs_mut().append_pair("filename", &ev.file_path);

        let res = client.post(url.to_string()).send().await?;

        if res.status().is_success() {
            Ok(())
        } else {
            let body = res.text().await?;
            Err(anyhow::anyhow!("unable to send scan: {}", body))
        }
    }
}

impl TargetProcess for FileFlows {
    async fn process<'a>(&mut self, evs: &[&'a ScanEvent]) -> anyhow::Result<Vec<String>> {
        let mut succeeded = Vec::new();

        for ev in evs {
            let res = self.scan(ev).await;

            match res {
                Ok(_) => {
                    succeeded.push(ev.file_path.clone());
                }
                Err(e) => {
                    error!("failed to process '{}': {:?}", ev.file_path, e);
                }
            }
        }

        Ok(succeeded)
    }
}
