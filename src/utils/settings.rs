use super::timer::Timer;
use crate::{
    db::models::ScanEvent,
    service::{
        targets::{command::Command, emby::Emby, plex::Plex, tdarr::Tdarr},
        triggers::{
            lidarr::LidarrRequest, notify::Notify, radarr::RadarrRequest, readarr::ReadarrRequest,
            sonarr::SonarrRequest,
        },
        webhooks::discord::DiscordWebhook,
    },
};
use config::{Config, File};
use serde::Deserialize;
use std::collections::HashMap;

/// App-specific settings
///
/// Example:
///
/// ```yml
/// app:
///   hostname: 0.0.0.0
///   port: 1234
///   database_url: sqlite://autopulse.db
///   log_level: debug
/// ```
#[derive(Deserialize, Clone)]
pub struct App {
    /// Hostname to bind to
    pub hostname: String,
    /// Port to bind to (default: 2875)
    pub port: u16,
    /// Database URL (see [AnyConnection](crate::db::conn::AnyConnection))
    pub database_url: String,
    /// Log level (default: info) (trace, debug, info, warn, error)
    // TODO: change to enum?
    pub log_level: String,
}

/// Authentication settings
///
/// Example:
///
/// ```yml
/// auth:
///   username: terry
///   password: yogurt
/// ```
#[derive(Deserialize, Clone)]
pub struct Auth {
    /// Username for basic auth (default: admin)
    pub username: String,
    /// Password for basic auth (default: password)
    pub password: String,
}

/// Global settings
///
/// Example:
///
/// ```yml
/// opts:
///   check_path: true
///   max_retries: 10
///   default_timer_wait: 300
///   cleanup_days: 7
/// ```
#[derive(Deserialize, Clone)]
pub struct Opts {
    /// Check if the path exists before processing (default: false)
    pub check_path: bool,
    /// Maximum retries before giving up (default: 5)
    pub max_retries: i32,
    /// Default timer wait time (default: 60)
    pub default_timer_wait: u64,
    /// Cleanup events older than x days (default: 10)
    pub cleanup_days: u64,
}

/// autopulse settings
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
            .try_deserialize::<Self>()
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

/// Rewrite structure for triggers
///
/// Example:
///
/// ```yml
/// triggers:
///   sonarr:
///     type: sonarr
///     rewrite:
///       from: /tv
///       to: /media/tv
#[derive(Deserialize, Clone)]
pub struct Rewrite {
    /// Path to rewrite from
    pub from: String,
    /// Path to rewrite to
    pub to: String,
}

/// [Triggers](crate::service::triggers) for the service
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
    Notify(Notify),
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

/// [Webhooks](crate::service::webhooks) for the service
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

/// [Targets](crate::service::targets) for the service
#[derive(Deserialize, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Target {
    Plex(Plex),
    Jellyfin(Emby),
    Emby(Emby),
    Tdarr(Tdarr),
    Command(Command),
}

impl Target {
    pub async fn process(&mut self, evs: &[&ScanEvent]) -> anyhow::Result<Vec<String>> {
        match self {
            Self::Plex(p) => p.process(evs).await,
            Self::Jellyfin(j) => j.process(evs).await,
            Self::Emby(e) => e.process(evs).await,
            Self::Command(c) => c.process(evs).await,
            Self::Tdarr(t) => t.process(evs).await,
        }
    }
}
