use actix_web::{get, web::Json, Responder};
use serde::Serialize;

#[derive(Serialize)]
struct Hello {
    autopulse: String,
}

#[get("/")]
pub async fn hello() -> impl Responder {
    let cargo_version = format!("v{}", env!("CARGO_PKG_VERSION"));

    Json(Hello {
        autopulse: cargo_version,
    })
}
