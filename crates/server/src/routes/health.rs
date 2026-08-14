use actix_web::{get, web::Json, Responder};
use serde_json::json;

#[get("/health")]
pub async fn health() -> impl Responder {
    Json(json!({
        "ok": true
    }))
}
