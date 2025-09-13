use crate::middleware::auth::check_auth;
use actix_web::{
    get, post,
    web::{Data, Json, Path, Query},
    HttpResponse, Result,
};
use actix_web_httpauth::extractors::basic::BasicAuth;
use autopulse_database::models::{FoundStatus, NewScanEvent};
use autopulse_service::settings::triggers::{autoscan::AutoscanQueryParams, Trigger};
use autopulse_service::{
    manager::PulseManager, settings::triggers::manual::ManualQueryParams,
    settings::webhooks::EventType,
};
use autopulse_utils::sify;
use serde::Deserialize;
use std::sync::Arc;
use tracing::{debug_span, error, info};

#[derive(Deserialize)]
#[serde(untagged)]
enum TriggerQueryParams {
    Manual(ManualQueryParams),
    Autoscan(AutoscanQueryParams),
}

#[post("/triggers/{trigger}")]
pub async fn trigger_post(
    trigger: Path<String>,
    manager: Data<Arc<PulseManager>>,
    auth: Option<BasicAuth>,
    body: Json<serde_json::Value>,
) -> Result<HttpResponse> {
    if !check_auth(
        &auth,
        &manager.settings.auth.enabled,
        &manager.settings.auth.username,
        &manager.settings.auth.password,
    ) {
        return Ok(HttpResponse::Unauthorized().body("Unauthorized"));
    }

    let trigger_settings = manager.settings.triggers.get(&trigger.to_string());

    if trigger_settings.is_none() {
        return Ok(HttpResponse::NotFound().body("Trigger not found"));
    }

    let trigger_settings = trigger_settings.unwrap();

    match trigger_settings {
        Trigger::Manual(_) | Trigger::Notify(_) => {
            Ok(HttpResponse::BadRequest().body("Invalid request"))
        }
        _ => {
            let rewrite = trigger_settings.get_rewrite();
            let decoded = trigger_settings.paths(body.into_inner());

            if let Err(e) = decoded {
                error!("failed to decode request: {e}");

                return Ok(HttpResponse::InternalServerError().body("Unable to parse request"));
            }

            let (event_name, paths) = decoded.unwrap();
            let timer = trigger_settings.get_timer(Some(event_name));

            let mut scan_events = vec![];

            for path in &paths {
                let (mut path, search) = path.clone();

                if let Some(rewrite) = &rewrite {
                    path = rewrite.rewrite_path(path);
                }

                let new_scan_event = NewScanEvent {
                    event_source: trigger.to_string(),
                    file_path: path.clone(),
                    found_status: if !search {
                        FoundStatus::Found.into()
                    } else {
                        FoundStatus::NotFound.into()
                    },
                    can_process: chrono::Utc::now().naive_utc()
                        + chrono::Duration::seconds(
                            timer
                                .wait
                                .unwrap_or(manager.settings.opts.default_timer_wait)
                                as i64,
                        ),
                    ..Default::default()
                };

                let scan_event = manager.add_event(&new_scan_event);

                if let Ok(scan_event) = scan_event {
                    scan_events.push(scan_event);
                }
            }

            manager
                .webhooks
                .add_event(
                    EventType::New,
                    Some(trigger.to_string()),
                    &paths
                        .clone()
                        .into_iter()
                        .map(|p| p.0)
                        .collect::<Vec<String>>(),
                )
                .await;

            debug_span!("", trigger = trigger.to_string()).in_scope(|| {
                info!("added {} file{}", scan_events.len(), sify(&scan_events));
            });

            if scan_events.len() != paths.len() {
                return Ok(HttpResponse::InternalServerError().body("Failed to add all events"));
            }

            Ok(HttpResponse::Ok().json(scan_events))
        }
    }
}

#[get("/triggers/{trigger}")]
pub async fn trigger_get(
    query: Query<TriggerQueryParams>,
    trigger: Path<String>,
    manager: Data<Arc<PulseManager>>,
    auth: Option<BasicAuth>,
) -> Result<HttpResponse> {
    if !check_auth(
        &auth,
        &manager.settings.auth.enabled,
        &manager.settings.auth.username,
        &manager.settings.auth.password,
    ) {
        return Ok(HttpResponse::Unauthorized().body("Unauthorized"));
    }

    let trigger_settings = manager.settings.triggers.get(&trigger.to_string());

    if trigger_settings.is_none() {
        return Ok(HttpResponse::NotFound().body("Trigger not found"));
    }

    let trigger_settings = trigger_settings.unwrap();

    match &trigger_settings {
        Trigger::Manual(trigger_settings) => match query.into_inner() {
            TriggerQueryParams::Manual(query) => {
                let mut file_path = query.path.clone();

                if let Some(rewrite) = &trigger_settings.rewrite {
                    // file_path = rewrite_path(file_path, rewrite);
                    file_path = rewrite.rewrite_path(file_path);
                }

                let new_scan_event = NewScanEvent {
                    event_source: trigger.to_string(),
                    file_path: file_path.clone(),
                    file_hash: query.hash.clone(),
                    can_process: chrono::Utc::now().naive_utc()
                        + chrono::Duration::seconds(
                            trigger_settings
                                .timer
                                .clone()
                                .unwrap_or_default()
                                .wait
                                .unwrap_or(manager.settings.opts.default_timer_wait)
                                as i64,
                        ),
                    ..Default::default()
                };

                let scan_event = manager.add_event(&new_scan_event);

                if let Err(e) = scan_event {
                    return Ok(HttpResponse::InternalServerError().body(e.to_string()));
                }

                manager
                    .webhooks
                    .add_event(
                        EventType::New,
                        Some(trigger.to_string()),
                        &[file_path.clone()],
                    )
                    .await;

                debug_span!("", trigger = trigger.to_string()).in_scope(|| {
                    info!("added 1 file");
                });

                let scan_event = scan_event.unwrap();

                Ok(HttpResponse::Ok().json(scan_event))
            }
            _ => Ok(HttpResponse::BadRequest().body("Invalid query parameters")),
        },
        Trigger::Autoscan(trigger_settings) => match query.into_inner() {
            TriggerQueryParams::Autoscan(query) => {
                let dir_path = query.dir.clone();

                let new_scan_event = NewScanEvent {
                    event_source: trigger.to_string(),
                    file_path: dir_path.clone(),
                    can_process: chrono::Utc::now().naive_utc()
                        + chrono::Duration::seconds(
                            trigger_settings
                                .timer
                                .clone()
                                .unwrap_or_default()
                                .wait
                                .unwrap_or(manager.settings.opts.default_timer_wait)
                                as i64,
                        ),
                    ..Default::default()
                };

                let scan_event = manager.add_event(&new_scan_event);

                if let Err(e) = scan_event {
                    return Ok(HttpResponse::InternalServerError().body(e.to_string()));
                }

                manager
                    .webhooks
                    .add_event(
                        EventType::New,
                        Some(trigger.to_string()),
                        std::slice::from_ref(&dir_path),
                    )
                    .await;

                debug_span!("", trigger = trigger.to_string()).in_scope(|| {
                    info!("added 1 directory");
                });

                let scan_event = scan_event.unwrap();

                Ok(HttpResponse::Ok().json(scan_event))
            }
            _ => Ok(HttpResponse::BadRequest().body("Invalid query parameters")),
        },
        _ => Ok(HttpResponse::Ok().body("Not implemented")),
    }
}
