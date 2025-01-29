use crate::{
    db::models::ScanEvent,
    settings::{rewrite::Rewrite, target::TargetProcess},
    utils::get_url::get_url,
};
use anyhow::Context;
use reqwest::header;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, path::Path, path::PathBuf};
use tracing::{debug, error, trace};

#[derive(Deserialize, Clone)]
pub struct FileFlows {
    /// URL to the FileFlows server
    url: String,
    /// Rewrite path for the file
    rewrite: Option<Rewrite>,
}

#[derive(Serialize, Deserialize, Clone, Eq, PartialEq, Hash, Debug)]
#[doc(hidden)]
#[serde(rename_all = "PascalCase")]
struct FileFlowsFlow {
    uid: String,
}

#[derive(Deserialize, Clone, Eq, PartialEq, Hash, Debug)]
#[doc(hidden)]
#[serde(rename_all = "PascalCase")]
struct FileFlowsLibrary {
    uid: String,
    enabled: bool,
    path: Option<String>,
    flow: Option<FileFlowsFlow>,
}

// #[derive(Serialize)]
// #[doc(hidden)]
// #[serde(rename_all = "PascalCase")]
// struct FileFlowsRescanLibraryRequest {
//     uids: Vec<String>,
// }

#[derive(Serialize, Debug)]
#[doc(hidden)]
#[serde(rename_all = "PascalCase")]
struct FileFlowsManuallyAddRequest {
    flow_uid: String,
    files: Vec<String>,
    #[serde(default)]
    custom_variables: HashMap<String, String>,
}

#[derive(Serialize)]
#[doc(hidden)]
#[serde(rename_all = "PascalCase")]
struct FileFlowsSearchRequest {
    path: String,
    limit: u32, // set to 1
}

#[derive(Serialize, Default, Debug)]
#[doc(hidden)]
#[serde(rename_all = "PascalCase")]
struct FileFlowsReprocessRequest {
    uids: Vec<String>,
    custom_variables: HashMap<String, String>,
    mode: u8,
    flow: Option<Value>,
    node: Option<Value>,
    bottom_of_queue: bool,
}

#[derive(Deserialize, Clone, Eq, PartialEq, Hash, Debug)]
#[doc(hidden)]
#[serde(rename_all = "PascalCase")]
struct FileFlowsLibraryFile {
    uid: String,
    flow_uid: String,
    name: String, // filename, maybe use output_path later..
}

// How to "scan" a file in fileflows
// First, get the libraries
// Group files with their library
// if the library disabled- error
// Next get each file and check their status
// If they are processed, send a reprocess request individually
// For the rest, send a manual-add request, again still in a group with their library

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

        let url = get_url(&self.url)?.join("api/library")?;

        let res = client.get(url.to_string()).send().await?;
        let status = res.status();

        if status.is_success() {
            let libraries: Vec<FileFlowsLibrary> = res.json().await?;

            Ok(libraries)
        } else {
            let body = res.text().await?;

            Err(anyhow::anyhow!(
                "failed to get libraries: {} - {}",
                status.as_u16(),
                body
            ))
        }
    }

    async fn get_library_file(
        &self,
        ev: &ScanEvent,
    ) -> anyhow::Result<Option<FileFlowsLibraryFile>> {
        let client = self.get_client()?;

        let url = get_url(&self.url)?.join("api/library-file/search")?;

        let req = FileFlowsSearchRequest {
            path: ev.get_path(&self.rewrite),
            limit: 1,
        };

        let res = client.post(url.to_string()).json(&req).send().await?;
        let status = res.status();

        if status.is_success() {
            let files: Vec<FileFlowsLibraryFile> = res.json().await?;

            Ok(files.first().cloned())
        } else {
            let body = res.text().await?;

            Err(anyhow::anyhow!(
                "failed to get library file: {} - {}",
                status.as_u16(),
                body
            ))
        }
    }

    async fn reprocess_library_filse(&self, evs: Vec<&FileFlowsLibraryFile>) -> anyhow::Result<()> {
        let client = self.get_client()?;

        let url = get_url(&self.url)?.join("api/library-file/reprocess")?;

        let req = FileFlowsReprocessRequest {
            uids: evs.iter().map(|ev| ev.uid.clone()).collect(),
            ..Default::default()
        };

        let res = client.post(url.to_string()).json(&req).send().await?;
        let status = res.status();

        if status.is_success() {
            Ok(())
        } else {
            let body = res.text().await?;

            Err(anyhow::anyhow!(
                "failed to send reprocess: {} - {}",
                status.as_u16(),
                body
            ))
        }
    }

    async fn manually_add_files(
        &self,
        library: &FileFlowsLibrary,
        files: Vec<&ScanEvent>,
    ) -> anyhow::Result<()> {
        let client = self.get_client()?;

        let url = get_url(&self.url)?.join("api/library-file/manually-add")?;

        let req = FileFlowsManuallyAddRequest {
            flow_uid: library.flow.as_ref().unwrap().uid.clone(),
            files: files.iter().map(|ev| ev.get_path(&self.rewrite)).collect(),
            custom_variables: HashMap::new(),
        };

        let res = client.post(url.to_string()).json(&req).send().await?;
        let status = res.status();

        if status.is_success() {
            Ok(())
        } else {
            let body = res.text().await?;

            Err(anyhow::anyhow!(
                "failed to send manual-add: {} - {}",
                status.as_u16(),
                body
            ))
        }
    }

    // async fn rescan_library(&self, libraries: &FileFlowsLibrary) -> anyhow::Result<()> {
    //     let client = self.get_client()?;

    //     let url = get_url(&self.url)?.join("/api/library/rescan")?;

    //     let req = FileFlowsRescanLibraryRequest {
    //         uids: vec![libraries.uid.clone()],
    //     };

    //     let res = client.put(url.to_string()).json(&req).send().await?;

    //     if res.status().is_success() {
    //         Ok(())
    //     } else {
    //         let body = res.text().await?;
    //         Err(anyhow::anyhow!("failed to send rescan: {}", body))
    //     }
    // }

    // No longer in fileflows..
    // async fn scan(&self, ev: &ScanEvent, library: &FileFlowsLibrary) -> anyhow::Result<()> {
    //     let client = self.get_client()?;

    //     let mut url = get_url(&self.url)?.join("/api/library-file/process-file")?;

    //     url.query_pairs_mut().append_pair("filename", &ev.file_path);

    //     let res = client.post(url.to_string()).send().await?;

    //     if res.status().is_success() {
    //         Ok(())
    //     } else {
    //         let body = res.text().await?;
    //         Err(anyhow::anyhow!("failed to send scan: {}", body))
    //     }
    // }
}

