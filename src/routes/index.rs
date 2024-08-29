use actix_web::{get, web::Json, Responder};
use serde::Serialize;

#[derive(Serialize)]
struct Hello {
    hello: String,
}

#[get("/")]
pub async fn hello() -> impl Responder {
    Json(Hello {
        hello: "world".to_string(),
    })
}
