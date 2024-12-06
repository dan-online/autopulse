use serde::Deserialize;

use crate::{
    db::models::ScanEvent,
    service::targets::{
        autopulse::Autopulse, command::Command, emby::Emby, fileflows::FileFlows, plex::Plex,
        tdarr::Tdarr,
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
    Command(Command),
    FileFlows(FileFlows),
    Autopulse(Autopulse),
}

impl Target {
    pub async fn process(&self, evs: &[&ScanEvent]) -> anyhow::Result<Vec<String>> {
        match self {
            Self::Plex(p) => p.process(evs).await,
            Self::Jellyfin(j) => j.process(evs).await,
            Self::Emby(e) => e.process(evs).await,
            Self::Command(c) => c.process(evs).await,
            Self::Tdarr(t) => t.process(evs).await,
            Self::FileFlows(f) => f.process(evs).await,
            Self::Autopulse(a) => a.process(evs).await,
        }
    }
}
