use crate::service::triggers::manual::Manual;
use app::App;
use auth::Auth;
use config::Config;
use opts::Opts;
use serde::Deserialize;
use std::{collections::HashMap, path::PathBuf};
use target::Target;
use timer::Timer;
use trigger::Trigger;
use webhook::Webhook;

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
pub mod app;

/// Authentication settings
///
/// Example:
///
/// ```yml
/// auth:
///   username: terry
///   password: yogurt
/// ```
pub mod auth;

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
pub mod opts;

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
pub mod rewrite;

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
pub mod timer;

/// Trigger structure
///
/// [Triggers](crate::service::triggers) for the service
pub mod trigger;

/// Target structure
///
/// [Targets](crate::service::targets) for the service
pub mod target;

/// Webhook structure
///
/// [Webhooks](crate::service::webhooks) for the service
pub mod webhook;

#[doc(hidden)]
fn default_triggers() -> HashMap<String, Trigger> {
    let mut triggers = HashMap::new();

    triggers.insert(
        "manual".to_string(),
        Trigger::Manual(Manual {
            rewrite: None,
            timer: Timer::default(),
            excludes: vec![],
        }),
    );

    triggers
}

/// autopulse settings
#[derive(Deserialize, Clone)]
pub struct Settings {
    #[serde(default)]
    pub app: App,

    #[serde(default)]
    pub auth: Auth,

    #[serde(default)]
    pub opts: Opts,

    #[serde(default = "default_triggers")]
    pub triggers: HashMap<String, Trigger>,
    #[serde(default)]
    pub targets: HashMap<String, Target>,

    #[serde(default)]
    pub webhooks: HashMap<String, Webhook>,

    /// List of paths to anchor the service to
    ///
    /// This is useful to prevent the service notifying a target when the drive is not mounted or visible
    /// The contents of the file/directory are not tampered with, only the presence of the file/directory is checked
    ///
    /// Example:
    /// ```yml
    /// anchors:
    ///  - /mnt/media/tv # Directory
    ///  - /mnt/media/anchor # File
    /// ```
    #[serde(default)]
    pub anchors: Vec<PathBuf>,
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

        let mut settings = settings
            .try_deserialize::<Self>()
            .map_err(|e| anyhow::anyhow!(e))?;

        settings.add_default_manual_trigger()?;

        Ok(settings)
    }

    pub fn add_default_manual_trigger(&mut self) -> anyhow::Result<()> {
        if !self.triggers.contains_key("manual") {
            self.triggers.insert(
                "manual".to_string(),
                Trigger::Manual(Manual {
                    rewrite: None,
                    timer: Timer::default(),
                    excludes: vec![],
                }),
            );
        }

        Ok(())
    }
}
