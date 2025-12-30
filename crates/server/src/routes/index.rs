use actix_web::{get, web::Json, Responder};
use serde::Serialize;

#[doc(hidden)]
#[derive(Serialize)]
struct Hello {
    autopulse: &'static str,
}

#[get("/")]
pub async fn hello() -> impl Responder {
    let cargo_version = env!("GIT_REVISION");

    Json(Hello {
        autopulse: cargo_version,
    })
}
