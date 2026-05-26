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
    match manager.get_event(&id) {
        Ok(Some(event)) => Ok(HttpResponse::Ok().json(event)),
        Ok(None) => Ok(HttpResponse::NotFound().body("Event not found")),
        Err(e) => Ok(HttpResponse::InternalServerError().body(e.to_string())),
    }
}
