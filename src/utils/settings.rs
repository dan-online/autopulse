use crate::{
    db::models::ScanEvent,
    service::{
        targets::{
            autopulse::Autopulse, command::Command, emby::Emby, fileflows::FileFlows, plex::Plex,
            tdarr::Tdarr,
        },
        triggers::{
            lidarr::{Lidarr, LidarrRequest},
            manual::Manual,
            notify::Notify,
            radarr::{Radarr, RadarrRequest},
            readarr::ReadarrRequest,
            sonarr::{Sonarr, SonarrRequest},
        },
        webhooks::{discord::DiscordWebhook, WebhookBatch},
    },
};
use base64::prelude::*;
use config::{Config, FileFormat};
use serde::Deserialize;
use std::{collections::HashMap, path::PathBuf};

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

impl Auth {
    pub fn to_auth_encoded(&self) -> String {
        format!(
            "Basic {}",
            BASE64_STANDARD.encode(format!("{}:{}", self.username, self.password))
        )
    }
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
    /// Log file path
    pub log_file: Option<PathBuf>,
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
    pub fn get_settings(optional_config_file: Option<String>) -> anyhow::Result<Self> {
        let mut settings = Config::builder()
            .add_source(config::File::from_str(
                include_str!("../../default.toml"),
                FileFormat::Toml,
            ))
            .add_source(config::File::with_name("config").required(false))
            .add_source(config::Environment::with_prefix("AUTOPULSE").separator("__"));

        if let Some(file_loc) = optional_config_file {
            settings = settings.add_source(config::File::with_name(&file_loc));
        }

        let settings = settings.build()?;

        settings
            .try_deserialize::<Self>()
            .map_err(|e| anyhow::anyhow!(e))
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

/// Timer structure for triggers
///
/// Example:
///
/// ```yml
/// triggers:
///  sonarr:
///   type: sonarr
///   timer:
///    wait: 300 # wait 5 minutes before processing
/// ```
#[derive(Deserialize, Clone, Default)]
pub struct Timer {
    /// Time to wait before processing
    pub wait: Option<u64>,
}

/// [Triggers](crate::service::triggers) for the service
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

/// [Webhooks](crate::service::webhooks) for the service
#[derive(Deserialize, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Webhook {
    Discord(DiscordWebhook),
}

impl Webhook {
    pub async fn send(&self, batch: &WebhookBatch) -> anyhow::Result<()> {
        match self {
            Self::Discord(d) => d.send(batch).await,
        }
    }
}

pub trait TargetProcess {
    fn process<'a>(
        &self,
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
