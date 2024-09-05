use actix_web::{
    get, post,
    web::{Data, Json, Path, Query},
    HttpRequest, HttpResponse, Responder, Result,
};
use actix_web_httpauth::extractors::basic::BasicAuth;

use crate::{
    db::models::NewScanEvent,
    service::{triggers::manual::ManualQueryParams, webhooks::EventType, PulseService},
    utils::{
        check_auth::check_auth,
        settings::{Settings, Trigger},
    },
};

#[post("/triggers/{trigger}")]
pub async fn trigger_post(
    trigger: Path<String>,
    settings: Data<Settings>,
    service: Data<PulseService>,
    auth: BasicAuth,
    body: Json<serde_json::Value>,
) -> Result<impl Responder> {
    if !check_auth(&auth, &settings) {
        return Ok(HttpResponse::Unauthorized().body("Unauthorized"));
    }

    let trigger_settings = settings.triggers.get(&trigger.to_string());

    if trigger_settings.is_none() {
        return Ok(HttpResponse::NotFound().body("Trigger not found"));
    }

    let trigger_settings = trigger_settings.unwrap();

    match &trigger_settings {
        Trigger::Sonarr { rewrite } | Trigger::Radarr { rewrite } => {
            let paths = trigger_settings.paths(body.into_inner());

            if paths.is_err() {
                return Ok(HttpResponse::BadRequest().body("Invalid request"));
            }

            let paths = paths.unwrap();

            let mut scan_events = vec![];

            for path in &paths {
                let mut path = path.clone();

                if let Some(rewrite) = rewrite {
                    let from = rewrite.from.clone();
                    let to = rewrite.to.clone();

                    path = path.replace(&from, &to);
                }

                let new_scan_event = NewScanEvent {
                    event_source: trigger.to_string(),
                    file_path: path.clone(),
                    ..Default::default()
                };

                let scan_event = service.add_event(&new_scan_event);

                if let Ok(scan_event) = scan_event {
                    scan_events.push(scan_event);
                }
            }

            service
                .webhooks
                .send(EventType::New, Some(trigger.to_string()), &paths)
                .await;

            if scan_events.len() != paths.len() {
                return Ok(HttpResponse::InternalServerError().body("Failed to add all events"));
            }

            Ok(HttpResponse::Ok().json(scan_events))
        }
        Trigger::Manual { .. } => {
            Ok(HttpResponse::BadRequest().body("Manual triggers must use GET requests"))
        }
    }
}

#[get("/triggers/{trigger}")]
pub async fn trigger_get(
    req: HttpRequest,
    trigger: Path<String>,
    settings: Data<Settings>,
    service: Data<PulseService>,
    auth: BasicAuth,
) -> Result<impl Responder> {
    if !check_auth(&auth, &settings) {
        return Ok(HttpResponse::Unauthorized().body("Unauthorized"));
    }

    let trigger_settings = settings.triggers.get(&trigger.to_string());

    if trigger_settings.is_none() {
        return Ok(HttpResponse::NotFound().body("Trigger not found"));
    }

    let trigger_settings = trigger_settings.unwrap();

    match &trigger_settings {
        Trigger::Manual { rewrite } => {
            let query = Query::<ManualQueryParams>::from_query(req.query_string())?;

            let mut file_path = query.path.clone();

            if let Some(rewrite) = rewrite {
                let from = rewrite.from.clone();
                let to = rewrite.to.clone();

                file_path = file_path.replace(&from, &to);
            }

            let new_scan_event = NewScanEvent {
                event_source: trigger.to_string(),
                file_path: file_path.clone(),
                file_hash: query.hash.clone(),
            };

            let scan_event = service.add_event(&new_scan_event);

            if let Err(e) = scan_event {
                return Ok(HttpResponse::InternalServerError().body(e.to_string()));
            }

            let scan_event = scan_event.unwrap();

            service
                .webhooks
                .send(EventType::New, Some(trigger.to_string()), &[file_path])
                .await;

            Ok(HttpResponse::Ok().json(scan_event))
        }
        _ => Ok(HttpResponse::Ok().body("Not implemented")),
    }
}
