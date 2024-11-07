use crate::{
    db::models::{FoundStatus, NewScanEvent},
    service::{manager::PulseManager, triggers::manual::ManualQueryParams, webhooks::EventType},
    settings::trigger::Trigger,
    utils::{check_auth::check_auth, sify::sify},
};
use actix_web::{
    get, post,
    web::{Data, Json, Path, Query},
    HttpRequest, HttpResponse, Result,
};
use actix_web_httpauth::extractors::basic::BasicAuth;
use std::sync::Arc;
use tracing::debug;

#[post("/triggers/{trigger}")]
pub async fn trigger_post(
    trigger: Path<String>,
    manager: Data<Arc<PulseManager>>,
    auth: Option<BasicAuth>,
    body: Json<serde_json::Value>,
) -> Result<HttpResponse> {
    if !check_auth(&auth, &manager.settings) {
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
            let timer = trigger_settings.get_timer();
            let paths = trigger_settings.paths(body.into_inner());

            if paths.is_err() {
                return Ok(HttpResponse::BadRequest().body("Invalid request"));
            }

            let paths = paths.unwrap();

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

            debug!(
                "added {} file{} from {} trigger",
                scan_events.len(),
                sify(&scan_events),
                trigger
            );

            if scan_events.len() != paths.len() {
                return Ok(HttpResponse::InternalServerError().body("Failed to add all events"));
            }

            Ok(HttpResponse::Ok().json(scan_events))
        }
    }
}

#[get("/triggers/{trigger}")]
pub async fn trigger_get(
    req: HttpRequest,
    trigger: Path<String>,
    manager: Data<Arc<PulseManager>>,
    auth: Option<BasicAuth>,
) -> Result<HttpResponse> {
    if !check_auth(&auth, &manager.settings) {
        return Ok(HttpResponse::Unauthorized().body("Unauthorized"));
    }

    let trigger_settings = manager.settings.triggers.get(&trigger.to_string());

    if trigger_settings.is_none() {
        return Ok(HttpResponse::NotFound().body("Trigger not found"));
    }

    let trigger_settings = trigger_settings.unwrap();

    match &trigger_settings {
        Trigger::Manual(trigger_settings) => {
            let query = Query::<ManualQueryParams>::from_query(req.query_string())?;

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
                            .wait
                            .unwrap_or(manager.settings.opts.default_timer_wait)
                            as i64,
                    ),
                ..Default::default()
            };

            println!("{:?}", new_scan_event.can_process);

            let scan_event = manager.add_event(&new_scan_event);

            if let Err(e) = scan_event {
                return Ok(HttpResponse::InternalServerError().body(e.to_string()));
            }

            manager
                .webhooks
                .add_event(EventType::New, Some(trigger.to_string()), &[file_path])
                .await;

            debug!("added 1 file from {} trigger", trigger);

            let scan_event = scan_event.unwrap();

            Ok(HttpResponse::Ok().json(scan_event))
        }
        _ => Ok(HttpResponse::Ok().body("Not implemented")),
    }
}
