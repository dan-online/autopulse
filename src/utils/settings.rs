use std::collections::HashMap;

use config::{Config, File};
use serde::Deserialize;

use crate::{
    db::models::ScanEvent,
    targets::{jellyfin::Jellyfin, plex::Plex},
    triggers::{radarr::RadarrRequest, sonarr::SonarrRequest},
};

#[derive(Deserialize, Clone, Debug)]
pub enum TriggerTypes {
    Manual,
    Radarr,
    Sonarr,
    Lidarr,
    Readarr,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Rewrite {
    pub from: String,
    pub to: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Trigger {
    #[serde(rename = "type")]
    pub t: TriggerTypes,
    pub rewrite: Option<Rewrite>,
}

impl Trigger {
    pub fn paths(&self, body: serde_json::Value) -> anyhow::Result<Vec<String>> {
        match &self.t {
            TriggerTypes::Sonarr => Ok(SonarrRequest::from_json(body)?.paths()),
            TriggerTypes::Radarr => Ok(RadarrRequest::from_json(body)?.paths()),
            _ => todo!(),
        }
    }
}

pub enum WebhookTypes {
    Discord,
    // Slack,
    // Telegram,
    // Manual,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Webhook {
    pub url: String,
    #[serde(rename = "type")]
    pub t: String,
}

pub trait TargetProcess {
    fn process(
        &self,
        file_path: &ScanEvent,
    ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send;
}

pub trait TriggerRequest {
    fn from_json(json: serde_json::Value) -> anyhow::Result<Self>
    where
        Self: Sized;

    fn paths(&self) -> Vec<String>;
}

#[derive(Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Target {
    Plex(Plex),
    Jellyfin(Jellyfin),
}

impl Target {
    pub async fn process(&self, ev: &ScanEvent) -> anyhow::Result<()> {
        match self {
            Target::Plex(p) => p.process(ev).await,
            Target::Jellyfin(j) => j.process(ev).await,
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct Settings {
    pub hostname: String,
    pub port: u16,
    pub database_url: String,

    pub username: String,
    pub password: String,

    pub check_path: bool,

    pub triggers: HashMap<String, Trigger>,
    pub targets: HashMap<String, Target>,

    pub webhooks: HashMap<String, Webhook>,
}

pub fn get_settings() -> anyhow::Result<Settings> {
    let settings = Config::builder()
        .add_source(File::with_name("default.toml"))
        .add_source(config::File::with_name("config").required(false))
        .add_source(config::Environment::with_prefix("AUTOPULSE"))
        .build()
        .unwrap();

    settings
        .try_deserialize::<Settings>()
        .map_err(|e| anyhow::anyhow!(e))
}