impl TargetProcess for FileFlows {
    async fn process(&self, evs: &[&ScanEvent]) -> anyhow::Result<Vec<String>> {
        let mut succeeded = Vec::new();
        let libraries = self
            .get_libraries()
            .await
            .context("failed to get libraries")?;

        let mut to_scan: HashMap<FileFlowsLibrary, Vec<&ScanEvent>> = HashMap::new();

        for library in libraries {
            let files = evs
                .iter()
                .filter_map(|ev| {
                    let ev_path = ev.get_path(&self.rewrite);
                    let ev_path = Path::new(&ev_path);
                    let lib_path = Path::new(library.path.as_deref()?);

                    if ev_path.starts_with(lib_path) {
                        Some(*ev)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();

            if files.is_empty() {
                continue;
            }

            if !library.enabled {
                error!(
                    "library '{}' is disabled but {} files will fail to scan",
                    library.uid,
                    files.len()
                );
                continue;
            }

            to_scan.insert(library, files);
        }

        for (library, evs) in to_scan {
            let mut library_files = HashMap::new();

            for ev in evs {
                // Skip directories
                if PathBuf::from(&ev.get_path(&self.rewrite)).is_dir() {
                    succeeded.push(ev.id.clone());
                    continue;
                }

                match self.get_library_file(ev).await {
                    Ok(file) => {
                        library_files.insert(ev, file);
                    }
                    Err(e) => {
                        error!("failed to get library file: {}", e);
                        library_files.insert(ev, None);
                    }
                }
            }

            let (processed, not_processed): (Vec<_>, Vec<_>) =
                library_files.iter().partition(|(_, file)| file.is_some());

            trace!(
                "library {} has {} processed and {} not processed files",
                library.uid,
                processed.len(),
                not_processed.len()
            );

            if !processed.is_empty() {
                match self
                    .reprocess_library_filse(
                        processed
                            .iter()
                            .filter_map(|(_, file)| file.as_ref())
                            .collect(),
                    )
                    .await
                {
                    Ok(()) => {
                        for (ev, _) in processed.iter() {
                            debug!("reprocessed file: {}", ev.get_path(&self.rewrite));
                        }
                        succeeded.extend(processed.iter().map(|(ev, _)| ev.id.clone()));
                    }
                    Err(e) => error!("failed to reprocess files: {}", e),
                }
            }

            if !not_processed.is_empty() {
                match self
                    .manually_add_files(
                        &library,
                        not_processed.iter().map(|(ev, _)| **ev).collect(),
                    )
                    .await
                {
                    Ok(()) => {
                        for (ev, _) in not_processed.iter() {
                            debug!("manually added file: {}", ev.get_path(&self.rewrite));
                        }
                        succeeded.extend(not_processed.iter().map(|(ev, _)| ev.id.clone()));
                    }
                    Err(e) => error!("failed to manually add files: {}", e),
                }
            }
        }

        Ok(succeeded)
    }
}
