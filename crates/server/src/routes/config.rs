use crate::middleware::auth::check_auth;
use actix_web::{get, web, HttpResponse, Result};
use actix_web_httpauth::extractors::basic::BasicAuth;
use autopulse_database::conn::DatabaseType;
use autopulse_service::manager::PulseManager;
use autopulse_service::settings::app::App;
use autopulse_service::settings::targets::{Target, TargetType};
use autopulse_service::settings::triggers::{Trigger, TriggerType};
use autopulse_service::settings::{default_triggers, Settings};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

const fn default_database_type() -> DatabaseType {
    DatabaseType::Sqlite
}

#[derive(Deserialize, Debug)]
pub struct TemplateQuery {
    /// Database type for the configuration (see [`DatabaseType`]) (default: sqlite)
    #[serde(default = "default_database_type")]
    pub database: DatabaseType,
    /// Comma-separated list of trigger types to include (see [`TriggerType`]) (default: manual)
    pub triggers: Option<String>,
    /// Comma-separated list of target types to include (see [`TargetType`])
    pub targets: Option<String>,
    /// Output format (json or toml) (default: toml)
    pub output: Option<OutputType>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum OutputType {
    Json,
    Toml,
}

#[derive(Serialize)]
pub struct TemplateResponse {
    pub config: String,
    pub version: String,
}

/// GET /api/config-template
///
/// Returns the base Autopulse configuration template with requested components.
/// This allows external applications to get the current Autopulse configuration
/// structure without hardcoding it.
#[get("/api/config-template")]
pub async fn config_template(
    query: web::Query<TemplateQuery>,
    auth: Option<BasicAuth>,
    manager: web::Data<Arc<PulseManager>>,
) -> Result<HttpResponse> {
    if !check_auth(
        &auth,
        &manager.settings.auth.enabled,
        &manager.settings.auth.username,
        &manager.settings.auth.password,
    ) {
        return Ok(HttpResponse::Unauthorized().json("Authentication required"));
    }

    let response = generate_config_template(
        &query.database,
        &serde_json::from_str::<Vec<TriggerType>>(&format!(
            "[{}]",
            query
                .triggers
                .as_ref()
                .map(|t| t
                    .split(',')
                    .map(|s| format!("\"{}\"", s.trim()))
                    .collect::<Vec<_>>()
                    .join(","))
                .unwrap_or_default()
        ))
        .map_err(|e| actix_web::error::ErrorBadRequest(format!("Invalid trigger types: {}", e)))?,
        &serde_json::from_str::<Vec<TargetType>>(&format!(
            "[{}]",
            query
                .targets
                .as_ref()
                .map(|t| t
                    .split(',')
                    .map(|s| format!("\"{}\"", s.trim()))
                    .collect::<Vec<_>>()
                    .join(","))
                .unwrap_or_default()
        ))
        .map_err(|e| actix_web::error::ErrorBadRequest(format!("Invalid target types: {}", e)))?,
        query.output.as_ref().unwrap_or(&OutputType::Toml),
    )
    .map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!(
            "Failed to generate config template: {}",
            e
        ))
    })?;

    Ok(HttpResponse::Ok().json(response))
}

