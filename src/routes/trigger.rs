use actix_web::{
    get,
    web::{Data, Path, Query},
    HttpRequest, HttpResponse, Responder, Result,
};
use actix_web_httpauth::extractors::basic::BasicAuth;

use crate::{
    db::models::NewScanEvent,
    service::PulseService,
    triggers::manual::ManualQueryParams,
    utils::{
        check_auth::check_auth,
        settings::{Settings, TriggerTypes},
    },
};

#[get("/triggers/{trigger}")]
pub async fn trigger(
    req: HttpRequest,
    trigger: Path<String>,
    settings: Data<Settings>,
    service: Data<PulseService>,
    auth: BasicAuth,
    // query: Json<serde_json::Value>,
) -> Result<impl Responder> {
    if !check_auth(&auth, &settings) {
        return Ok(HttpResponse::Unauthorized().body("Unauthorized"));
    }

    let trigger_settings = settings.triggers.get(&trigger.to_string());

    if trigger_settings.is_none() {
        return Ok(HttpResponse::NotFound().body("Trigger not found"));
    }

    let trigger_settings = trigger_settings.unwrap();

    match &trigger_settings.t {
        TriggerTypes::Manual => {
            let query = Query::<ManualQueryParams>::from_query(req.query_string())?;

            let mut file_path = query.path.clone();

            if let Some(rewrite) = &trigger_settings.rewrite {
                let from = rewrite.from.clone();
                let to = rewrite.to.clone();

                file_path = file_path.replace(&from, &to);
            }

            let new_scan_event = NewScanEvent {
                event_source: trigger.to_string(),
                file_path,
                file_hash: query.hash.clone(),
            };

            let scan_event = service.add_event(new_scan_event);

            Ok(HttpResponse::Ok().json(scan_event))
        }
        _ => todo!(),
    }
}
