use crate::middleware::auth::check_auth;
use actix_web::{
    get,
    web::{Data, Path},
    HttpResponse, Responder, Result,
};
use actix_web_httpauth::extractors::basic::BasicAuth;
use autopulse_service::manager::PulseManager;

#[doc(hidden)]
#[get("/status/{id}")]
pub async fn status(
    id: Path<String>,
    manager: Data<PulseManager>,
    auth: Option<BasicAuth>,
) -> Result<impl Responder> {
    if !check_auth(
        &auth,
        &manager.settings.auth.enabled,
        &manager.settings.auth.username,
        &manager.settings.auth.password,
    ) {
        return Ok(HttpResponse::Unauthorized().body("Unauthorized"));
    }

    let scan_ev = manager.get_event(&id);

    if let Err(e) = scan_ev {
        return Ok(HttpResponse::InternalServerError().body(e.to_string()));
    }

    Ok(HttpResponse::Ok().json(scan_ev.unwrap()))
}
