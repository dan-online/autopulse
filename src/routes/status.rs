use actix_web::{
    get,
    web::{Data, Path},
    HttpResponse, Responder, Result,
};
use actix_web_httpauth::extractors::basic::BasicAuth;

use crate::{
    service::PulseService,
    utils::{check_auth::check_auth, settings::Settings},
};

#[get("/status/{id}")]
pub async fn status(
    id: Path<String>,
    service: Data<PulseService>,
    settings: Data<Settings>,
    auth: BasicAuth,
) -> Result<impl Responder> {
    if !check_auth(&auth, &settings) {
        return Ok(HttpResponse::Unauthorized().body("Unauthorized"));
    }

    let id = id.parse::<i32>().unwrap();

    let scan_ev = service.get_event(&id);

    Ok(HttpResponse::Ok().json(scan_ev))
}
