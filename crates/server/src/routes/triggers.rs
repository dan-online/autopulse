use crate::middleware::auth::AuthenticatedUser;
use actix_web::{
    get, post,
    web::{Bytes, Data, Path, Query},
    HttpRequest, HttpResponse, Result,
};
use autopulse_database::models::{FoundStatus, NewScanEvent};
use autopulse_service::settings::triggers::{autoscan::AutoscanQueryParams, Trigger};
use autopulse_service::{
    manager::PulseManager, settings::triggers::manual::ManualQueryParams,
    settings::webhooks::EventType,
};
use autopulse_utils::sify;
use serde::Deserialize;
use tracing::{debug, debug_span, error, info};

#[derive(Deserialize)]
#[serde(untagged)]
enum TriggerQueryParams {
    Manual(ManualQueryParams),
    Autoscan(AutoscanQueryParams),
}

async fn trigger_post_inner(
    req: &HttpRequest,
    trigger_name: &str,
    manager: &Data<PulseManager>,
    body: Bytes,
) -> Result<HttpResponse> {
    if body.is_empty() {
        let query = Query::<TriggerQueryParams>::from_query(req.query_string())
            .map_err(actix_web::error::ErrorBadRequest)?;
        return trigger_get_inner(query.into_inner(), trigger_name, manager).await;
    }

    let body: serde_json::Value =
        serde_json::from_slice(&body).map_err(actix_web::error::ErrorBadRequest)?;

    let Some(trigger_settings) = manager.settings.triggers.get(trigger_name) else {
        return Ok(HttpResponse::NotFound().body("Trigger not found"));
    };

    match trigger_settings {
        Trigger::Manual(_) | Trigger::Notify(_) => {
            Ok(HttpResponse::BadRequest().body("Invalid request"))
        }
        _ => {
            let rewrite = trigger_settings.get_rewrite();
            let (event_name, paths) = match trigger_settings.paths(body) {
                Ok(v) => v,
                Err(e) => {
                    error!("failed to decode request: {e}");
                    return Ok(HttpResponse::InternalServerError().body("Unable to parse request"));
                }
            };
            let timer = trigger_settings.get_timer(Some(event_name));

            let mut scan_events = vec![];
            let mut queued_paths = vec![];

            let mut excluded_paths: Vec<String> = vec![];

            for path in &paths {
                let (mut path, search) = path.clone();

                if let Some(rewrite) = &rewrite {
                    path = rewrite.rewrite_path(path);
                }

                if !trigger_settings.should_process_path(&path) {
                    tracing::trace!("trigger '{trigger_name}' filtered path '{path}'");
                    excluded_paths.push(path);
                    continue;
                }

                let new_scan_event = NewScanEvent {
                    event_source: trigger_name.to_owned(),
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

                match manager.add_event(&new_scan_event) {
                    Ok(scan_event) => {
                        queued_paths.push(path.clone());
                        scan_events.push(scan_event);
                    }
                    Err(e) => error!("failed to add event for '{}': {e}", path),
                }
            }

            if scan_events.len() + excluded_paths.len() != paths.len() {
                return Ok(HttpResponse::InternalServerError().body("failed to add all events"));
            }

            if !queued_paths.is_empty() {
                manager
                    .webhooks
                    .add_event(EventType::New, Some(trigger_name.to_owned()), &queued_paths)
                    .await;
            }

            debug_span!("", trigger = trigger_name).in_scope(|| {
                info!("added {} file{}", scan_events.len(), sify(&scan_events));
            });

            Ok(HttpResponse::Ok().json(scan_events))
        }
    }
}

#[post("/triggers/{trigger}")]
pub async fn trigger_post(
    req: HttpRequest,
    trigger: Path<String>,
    manager: Data<PulseManager>,
    _auth: AuthenticatedUser,
    body: Bytes,
) -> Result<HttpResponse> {
    let trigger_name = trigger.into_inner();
    trigger_post_inner(&req, &trigger_name, &manager, body).await
}

/// A-Train hardcodes its outbound URL to `/triggers/a-train/{drive_id}` and
/// won't let the user reshape it. This sibling route swallows the trailing
/// `{drive_id}` segment so the same trigger handler can serve it.
///
/// Returns 404 unless the resolved trigger is `Trigger::Atrain`, so a
/// stray `POST /triggers/sonarr/anything` can't sneak past as a valid
/// sonarr request.
#[post("/triggers/{trigger}/{_drive_id}")]
pub async fn trigger_post_rest(
    req: HttpRequest,
    path: Path<(String, String)>,
    manager: Data<PulseManager>,
    _auth: AuthenticatedUser,
    body: Bytes,
) -> Result<HttpResponse> {
    let (trigger_name, _drive_id) = path.into_inner();

    match manager.settings.triggers.get(&trigger_name) {
        Some(Trigger::Atrain(_)) => trigger_post_inner(&req, &trigger_name, &manager, body).await,
        _ => Ok(HttpResponse::NotFound().body("Trigger not found")),
    }
}

async fn trigger_get_inner(
    query: TriggerQueryParams,
    trigger_name: &str,
    manager: &Data<PulseManager>,
) -> Result<HttpResponse> {
    let Some(trigger_settings) = manager.settings.triggers.get(trigger_name) else {
        return Ok(HttpResponse::NotFound().body("Trigger not found"));
    };

    match trigger_settings {
        Trigger::Manual(trigger_settings) | Trigger::Bazarr(trigger_settings) => match query {
            TriggerQueryParams::Manual(query) => {
                let mut file_path = query.path.clone();

                if let Some(rewrite) = &trigger_settings.rewrite {
                    file_path = rewrite.rewrite_path(file_path);
                }

                if !trigger_settings.filter.allows(&file_path) {
                    tracing::trace!("trigger '{trigger_name}' filtered path '{file_path}'");
                    return Ok(HttpResponse::NoContent().finish());
                }

                let new_scan_event = NewScanEvent {
                    event_source: trigger_name.to_owned(),
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

                let scan_event = match manager.add_event(&new_scan_event) {
                    Ok(ev) => ev,
                    Err(e) => {
                        return Ok(HttpResponse::InternalServerError().body(e.to_string()));
                    }
                };

                manager
                    .webhooks
                    .add_event(
                        EventType::New,
                        Some(trigger_name.to_owned()),
                        &[file_path.clone()],
                    )
                    .await;

                debug_span!("", trigger = trigger_name).in_scope(|| {
                    info!("added 1 file");
                    debug!("added file '{}'", file_path);
                });

                Ok(HttpResponse::Ok().json(scan_event))
            }
            _ => Ok(HttpResponse::BadRequest().body("Invalid query parameters")),
        },
        Trigger::Autoscan(trigger_settings) => match query {
            TriggerQueryParams::Autoscan(query) => {
                let mut dir_path = query.dir.clone();

                if let Some(rewrite) = &trigger_settings.rewrite {
                    dir_path = rewrite.rewrite_path(dir_path);
                }

                if !trigger_settings.filter.allows(&dir_path) {
                    tracing::trace!("trigger '{trigger_name}' filtered path '{dir_path}'");
                    return Ok(HttpResponse::NoContent().finish());
                }

                let new_scan_event = NewScanEvent {
                    event_source: trigger_name.to_owned(),
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

                let scan_event = match manager.add_event(&new_scan_event) {
                    Ok(ev) => ev,
                    Err(e) => {
                        return Ok(HttpResponse::InternalServerError().body(e.to_string()));
                    }
                };

                manager
                    .webhooks
                    .add_event(
                        EventType::New,
                        Some(trigger_name.to_owned()),
                        std::slice::from_ref(&dir_path),
                    )
                    .await;

                debug_span!("", trigger = trigger_name).in_scope(|| {
                    info!("added 1 directory");
                    debug!("added directory '{}'", dir_path);
                });

                Ok(HttpResponse::Ok().json(scan_event))
            }
            _ => Ok(HttpResponse::BadRequest().body("Invalid query parameters")),
        },
        _ => Ok(HttpResponse::Ok().body("Not implemented")),
    }
}

#[get("/triggers/{trigger}")]
pub async fn trigger_get(
    query: Query<TriggerQueryParams>,
    trigger: Path<String>,
    manager: Data<PulseManager>,
    _auth: AuthenticatedUser,
) -> Result<HttpResponse> {
    let trigger_name = trigger.into_inner();
    trigger_get_inner(query.into_inner(), &trigger_name, &manager).await
}
