use crate::middleware::auth::AuthenticatedUser;
use actix_web::{
    get,
    web::{Data, Path},
    HttpResponse, Responder, Result,
};
use autopulse_service::manager::PulseManager;

#[doc(hidden)]
#[get("/status/{id}")]
pub async fn status(
    id: Path<String>,
    manager: Data<PulseManager>,
    _auth: AuthenticatedUser,
) -> Result<impl Responder> {
    let scan_ev = manager.get_event(&id);

    if let Err(e) = scan_ev {
        return Ok(HttpResponse::InternalServerError().body(e.to_string()));
    }

    Ok(HttpResponse::Ok().json(scan_ev.unwrap()))
}
