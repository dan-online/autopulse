use serde::Deserialize;

use crate::{
    db::models::ScanEvent,
    service::targets::{
        autopulse::Autopulse, command::Command, emby::Emby, fileflows::FileFlows, plex::Plex,
        radarr::Radarr, sonarr::Sonarr, tdarr::Tdarr,
    },
};

pub trait TargetProcess {
    fn process(
        &self,
        evs: &[&ScanEvent],
    ) -> impl std::future::Future<Output = anyhow::Result<Vec<String>>> + Send;
}

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
