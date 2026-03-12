use crate::middleware::auth::AuthenticatedUser;
use actix_web::{post, HttpResponse};
use actix_web::{Responder, Result};
use serde_json::json;

#[post("/login")]
pub async fn login(_auth: AuthenticatedUser) -> Result<impl Responder> {
    Ok(HttpResponse::Ok().json(json!({"status": "ok"})))
}