/// Generates configuration templates based on requested database and service types.
///
/// This function creates the base app configuration for the specified database type
/// and includes only the requested trigger and target templates. Templates contain
/// placeholder values (like {name}, {url}, {token}) that consuming applications
/// can replace with actual values.
///
/// # Arguments
/// * `database` - The type of database to configure (see [`DatabaseType`]).
/// * `input_triggers` - A list of trigger types to include in the configuration.
/// * `input_targets` - A list of target types to include in the configuration.
/// * `output_type` - The desired output format for the configuration (see [`OutputType`]).
fn generate_config_template(
    database: &DatabaseType,
    input_triggers: &Vec<TriggerType>,
    input_targets: &Vec<TargetType>,
    output_type: &OutputType,
) -> anyhow::Result<impl Serialize> {
    let app = App {
        database_url: match database {
            DatabaseType::Sqlite => "sqlite://data/autopulse.db",
            DatabaseType::Postgres => "postgres://autopulse:autopulse@localhost:5432/autopulse",
        }
        .into(),
        ..Default::default()
    };

    let mut triggers = default_triggers();

    for trigger in input_triggers {
        triggers.insert(
            format!("my_{}", serde_json::to_string(trigger)?.replace('"', "")),
            match trigger {
                TriggerType::Manual => Trigger::Manual(serde_json::from_str(r#"{}"#)?),
                TriggerType::Bazarr => Trigger::Bazarr(serde_json::from_str(r#"{}"#)?),
                TriggerType::Autoscan => Trigger::Autoscan(serde_json::from_str(r#"{}"#)?),
                TriggerType::Radarr => Trigger::Radarr(serde_json::from_str(r#"{}"#)?),
                TriggerType::Sonarr => Trigger::Sonarr(serde_json::from_str(r#"{}"#)?),
                TriggerType::Lidarr => Trigger::Lidarr(serde_json::from_str(r#"{}"#)?),
                TriggerType::Readarr => Trigger::Readarr(serde_json::from_str(r#"{}"#)?),
                TriggerType::Notify => {
                    Trigger::Notify(serde_json::from_str(r#"{"paths": ["/media"]}"#)?)
                }
            },
        );
    }

    let mut targets = HashMap::new();

    for target in input_targets {
        targets.insert(
            format!("my_{}", serde_json::to_string(target)?.replace('"', "")),
            match target {
                TargetType::Plex => Target::Plex(serde_json::from_str(
                    r#"{"url": "{url}", "token": "{token}"}"#,
                )?),
                TargetType::Jellyfin => Target::Jellyfin(serde_json::from_str(
                    r#"{"url": "{url}", "token": "{token}"}"#,
                )?),
                TargetType::Emby => Target::Emby(serde_json::from_str(
                    r#"{"url": "{url}", "token": "{token}"}"#,
                )?),
                TargetType::Tdarr => Target::Tdarr(serde_json::from_str(
                    r#"{"url": "{url}", "db_id": "{library_id}"}"#,
                )?),
                TargetType::Sonarr => Target::Sonarr(serde_json::from_str(
                    r#"{"url": "{url}", "token": "{token}"}"#,
                )?),
                TargetType::Radarr => Target::Radarr(serde_json::from_str(
                    r#"{"url": "{url}", "token": "{token}"}"#,
                )?),
                TargetType::Command => Target::Command(serde_json::from_str(
                    r#"{"command": "echo 'Processing {path}'"}"#,
                )?),
                TargetType::FileFlows => {
                    Target::FileFlows(serde_json::from_str(r#"{"url": "{url}"}"#)?)
                }
                TargetType::Autopulse => {
                    Target::Autopulse(serde_json::from_str(r#"{"url": "{url}", "auth": {"username": "{username}", "password": "{password}" }}"#)?)
                }
            },
        );
    }

    let settings = Settings {
        app,
        triggers,
        targets,
        ..Default::default()
    };

    let app_config = match output_type {
        OutputType::Json => serde_json::to_string_pretty(&settings)?,
        OutputType::Toml => toml::to_string_pretty(&settings)?,
    };

    Ok(TemplateResponse {
        config: app_config,
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use autopulse_service::settings::targets::TargetType;
    use autopulse_service::settings::triggers::TriggerType;

    #[test]
    fn test_generate_config_template_json_manual_trigger_plex_target() {
        let triggers = vec![TriggerType::Manual];
        let targets = vec![TargetType::Plex];
        let result = generate_config_template(
            &DatabaseType::Sqlite,
            &triggers,
            &targets,
            &OutputType::Json,
        )
        .unwrap();

        let response = serde_json::to_value(&result).unwrap();
        assert!(response["config"].is_string());
        assert!(response["version"].is_string());
        let config_str = response["config"].as_str().unwrap();
        assert!(config_str.contains("my_manual"));
        assert!(config_str.contains("my_plex"));
        assert!(config_str.contains("sqlite://data/autopulse.db"));
    }

    #[test]
    fn test_generate_config_template_toml_multiple_triggers_targets() {
        let triggers = vec![
            TriggerType::Manual,
            TriggerType::Radarr,
            TriggerType::Sonarr,
        ];
        let targets = vec![TargetType::Jellyfin, TargetType::Tdarr];
        let result = generate_config_template(
            &DatabaseType::Postgres,
            &triggers,
            &targets,
            &OutputType::Toml,
        )
        .unwrap();

        let response = serde_json::to_value(&result).unwrap();
        assert!(response["config"].is_string());
        assert!(response["version"].is_string());
        let config_str = response["config"].as_str().unwrap();
        assert!(config_str.contains("my_manual"));
        assert!(config_str.contains("my_radarr"));
        assert!(config_str.contains("my_sonarr"));
        assert!(config_str.contains("my_jellyfin"));
        assert!(config_str.contains("my_tdarr"));
        assert!(config_str.contains("postgres://autopulse:autopulse@localhost:5432/autopulse"));
    }

    #[test]
    fn test_generate_config_template_empty_triggers_targets() {
        let triggers = vec![];
        let targets = vec![];
        let result = generate_config_template(
            &DatabaseType::Sqlite,
            &triggers,
            &targets,
            &OutputType::Json,
        )
        .unwrap();

        let response = serde_json::to_value(&result).unwrap();
        assert!(response["config"].is_string());
        assert!(response["version"].is_string());
        let config_str = response["config"].as_str().unwrap();
        // Should not contain any custom triggers/targets
        assert!(!config_str.contains("my_manual"));
        assert!(!config_str.contains("my_plex"));
    }

    #[test]
    fn test_generate_config_template_invalid_output_type() {
        let triggers = vec![TriggerType::Manual];
        let targets = vec![TargetType::Plex];
        // OutputType is always valid due to enum, so this test is not needed.
        // But we can test that TOML output parses.
        let result = generate_config_template(
            &DatabaseType::Sqlite,
            &triggers,
            &targets,
            &OutputType::Toml,
        )
        .unwrap();

        let response = serde_json::to_value(&result).unwrap();
        assert!(response["config"].is_string());
        let config_str = response["config"].as_str().unwrap();
        assert!(config_str.contains("my_manual"));
        assert!(config_str.contains("my_plex"));
        assert!(config_str.contains("sqlite://data/autopulse.db"));
    }
}
