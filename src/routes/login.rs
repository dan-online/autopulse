use crate::{service::manager::PulseManager, utils::check_auth::check_auth};
use actix_web::web::Data;
use actix_web::{post, HttpResponse};
use actix_web::{Responder, Result};
use actix_web_httpauth::extractors::basic::BasicAuth;
use serde_json::json;
use std::sync::Arc;

#[post("/login")]
pub async fn login(manager: Data<Arc<PulseManager>>, auth: BasicAuth) -> Result<impl Responder> {
    if !check_auth(&auth, &manager.settings) {
        return Ok(HttpResponse::Unauthorized().body("Unauthorized"));
    }

    Ok(HttpResponse::Ok().json(json!({"status": "ok"})))
}
