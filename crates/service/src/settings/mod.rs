use app::App;
use auth::Auth;
use figment::{
    providers::{Env, Format, Json, Toml, Yaml},
    Figment,
};
use opts::Opts;
use serde::{Deserialize, Serialize};
use std::env;
use std::path::Path;
use std::{collections::HashMap, path::PathBuf};
use targets::Target;
use triggers::manual::Manual;
use triggers::Trigger;
use webhooks::Webhook;

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

/// Path-level include/exclude filters for triggers and targets.
pub mod path_filter;

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
pub use autopulse_utils::rewrite;

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
/// [Triggers](triggers) for all triggers
pub mod triggers;

/// Target structure
///
/// [Targets](targets) for all targets
pub mod targets;

/// Webhook structure
///
/// [Webhooks](webhooks) for all webhooks
pub mod webhooks;

#[doc(hidden)]
pub fn default_triggers() -> HashMap<String, Trigger> {
    let mut triggers = HashMap::new();

    triggers.insert(
        "manual".to_string(),
        Trigger::Manual(Manual {
            rewrite: None,
            timer: None,
            excludes: vec![],
            filter: Default::default(),
        }),
    );

    triggers
}

#[derive(Serialize, Deserialize, Clone)]
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

impl Default for Settings {
    fn default() -> Self {
        Self {
            app: App::default(),
            auth: Auth::default(),
            opts: Opts::default(),
            triggers: default_triggers(),
            targets: HashMap::new(),
            webhooks: HashMap::new(),
            anchors: vec![],
        }
    }
}

impl Settings {
    /// Candidate filenames probed when no explicit `--config` is given.
    /// Order is significant — the first match wins. The set is what
    /// figment's bundled providers handle natively; json5/ron/ini are
    /// not supported (dropped from the prior `config` crate setup).
    pub const CONFIG_CANDIDATES: &'static [&'static str] =
        &["config.toml", "config.yaml", "config.yml", "config.json"];

    pub fn resolved_config_path(cwd: &Path) -> Option<PathBuf> {
        Self::CONFIG_CANDIDATES
            .iter()
            .map(|name| cwd.join(name))
            .find(|p| p.is_file())
    }

    pub fn searched_paths(cwd: &Path) -> Vec<PathBuf> {
        Self::CONFIG_CANDIDATES
            .iter()
            .map(|n| cwd.join(n))
            .collect()
    }

    /// Expands `AUTOPULSE__KEY__FILE=/path` environment variables into
    /// `AUTOPULSE__KEY=<file contents>` so figment's `Env` provider can
    /// read secrets sourced from files (Docker/Kubernetes secret mounts).
    ///
    /// # Safety
    ///
    /// Mutates the process environment. Must be called from `get_settings`
    /// at startup, before any worker thread reads env vars. The trade-off
    /// vs. a custom Provider is paid here so the figment chain stays
    /// idiomatic.
    unsafe fn expand_file_env_vars() {
        let pending: Vec<(String, String)> = env::vars()
            .filter_map(|(k, v)| {
                let base = k.strip_suffix("__FILE")?.to_string();
                let contents = std::fs::read_to_string(&v).ok()?;
                Some((base, contents.trim().to_string()))
            })
            .collect();
        for (k, v) in pending {
            // SAFETY: caller guarantees single-threaded startup context.
            unsafe { env::set_var(k, v) };
        }
    }

    pub fn get_settings(optional_config_file: Option<String>) -> anyhow::Result<Self> {
        let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

        let explicit = optional_config_file.is_some();
        let chosen = optional_config_file
            .map(PathBuf::from)
            .or_else(|| Self::resolved_config_path(&cwd));

        // Explicit --config path must exist. Silent fall-through to defaults
        // when the user told us exactly where the file is would be the same
        // bug class as #435.
        if explicit {
            if let Some(p) = &chosen {
                if !p.is_file() {
                    anyhow::bail!("--config {} does not exist", p.display());
                }
            }
        }

        // SAFETY: runs once at process startup before tokio spawns workers.
        unsafe { Self::expand_file_env_vars() };

        let mut fig = Figment::new();
        if let Some(p) = chosen.as_deref() {
            fig = match p.extension().and_then(|s| s.to_str()) {
                Some("toml") => fig.merge(Toml::file(p)),
                Some("yaml") | Some("yml") => fig.merge(Yaml::file(p)),
                Some("json") => fig.merge(Json::file(p)),
                other => anyhow::bail!(
                    "unsupported config format {:?} for {} (supported: toml, yaml, yml, json)",
                    other.unwrap_or(""),
                    p.display()
                ),
            };
        }
        // Env overrides file (last-write-wins).
        fig = fig.merge(Env::prefixed("AUTOPULSE__").split("__"));

        let mut settings: Self = fig.extract().map_err(|e| anyhow::anyhow!(e))?;
        settings.add_default_manual_trigger()?;

        // Use figment's own provenance: ask each merged provider where
        // its data came from. Avoids tracking "loaded_from" ourselves.
        let mut saw_file = false;
        for md in fig.metadata() {
            if let Some(src) = md.source.as_ref() {
                tracing::info!(target: "autopulse", "loaded config from {} ({})", src, md.name);
                saw_file = true;
            } else {
                tracing::debug!(target: "autopulse", "config provider: {}", md.name);
            }
        }
        if !saw_file && !explicit {
            tracing::warn!(
                target: "autopulse",
                "no config file found in {}. Searched: {:?}. \
                 Using defaults + environment overrides only. \
                 Pass --config /path/to/config.toml or place one of the candidate files in the cwd.",
                cwd.display(),
                Self::searched_paths(&cwd)
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>(),
            );
        }

        Ok(settings)
    }

    /// Emits an INFO-level summary of the effective config shape at startup.
    /// Just counts — keeps the line short and confirms the parse worked.
    pub fn log_summary(&self) {
        tracing::info!(
            target: "autopulse",
            "effective config: triggers={} targets={} webhooks={} anchors={}",
            self.triggers.len(),
            self.targets.len(),
            self.webhooks.len(),
            self.anchors.len(),
        );
    }

    pub fn add_default_manual_trigger(&mut self) -> anyhow::Result<()> {
        if !self.triggers.contains_key("manual") {
            self.triggers.insert(
                "manual".to_string(),
                Trigger::Manual(Manual {
                    rewrite: None,
                    timer: None,
                    excludes: vec![],
                    filter: Default::default(),
                }),
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn resolved_config_path_picks_toml_when_present() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config.toml");
        std::fs::File::create(&p).unwrap().write_all(b"").unwrap();
        assert_eq!(Settings::resolved_config_path(dir.path()), Some(p));
    }

    #[test]
    fn resolved_config_path_returns_none_when_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        assert!(Settings::resolved_config_path(dir.path()).is_none());
    }

    #[test]
    fn resolved_config_path_prefers_toml_over_yaml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::File::create(dir.path().join("config.yaml")).unwrap();
        std::fs::File::create(dir.path().join("config.toml")).unwrap();
        assert_eq!(
            Settings::resolved_config_path(dir.path())
                .unwrap()
                .file_name()
                .unwrap(),
            "config.toml"
        );
    }
}
