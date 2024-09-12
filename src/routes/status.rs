use crate::{service::manager::PulseManager, utils::check_auth::check_auth};
use actix_web::{
    get,
    web::{Data, Path},
    HttpResponse, Responder, Result,
};
use actix_web_httpauth::extractors::basic::BasicAuth;

#[doc(hidden)]
#[get("/status/{id}")]
pub async fn status(
    id: Path<String>,
    manager: Data<PulseManager>,
    auth: BasicAuth,
) -> Result<impl Responder> {
    if !check_auth(&auth, &manager.settings) {
        return Ok(HttpResponse::Unauthorized().body("Unauthorized"));
    }

    let scan_ev = manager.get_event(&id);

    Ok(HttpResponse::Ok().json(scan_ev))
}
