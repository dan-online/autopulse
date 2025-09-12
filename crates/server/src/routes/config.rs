use crate::middleware::auth::check_auth;
use actix_web::{get, post, web, HttpResponse, Result};
use actix_web_httpauth::extractors::basic::BasicAuth;
use autopulse_service::manager::PulseManager;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Deserialize)]
pub struct TemplateQuery {
    pub include_examples: Option<bool>,
    pub database_type: Option<String>, // "sqlite" or "postgres"
    pub trigger_types: Option<String>, // "manual,sonarr,radarr"
    pub target_types: Option<String>,  // "plex,jellyfin,emby"
}

#[derive(Serialize)]
pub struct TemplateResponse {
    pub app_config: String,
    pub trigger_templates: HashMap<String, String>,
    pub target_templates: HashMap<String, String>,
    pub example_config: Option<String>,
    pub version: String,
}

#[derive(Deserialize)]
pub struct MergeRequest {
    pub base_template: String,
    pub trigger_configs: HashMap<String, String>,
    pub target_configs: HashMap<String, String>,
}

#[derive(Serialize)]
pub struct MergeResponse {
    pub merged_config: String,
    pub validation_warnings: Vec<String>,
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
    // Authenticate user
    if !check_auth(
        &auth,
        &manager.settings.auth.enabled,
        &manager.settings.auth.username,
        &manager.settings.auth.password,
    ) {
        return Ok(HttpResponse::Unauthorized().json("Authentication required"));
    }

    let database_type = query.database_type.as_deref().unwrap_or("sqlite");
    let trigger_types: Vec<&str> = query
        .trigger_types
        .as_deref()
        .unwrap_or("manual")
        .split(',')
        .collect();
    let target_types: Vec<&str> = query
        .target_types
        .as_deref()
        .unwrap_or("plex")
        .split(',')
        .collect();
    let include_examples = query.include_examples.unwrap_or(false);

    let response = generate_config_template(database_type, &trigger_types, &target_types, include_examples);
    
    Ok(HttpResponse::Ok().json(response))
}

/// POST /api/config-merge
/// 
/// Merges provided trigger and target configurations with the base template.
/// This is optional - external applications can also merge configurations themselves.
#[post("/api/config-merge")]
pub async fn config_merge(
    request: web::Json<MergeRequest>,
    auth: Option<BasicAuth>,
    manager: web::Data<Arc<PulseManager>>,
) -> Result<HttpResponse> {
    // Authenticate user
    if !check_auth(
        &auth,
        &manager.settings.auth.enabled,
        &manager.settings.auth.username,
        &manager.settings.auth.password,
    ) {
        return Ok(HttpResponse::Unauthorized().json("Authentication required"));
    }

    let req = request.into_inner();
    let response = merge_configurations(&req);
    
    Ok(HttpResponse::Ok().json(response))
}

fn generate_config_template(database_type: &str, trigger_types: &[&str], target_types: &[&str], include_examples: bool) -> TemplateResponse {
    // Base app configuration
    let app_config = match database_type {
        "postgres" => {
            r#"[app]
database_url = "postgres://autopulse:autopulse@localhost:5432/autopulse"
log_level = "info"
hostname = "0.0.0.0"
port = 2875"#
        },
        _ => {
            r#"[app]
database_url = "sqlite://data/autopulse.db"
log_level = "info"
hostname = "0.0.0.0"
port = 2875"#
        }
    };
    
    // Trigger templates
    let mut trigger_templates = HashMap::new();
    trigger_templates.insert("manual".to_string(), 
        r#"[triggers.{name}]
type = "manual"
# Optional: rewrite paths
# rewrite.from = "/source/path"
# rewrite.to = "/target/path"
# Optional: timer settings
# timer.wait = 30"#.to_string());
    
    if trigger_types.contains(&"sonarr") {
        trigger_templates.insert("sonarr".to_string(),
            r#"[triggers.{name}]
type = "sonarr"
# Optional: rewrite paths
# rewrite.from = "/downloads"
# rewrite.to = "/media/tv"
# Optional: timer settings
# timer.wait = 30"#.to_string());
    }
    
    if trigger_types.contains(&"radarr") {
        trigger_templates.insert("radarr".to_string(),
            r#"[triggers.{name}]
type = "radarr"
# Optional: rewrite paths
# rewrite.from = "/downloads"
# rewrite.to = "/media/movies"
# Optional: timer settings
# timer.wait = 30"#.to_string());
    }
    
    // Target templates
    let mut target_templates = HashMap::new();
    
    if target_types.contains(&"plex") {
        target_templates.insert("plex".to_string(),
            r#"[targets.{name}]
type = "plex"
url = "{url}"
token = "{token}"
refresh = true
analyze = false
# Optional: rewrite paths
# rewrite.from = "/media"
# rewrite.to = "/plex/media""#.to_string());
    }
    
    if target_types.contains(&"jellyfin") {
        target_templates.insert("jellyfin".to_string(),
            r#"[targets.{name}]
type = "jellyfin"
url = "{url}"
token = "{token}"
# Optional: rewrite paths
# rewrite.from = "/media"
# rewrite.to = "/jellyfin/media""#.to_string());
    }
    
    if target_types.contains(&"emby") {
        target_templates.insert("emby".to_string(),
            r#"[targets.{name}]
type = "emby"
url = "{url}"
token = "{token}"
# Optional: rewrite paths
# rewrite.from = "/media"
# rewrite.to = "/emby/media""#.to_string());
    }
    
    // Example configuration if requested
    let example_config = if include_examples {
        Some(format!(r#"# Complete example configuration
{}

[triggers.my_manual]
type = "manual"

[triggers.my_sonarr]
type = "sonarr"
rewrite.from = "/downloads"
rewrite.to = "/media/tv"

[targets.my_plex]
type = "plex"
url = "http://plex:32400"
token = "your-plex-token"
refresh = true
analyze = false"#, app_config))
    } else {
        None
    };
    
    TemplateResponse {
        app_config: app_config.to_string(),
        trigger_templates,
        target_templates,
        example_config,
        version: "1.3.2".to_string(),
    }
}

fn merge_configurations(req: &MergeRequest) -> MergeResponse {
    let mut merged_config = req.base_template.clone();
    let mut warnings = Vec::new();
    
    // Add trigger configurations
    for (_name, config) in &req.trigger_configs {
        merged_config.push_str("\n\n");
        merged_config.push_str(config);
    }
    
    // Add target configurations
    for (_name, config) in &req.target_configs {
        merged_config.push_str("\n\n");
        merged_config.push_str(config);
    }
    
    // Basic validation
    if !merged_config.contains("[app]") {
        warnings.push("Missing [app] section".to_string());
    }
    
    if !merged_config.contains("database_url") {
        warnings.push("Missing database_url configuration".to_string());
    }
    
    MergeResponse {
        merged_config,
        validation_warnings: warnings,
    }
}
