use app::App;
use auth::Auth;
use config::Config;
use opts::Opts;
use serde::Deserialize;
use std::collections::HashMap;
use target::Target;
use trigger::Trigger;
use webhook::Webhook;

/// App-specific settings
pub mod app;

/// Authentication settings
pub mod auth;

/// Global settings
pub mod opts;

/// Rewrite structure
pub mod rewrite;

/// Timer structure
pub mod timer;

/// Trigger structure
pub mod trigger;

/// Target structure
pub mod target;

/// Webhook structure
pub mod webhook;

/// autopulse settings
#[derive(Deserialize, Clone)]
pub struct Settings {
    #[serde(default)]
    pub app: App,

    #[serde(default)]
    pub auth: Auth,

    #[serde(default)]
    pub opts: Opts,

    #[serde(default)]
    pub triggers: HashMap<String, Trigger>,
    #[serde(default)]
    pub targets: HashMap<String, Target>,

    #[serde(default)]
    pub webhooks: HashMap<String, Webhook>,
}

impl Settings {
    pub fn get_settings(optional_config_file: Option<String>) -> anyhow::Result<Self> {
        let mut settings = Config::builder()
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
