/// Autopulse - Autopulse target
///
/// This target is used to process a file in another instance of Autopulse
///
/// # Example
///
/// ```yml
/// targets:
///   autopulse:
///     type: autopulse
///     url: http://localhost:2875
///     auth:
///       username: "admin"
///       password: "password"
/// ```
/// or
/// ```yml
/// targets:
///   autopulse:
///     type: autopulse
///     url: http://localhost:2875
///     auth:
///       username: "admin"
///       password: "password"
///     trigger: "other"
/// ```
///
/// See [`Autopulse`] for all options
pub mod autopulse;
/// Command - Command target
///
/// This target is used to run a command to process a file
///
/// # Example
///
/// ```yml
/// targets:
///   list:
///     type: command
///     raw: "echo $FILE_PATH >> list.log"
/// ```
///
/// or
///
/// ```yml
/// targets:
///   list:
///     type: command
///     path: "/path/to/script.sh"
/// ```
///
/// See [`Command`] for all options
pub mod command;
/// Emby - Emby/Jellyfin target
///
/// This target is used to refresh/scan a file in Emby/Jellyfin
///
/// # Example
///
/// ```yml
/// targets:
///   my_jellyfin:
///     type: jellyfin
///     url: http://localhost:8096
///     token: "<API_KEY>"
///     # refresh_metadata: false # To disable metadata refresh
/// ```
/// or
/// ```yml
/// targets:
///   my_emby:
///     type: emby
///     url: http://localhost:8096
///     token: "<API_KEY>"
///     # refresh_metadata: false # To disable metadata refresh
///     # metadata_refresh_mode: "validation_only" # To change metadata refresh mode
/// ```
///
/// See [`Emby`] for all options
#[doc(alias("jellyfin"))]
pub mod emby;
/// `FileFlows` - `FileFlows` target
///
/// This target is used to process a file in `FileFlows`
///
/// # Example
///
/// ```yml
/// targets:
///   fileflows:
///     type: fileflows
///     url: http://localhost:5000
/// ```
///
/// See [`FileFlows`] for all options
pub mod fileflows;
/// Plex - Plex target
///
/// This target is used to scan a file in Plex
///
/// # Example
///
/// ```yml
/// targets:
///   my_plex:
///     type: plex
///     url: http://localhost:32400
///     token: "<PLEX_TOKEN>"
/// ```
/// or
/// ```yml
/// targets:
///   my_plex:
///     type: plex
///     url: http://localhost:32400
///     token: "<PLEX_TOKEN>"
///     refresh: true
///     analyze: true
/// ```
///
/// See [`Plex`] for all options
pub mod plex;
/// Radarr - Radarr target
///
/// This target is used to refresh/rescan a movie in Radarr
///
/// # Example
///
/// ```yml
/// targets:
///   radarr:
///     type: radarr
///     url: http://localhost:7878
///     token: "<API_KEY>"
/// ```
///
/// See [`Radarr`] for all options
pub mod radarr;
/// Sonarr - Sonarr target
///
/// This target is used to refresh/rescan a series in Sonarr
///
/// # Example
///
/// ```yml
/// targets:
///   sonarr:
///     type: sonarr
///     url: http://localhost:8989
///     token: "<API_KEY>"
/// ```
///
/// See [`Sonarr`] for all options
pub mod sonarr;
/// Tdarr - Tdarr target
///
/// This target is used to process a file in Tdarr
///
/// # Example
///
/// ```yml
/// targets:
///   tdarr:
///     type: tdarr
///     url: http://localhost:8265
///     db_id: "<LIBRARY_ID>"
/// ```
///
/// See [`Tdarr`] for all options
pub mod tdarr;

use autopulse_database::models::ScanEvent;
use reqwest::{RequestBuilder, Response};
use serde::Deserialize;
use {
    autopulse::Autopulse, command::Command, emby::Emby, fileflows::FileFlows, plex::Plex,
    radarr::Radarr, sonarr::Sonarr, tdarr::Tdarr,
};

#[derive(Deserialize, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Target {
    Plex(Plex),
    Jellyfin(Emby),
    Emby(Emby),
    Tdarr(Tdarr),
    Sonarr(Sonarr),
    Radarr(Radarr),
    Command(Command),
    FileFlows(FileFlows),
    Autopulse(Autopulse),
}

pub trait TargetProcess {
    fn process(
        &self,
        evs: &[&ScanEvent],
    ) -> impl std::future::Future<Output = anyhow::Result<Vec<String>>> + Send;
}

impl TargetProcess for Target {
    async fn process(&self, evs: &[&ScanEvent]) -> anyhow::Result<Vec<String>> {
        match self {
            Self::Plex(t) => t.process(evs).await,
            Self::Jellyfin(t) | Self::Emby(t) => t.process(evs).await,
            Self::Command(t) => t.process(evs).await,
            Self::Tdarr(t) => t.process(evs).await,
            Self::Sonarr(t) => t.process(evs).await,
            Self::Radarr(t) => t.process(evs).await,
            Self::FileFlows(t) => t.process(evs).await,
            Self::Autopulse(t) => t.process(evs).await,
        }
    }
}

pub trait RequestBuilderPerform {
    fn perform(self) -> impl std::future::Future<Output = anyhow::Result<Response>> + Send;
}

impl RequestBuilderPerform for RequestBuilder {
    async fn perform(self) -> anyhow::Result<Response> {
        let copy = self
            .try_clone()
            .ok_or_else(|| anyhow::anyhow!("failed to clone request"))?;
        let built = copy
            .build()
            .map_err(|e| anyhow::anyhow!("failed to build request: {}", e))?;
        let response = self.send().await;

        match response {
            Ok(response) => {
                if !response.status().is_success() {
                    return Err(anyhow::anyhow!(
                        // failed to PUT /path/to/file: 404 - Not Found
                        "unable to {} {}: {} - {}",
                        built.method(),
                        built.url(),
                        response.status(),
                        response
                            .text()
                            .await
                            .unwrap_or_else(|_| "unknown error".to_string()),
                    ));
                }

                Ok(response)
            }

            Err(e) => Err(anyhow::anyhow!(
                "failed to {} {}: {}",
                built.method(),
                built.url(),
                e,
            )),
        }
    }
}
