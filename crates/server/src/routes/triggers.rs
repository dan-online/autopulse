use crate::middleware::auth::check_auth;
use actix_web::{
    get, post,
    web::{Data, Path, Query},
    HttpRequest, HttpResponse, Result,
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
use tracing::{debug, debug_span, error, info};

#[derive(Deserialize)]
#[serde(untagged)]
enum TriggerQueryParams {
    Manual(ManualQueryParams),
    Autoscan(AutoscanQueryParams),
}

#[post("/triggers/{trigger}")]
pub async fn trigger_post(
    req: HttpRequest,
    trigger: Path<String>,
    manager: Data<PulseManager>,
    auth: Option<BasicAuth>,
    body: actix_web::web::Bytes,
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

    // Support legacy autoscan behavior: POST with query params and no JSON body.
    // The original autoscan (github.com/cloudbox/autoscan) sends scan requests as
    // POST /triggers/manual?dir=/path with an empty body when forwarding between
    // autoscan instances. Handle this by falling through to the GET-style query
    // param handler when the body is absent or empty.
    let parsed_body: Option<serde_json::Value> = if body.is_empty() {
        None
    } else {
        serde_json::from_slice(&body).ok()
    };

    let has_body = parsed_body
        .as_ref()
        .map(|b| !b.is_null())
        .unwrap_or(false);

    if !has_body {
        if let Ok(query) =
            Query::<TriggerQueryParams>::from_query(req.query_string())
        {
            return trigger_get_inner(&trigger, query.into_inner(), &manager, trigger_settings)
                .await;
        }
    }

    match trigger_settings {
        Trigger::Manual(_) | Trigger::Notify(_) => {
            Ok(HttpResponse::BadRequest().body("Invalid request"))
        }
        _ => {
            let rewrite = trigger_settings.get_rewrite();
            let body_value = parsed_body.unwrap_or(serde_json::Value::Null);
            let decoded = trigger_settings.paths(body_value);

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
                return Ok(HttpResponse::InternalServerError().body("falsed to add all events"));
            }

            Ok(HttpResponse::Ok().json(scan_events))
        }
    }
}

#[get("/triggers/{trigger}")]
pub async fn trigger_get(
    query: Query<TriggerQueryParams>,
    trigger: Path<String>,
    manager: Data<PulseManager>,
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

    trigger_get_inner(&trigger, query.into_inner(), &manager, trigger_settings).await
}

/// Shared handler for query-param-based trigger requests (used by both GET and
/// POST-with-query-params code paths).
async fn trigger_get_inner(
    trigger: &str,
    query: TriggerQueryParams,
    manager: &PulseManager,
    trigger_settings: &Trigger,
) -> Result<HttpResponse> {
    match &trigger_settings {
        Trigger::Manual(trigger_settings) | Trigger::Bazarr(trigger_settings) => {
            match query {
                TriggerQueryParams::Manual(query) => {
                    let mut file_path = query.path.clone();

                    if let Some(rewrite) = &trigger_settings.rewrite {
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
                        debug!("added file '{}'", file_path);
                    });

                    let scan_event = scan_event.unwrap();

                    Ok(HttpResponse::Ok().json(scan_event))
                }
                _ => Ok(HttpResponse::BadRequest().body("Invalid query parameters")),
            }
        }
        Trigger::Autoscan(trigger_settings) => match query {
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
                    debug!("added directory '{}'", dir_path);
                });

                let scan_event = scan_event.unwrap();

                Ok(HttpResponse::Ok().json(scan_event))
            }
            _ => Ok(HttpResponse::BadRequest().body("Invalid query parameters")),
        },
        _ => Ok(HttpResponse::Ok().body("Not implemented")),
    }
}
