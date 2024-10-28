use super::{rewrite::Rewrite, timer::Timer};
use crate::service::triggers::{
    lidarr::{Lidarr, LidarrRequest},
    manual::Manual,
    notify::Notify,
    radarr::{Radarr, RadarrRequest},
    readarr::ReadarrRequest,
    sonarr::{Sonarr, SonarrRequest},
};
use serde::Deserialize;

pub trait TriggerRequest {
    fn from_json(json: serde_json::Value) -> anyhow::Result<Self>
    where
        Self: Sized;

    // where the bool represents whether to check found status
    fn paths(&self) -> Vec<(String, bool)>;
}

#[derive(Deserialize, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Trigger {
    Manual(Manual),
    Radarr(Radarr),
    Sonarr(Sonarr),
    Lidarr(Lidarr),
    Readarr(Sonarr),
    Notify(Notify),
}

impl Trigger {
    pub const fn get_rewrite(&self) -> Option<&Rewrite> {
        match &self {
            Self::Sonarr(trigger) => trigger.rewrite.as_ref(),
            Self::Radarr(trigger) => trigger.rewrite.as_ref(),
            Self::Lidarr(trigger) => trigger.rewrite.as_ref(),
            Self::Readarr(trigger) => trigger.rewrite.as_ref(),
            Self::Manual(_) | Self::Notify(_) => None,
        }
    }

    pub const fn get_timer(&self) -> &Timer {
        match &self {
            Self::Sonarr(trigger) => &trigger.timer,
            Self::Radarr(trigger) => &trigger.timer,
            Self::Lidarr(trigger) => &trigger.timer,
            Self::Readarr(trigger) => &trigger.timer,
            Self::Manual(trigger) => &trigger.timer,
            Self::Notify(trigger) => &trigger.timer,
        }
    }

    pub fn paths(&self, body: serde_json::Value) -> anyhow::Result<Vec<(String, bool)>> {
        match &self {
            Self::Sonarr(_) => Ok(SonarrRequest::from_json(body)?.paths()),
            Self::Radarr(_) => Ok(RadarrRequest::from_json(body)?.paths()),
            Self::Lidarr(_) => Ok(LidarrRequest::from_json(body)?.paths()),
            Self::Readarr(_) => Ok(ReadarrRequest::from_json(body)?.paths()),
            Self::Manual(_) | Self::Notify(_) => {
                Err(anyhow::anyhow!("Manual trigger does not have paths"))
            }
        }
    }

    pub const fn excludes(&self) -> &Vec<String> {
        match &self {
            Self::Manual(trigger) => &trigger.excludes,
            Self::Radarr(trigger) => &trigger.excludes,
            Self::Sonarr(trigger) => &trigger.excludes,
            Self::Lidarr(trigger) => &trigger.excludes,
            Self::Readarr(trigger) => &trigger.excludes,
            Self::Notify(trigger) => &trigger.excludes,
        }
    }
}
