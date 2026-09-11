use super::{Request, RequestBuilderPerform};
use crate::settings::path_filter::PathFilter;
use crate::settings::rewrite::Rewrite;
use crate::settings::targets::TargetProcess;
use anyhow::Context;
use autopulse_database::models::ScanEvent;
use autopulse_utils::{get_url, RuntimePath};
use reqwest::header;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tracing::{debug, error, trace};

#[derive(Serialize, Deserialize, Clone)]
pub struct FileFlows {
    /// URL to the `FileFlows` server
    pub url: String,
    /// Rewrite path for the file
    pub rewrite: Option<Rewrite>,
    /// Path filter matched against the target-rewritten path.
    #[serde(default)]
    pub filter: PathFilter,
    /// HTTP request options
    #[serde(default)]
    pub request: Request,
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

fn path_in_library(path: &str, library_path: &str) -> bool {
    RuntimePath::new(path).starts_with(RuntimePath::new(library_path))
}

impl FileFlows {
    fn get_client(&self) -> anyhow::Result<reqwest::Client> {
        self.request
            .client_builder(header::HeaderMap::new())
            .build()
            .map_err(Into::into)
    }

    async fn get_libraries(&self) -> anyhow::Result<Vec<FileFlowsLibrary>> {
        let client = self.get_client()?;
        let url = get_url(&self.url)?.join("api/library")?;

        let res = client.get(url).perform().await?;

        let libraries: Vec<FileFlowsLibrary> = res.json().await?;

        Ok(libraries)
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

        let res = client.post(url).json(&req).perform().await?;

        let files: Vec<FileFlowsLibraryFile> = res.json().await?;

        Ok(files.first().cloned())
    }

    async fn reprocess_library_file(&self, evs: Vec<&FileFlowsLibraryFile>) -> anyhow::Result<()> {
        let client = self.get_client()?;

        let url = get_url(&self.url)?.join("api/library-file/reprocess")?;

        let req = FileFlowsReprocessRequest {
            uids: evs.iter().map(|ev| ev.uid.clone()).collect(),
            ..Default::default()
        };

        client.post(url).json(&req).perform().await.map(|_| ())
    }

    async fn manually_add_files(
        &self,
        library: &FileFlowsLibrary,
        files: Vec<&ScanEvent>,
    ) -> anyhow::Result<()> {
        let client = self.get_client()?;

        let url = get_url(&self.url)?.join("api/library-file/manually-add")?;

        let flow = library
            .flow
            .as_ref()
            .context("library has no flow configured")?;

        let req = FileFlowsManuallyAddRequest {
            flow_uid: flow.uid.clone(),
            files: files.iter().map(|ev| ev.get_path(&self.rewrite)).collect(),
            custom_variables: HashMap::new(),
        };

        client.post(url).json(&req).perform().await.map(|_| ())
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
                    let event_path = ev.get_path(&self.rewrite);
                    let library_path = library.path.as_deref()?;

                    path_in_library(&event_path, library_path).then_some(*ev)
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
                let event_path = ev.get_path(&self.rewrite);
                if RuntimePath::new(&event_path).is_directory() {
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
                    .reprocess_library_file(
                        processed
                            .iter()
                            .filter_map(|(_, file)| file.as_ref())
                            .collect(),
                    )
                    .await
                {
                    Ok(()) => {
                        for (ev, _) in &processed {
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
                        for (ev, _) in &not_processed {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_windows_fileflows_library_by_runtime_components() {
        assert!(path_in_library(
            r"D:\MEDIA\Incoming\Film\Film.mkv",
            r"d:\media\incoming"
        ));
        assert!(!path_in_library(
            r"D:\Media\Incoming-Archive\Film.mkv",
            r"D:\Media\Incoming"
        ));
        assert!(!path_in_library(
            r"D:\Media\Incoming\Film.mkv",
            "/Media/Incoming"
        ));
    }
}
