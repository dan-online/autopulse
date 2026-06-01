use anyhow::Context;
use app::App;
use auth::Auth;
use figment::{
    providers::{Env, Format, Json, Serialized, Toml, Yaml},
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
#[serde(default)]
pub struct Settings {
    pub app: App,

    pub auth: Auth,

    pub opts: Opts,

    pub triggers: HashMap<String, Trigger>,
    pub targets: HashMap<String, Target>,

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
    pub anchors: Vec<PathBuf>,
}

pub struct LoadedSettings {
    pub settings: Settings,
    pub diagnostics: Vec<ConfigDiagnostic>,
}

impl LoadedSettings {
    pub fn log_diagnostics(&self) {
        for diagnostic in &self.diagnostics {
            diagnostic.log();
        }
    }
}

pub enum ConfigDiagnostic {
    LoadedFile(PathBuf),
    LoadedFileEnv {
        count: usize,
    },
    MissingConfig {
        cwd: PathBuf,
        searched: Vec<PathBuf>,
    },
}

impl ConfigDiagnostic {
    fn log(&self) {
        match self {
            Self::LoadedFile(path) => {
                tracing::info!(target: "autopulse", "loaded config from {}", path.display());
            }
            Self::LoadedFileEnv { count } => {
                tracing::info!(
                    target: "autopulse",
                    "loaded {count} config override(s) from AUTOPULSE__...__FILE"
                );
            }
            Self::MissingConfig { cwd, searched } => {
                tracing::warn!(
                    target: "autopulse",
                    "no config file found in {}. Searched: {:?}. \
                     Using defaults + environment overrides only. \
                     Pass --config /path/to/config.toml or place one of the candidate files in the cwd.",
                    cwd.display(),
                    searched
                        .iter()
                        .map(|p| p.display().to_string())
                        .collect::<Vec<_>>(),
                );
            }
        }
    }
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

    fn file_env_overrides_from(
        vars: impl IntoIterator<Item = (String, String)>,
    ) -> anyhow::Result<Vec<(String, String)>> {
        const PREFIX: &str = "AUTOPULSE__";
        const SUFFIX: &str = "__FILE";

        vars.into_iter()
            .filter_map(|(key, path)| {
                let key_path = key
                    .strip_prefix(PREFIX)?
                    .strip_suffix(SUFFIX)?
                    .replace("__", ".")
                    .to_ascii_lowercase();
                Some((key, key_path, path))
            })
            .map(|(key, key_path, path)| {
                let contents = std::fs::read_to_string(&path)
                    .with_context(|| format!("failed to read file referenced by {key}: {path}"))?;

                Ok((key_path, contents.trim().to_string()))
            })
            .collect()
    }

    fn file_env_overrides() -> anyhow::Result<Vec<(String, String)>> {
        Self::file_env_overrides_from(env::vars())
    }

    pub fn get_settings(optional_config_file: Option<String>) -> anyhow::Result<LoadedSettings> {
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

        let mut diagnostics = vec![];
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
            diagnostics.push(ConfigDiagnostic::LoadedFile(p.to_path_buf()));
        } else if !explicit {
            diagnostics.push(ConfigDiagnostic::MissingConfig {
                cwd: cwd.clone(),
                searched: Self::searched_paths(&cwd),
            });
        }
        // Env overrides file (last-write-wins).
        fig = fig.merge(
            Env::prefixed("AUTOPULSE__")
                .filter(|key| !key.as_str().ends_with("__FILE"))
                .split("__"),
        );
        // File-secret env vars override direct env vars, matching the old
        // config source behavior without mutating the process environment.
        let file_overrides = Self::file_env_overrides()?;
        if !file_overrides.is_empty() {
            diagnostics.push(ConfigDiagnostic::LoadedFileEnv {
                count: file_overrides.len(),
            });
        }
        for (key, value) in file_overrides {
            fig = fig.merge(Serialized::default(&key, value));
        }

        let mut settings: Self = fig.extract().map_err(|e| anyhow::anyhow!(e))?;
        settings.normalize()?;

        Ok(LoadedSettings {
            settings,
            diagnostics,
        })
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

    pub fn normalize(&mut self) -> anyhow::Result<()> {
        self.add_default_manual_trigger()?;

        Ok(())
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

    #[test]
    fn file_env_overrides_read_autopulse_secret_files() {
        let dir = tempfile::tempdir().unwrap();
        let secret = dir.path().join("password");
        std::fs::write(&secret, "secret\n").unwrap();

        let vars = vec![
            (
                "AUTOPULSE__AUTH__PASSWORD__FILE".to_string(),
                secret.display().to_string(),
            ),
            ("OTHER__PASSWORD__FILE".to_string(), "ignored".to_string()),
        ];

        let overrides = Settings::file_env_overrides_from(vars).unwrap();

        assert_eq!(
            overrides,
            vec![("auth.password".to_string(), "secret".to_string())]
        );
    }

    #[test]
    fn file_env_overrides_key_paths_deserialize_into_settings() {
        let dir = tempfile::tempdir().unwrap();
        let secret = dir.path().join("password");
        std::fs::write(&secret, "secret\n").unwrap();

        let overrides = Settings::file_env_overrides_from(vec![(
            "AUTOPULSE__AUTH__PASSWORD__FILE".to_string(),
            secret.display().to_string(),
        )])
        .unwrap();
        let mut fig = Figment::new();
        for (key, value) in overrides {
            fig = fig.merge(Serialized::default(&key, value));
        }

        let settings = fig.extract::<Settings>().unwrap();

        assert_eq!(settings.auth.password, "secret");
    }

    #[test]
    fn file_env_overrides_error_when_secret_file_cannot_be_read() {
        let vars = vec![(
            "AUTOPULSE__AUTH__PASSWORD__FILE".to_string(),
            "/tmp/missing-autopulse-secret".to_string(),
        )];

        let err = Settings::file_env_overrides_from(vars).unwrap_err();

        assert!(
            err.to_string()
                .contains("failed to read file referenced by AUTOPULSE__AUTH__PASSWORD__FILE"),
            "{err:?}"
        );
    }

    #[test]
    fn empty_settings_deserialize_like_rust_default() {
        let settings: Settings = serde_json::from_str("{}").expect("empty settings should load");
        let default = Settings::default();

        assert_eq!(
            serde_json::to_value(&settings).expect("settings serialize"),
            serde_json::to_value(&default).expect("default settings serialize")
        );
        assert_eq!(settings.auth.enabled, default.auth.enabled);
        assert!(matches!(
            settings.triggers.get("manual"),
            Some(Trigger::Manual(_))
        ));
    }

    #[test]
    fn normalize_adds_manual_trigger_to_present_empty_trigger_map() {
        let mut settings: Settings =
            serde_json::from_str(r#"{"triggers":{}}"#).expect("settings should load");

        assert!(!settings.triggers.contains_key("manual"));

        settings.normalize().expect("settings should normalize");

        assert!(matches!(
            settings.triggers.get("manual"),
            Some(Trigger::Manual(_))
        ));
    }
}
