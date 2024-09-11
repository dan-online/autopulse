use std::collections::HashMap;

use config::{Config, File};
use serde::Deserialize;

use crate::{
    db::models::ScanEvent,
    service::{
        targets::{command::Command, emby::Emby, plex::Plex},
        triggers::{
            lidarr::LidarrRequest, notify::NotifyService, radarr::RadarrRequest,
            readarr::ReadarrRequest, sonarr::SonarrRequest,
        },
        webhooks::discord::DiscordWebhook,
    },
};

use super::timer::Timer;

#[derive(Deserialize, Clone)]
pub struct App {
    pub hostname: String,
    pub port: u16,
    pub database_url: String,
    pub log_level: String,
}

#[derive(Deserialize, Clone)]
pub struct Auth {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize, Clone)]
pub struct Opts {
    pub check_path: bool,
    pub max_retries: i32,
    pub default_timer_wait: u64,
    pub cleanup_days: u64,
}

#[derive(Deserialize, Clone)]
pub struct Settings {
    pub app: App,
    pub auth: Auth,

    pub opts: Opts,

    pub triggers: HashMap<String, Trigger>,
    pub targets: HashMap<String, Target>,

    pub webhooks: HashMap<String, Webhook>,
}

impl Settings {
    pub fn get_settings() -> anyhow::Result<Self> {
        let settings = Config::builder()
            .add_source(File::with_name("default.toml"))
            .add_source(config::File::with_name("config").required(false))
            .add_source(config::Environment::with_prefix("AUTOPULSE").separator("__"))
            .build()?;

        settings
            .try_deserialize::<Settings>()
            .map_err(|e| anyhow::anyhow!(e))
    }

    pub fn get_tickable_triggers(&self) -> Vec<String> {
        self.triggers
            .iter()
            .filter(|(_, x)| x.can_tick(self.opts.default_timer_wait))
            .map(|(k, _)| k.clone())
            .collect::<Vec<String>>()
    }
}

#[derive(Deserialize, Clone)]
pub struct Rewrite {
    pub from: String,
    pub to: String,
}

#[derive(Deserialize, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Trigger {
    Manual {
        rewrite: Option<Rewrite>,
        #[serde(default)]
        timer: Timer,
    },
    Radarr {
        rewrite: Option<Rewrite>,
        #[serde(default)]
        timer: Timer,
    },
    Sonarr {
        rewrite: Option<Rewrite>,
        #[serde(default)]
        timer: Timer,
    },
    Lidarr {
        rewrite: Option<Rewrite>,
        #[serde(default)]
        timer: Timer,
    },
    Readarr {
        rewrite: Option<Rewrite>,
        #[serde(default)]
        timer: Timer,
    },
    Notify(NotifyService),
}

impl Trigger {
    pub fn paths(&self, body: serde_json::Value) -> anyhow::Result<Vec<(String, bool)>> {
        match &self {
            Self::Sonarr { .. } => Ok(SonarrRequest::from_json(body)?.paths()),
            Self::Radarr { .. } => Ok(RadarrRequest::from_json(body)?.paths()),
            Self::Lidarr { .. } => Ok(LidarrRequest::from_json(body)?.paths()),
            Self::Readarr { .. } => Ok(ReadarrRequest::from_json(body)?.paths()),
            Self::Manual { .. } | Self::Notify(_) => {
                Err(anyhow::anyhow!("Manual trigger does not have paths"))
            }
        }
    }

    pub fn can_tick(&self, default: u64) -> bool {
        match &self {
            Self::Manual { timer, .. }
            | Self::Radarr { timer, .. }
            | Self::Sonarr { timer, .. }
            | Self::Lidarr { timer, .. }
            | Self::Readarr { timer, .. } => timer.can_tick(default),
            Self::Notify(service) => service.timer.can_tick(default),
        }
    }

    pub fn tick(&self) {
        match &self {
            Self::Manual { timer, .. } => timer.tick(),
            Self::Radarr { timer, .. } => timer.tick(),
            Self::Sonarr { timer, .. } => timer.tick(),
            Self::Lidarr { timer, .. } => timer.tick(),
            Self::Readarr { timer, .. } => timer.tick(),
            Self::Notify(service) => service.timer.tick(),
        }
    }
}

#[derive(Deserialize, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Webhook {
    Discord(DiscordWebhook),
}

pub trait TargetProcess {
    fn process<'a>(
        &mut self,
        evs: &[&'a ScanEvent],
    ) -> impl std::future::Future<Output = anyhow::Result<Vec<String>>> + Send;
}

pub trait TriggerRequest {
    fn from_json(json: serde_json::Value) -> anyhow::Result<Self>
    where
        Self: Sized;

    // where the bool represents whether to check found status
    fn paths(&self) -> Vec<(String, bool)>;
}

#[derive(Deserialize, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Target {
    Plex(Plex),
    Jellyfin(Emby),
    Emby(Emby),
    Command(Command),
}

impl Target {
    pub async fn process(&mut self, evs: &[&ScanEvent]) -> anyhow::Result<Vec<String>> {
        match self {
            Self::Plex(p) => p.process(evs).await,
            Self::Jellyfin(j) => j.process(evs).await,
            Self::Emby(e) => e.process(evs).await,
            Self::Command(c) => c.process(evs).await,
        }
    }
}
