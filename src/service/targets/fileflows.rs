use std::collections::HashMap;

use crate::{db::models::ScanEvent, settings::target::TargetProcess};
use reqwest::header;
use serde::{Deserialize, Serialize};
use tracing::error;

#[derive(Deserialize, Clone)]
pub struct FileFlows {
    /// URL to the FileFlows server
    pub url: String,
}

#[derive(Deserialize, Clone, Eq, PartialEq, Hash, Debug)]
#[doc(hidden)]
#[serde(rename_all = "PascalCase")]
struct FileFlowsLibrary {
    uid: String,
    enabled: bool,
    path: Option<String>,
}

#[derive(Serialize)]
#[doc(hidden)]
#[serde(rename_all = "PascalCase")]
struct FileFlowsRescanLibraryRequest {
    uids: Vec<String>,
}

// TODO: get library files and then reprocess them
impl FileFlows {
    fn get_client(&self) -> anyhow::Result<reqwest::Client> {
        let headers = header::HeaderMap::new();

        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .default_headers(headers)
            .build()
            .map_err(Into::into)
    }

    async fn get_libraries(&self) -> anyhow::Result<Vec<FileFlowsLibrary>> {
        let client = self.get_client()?;

        let url = url::Url::parse(&self.url)?.join("/api/library")?;

        let res = client.get(url.to_string()).send().await?;

        if res.status().is_success() {
            let body = res.text().await?;
            let libraries: Vec<FileFlowsLibrary> = serde_json::from_str(&body)?;
            Ok(libraries)
        } else {
            let body = res.text().await?;
            Err(anyhow::anyhow!("unable to get libraries: {}", body))
        }
    }

    async fn rescan_library(&self, libraries: &FileFlowsLibrary) -> anyhow::Result<()> {
        let client = self.get_client()?;

        let url = url::Url::parse(&self.url)?.join("/api/library/rescan")?;

        let req = FileFlowsRescanLibraryRequest {
            uids: vec![libraries.uid.clone()],
        };

        let res = client.put(url.to_string()).json(&req).send().await?;

        if res.status().is_success() {
            Ok(())
        } else {
            let body = res.text().await?;
            Err(anyhow::anyhow!("unable to send rescan: {}", body))
        }
    }

    // No longer in fileflows..
    // async fn scan(&self, ev: &ScanEvent, library: &FileFlowsLibrary) -> anyhow::Result<()> {
    //     let client = self.get_client()?;

    //     let mut url = url::Url::parse(&self.url)?.join("/api/library-file/process-file")?;

    //     url.query_pairs_mut().append_pair("filename", &ev.file_path);

    //     let res = client.post(url.to_string()).send().await?;

    //     if res.status().is_success() {
    //         Ok(())
    //     } else {
    //         let body = res.text().await?;
    //         Err(anyhow::anyhow!("unable to send scan: {}", body))
    //     }
    // }
}

impl TargetProcess for FileFlows {
    async fn process(&self, evs: &[&ScanEvent]) -> anyhow::Result<Vec<String>> {
        let mut succeeded = Vec::new();
        let libraries = self.get_libraries().await?;

        let mut to_scan: HashMap<&FileFlowsLibrary, Vec<&ScanEvent>> = HashMap::new();

        for ev in evs {
            let library = libraries.iter().find(|l| {
                l.path
                    .as_ref()
                    .map_or(false, |path| ev.file_path.starts_with(path))
            });

            if library.is_none() {
                error!("unable to find library for file: {}", ev.file_path);
                continue;
            }

            let library = library.unwrap();

            if !library.enabled {
                error!("library '{}' is disabled", library.uid);
                continue;
            }

            // let res = self.scan(ev, library).await;

            // match res {
            //     Ok(_) => {
            //         succeeded.push(ev.file_path.clone());
            //     }
            //     Err(e) => {
            //         error!("failed to process '{}': {:?}", ev.file_path, e);
            //     }
            // }

            to_scan.entry(library).or_default().push(ev);
        }

        for (library, evs) in to_scan {
            if let Err(e) = self.rescan_library(library).await {
                error!(
                    "failed to rescan '{}': {:?}",
                    library.path.clone().unwrap_or_else(|| library.uid.clone()),
                    e
                );
            } else {
                succeeded.extend(evs.iter().map(|ev| ev.id.clone()));
            }
        }

        Ok(succeeded)
    }
}
