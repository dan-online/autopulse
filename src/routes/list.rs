use crate::{service::manager::PulseManager, utils::check_auth::check_auth};
use actix_web::web::{self, Data};
use actix_web::{get, HttpResponse};
use actix_web::{Responder, Result};
use actix_web_httpauth::extractors::basic::BasicAuth;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
#[serde(default)]
struct ListQuery {
    limit: u8,
    page: u64,
    sort: Option<String>,
    status: Option<String>,
    search: Option<String>,
}

impl Default for ListQuery {
    fn default() -> Self {
        Self {
            limit: 10,
            page: 1,
            sort: None,
            status: None,
            search: None,
        }
    }
}

#[get("/list")]
pub async fn list(
    manager: Data<Arc<PulseManager>>,
    auth: BasicAuth,
    query: web::Query<ListQuery>,
) -> Result<impl Responder> {
    if !check_auth(&auth, &manager.settings) {
        return Ok(HttpResponse::Unauthorized().body("Unauthorized"));
    }

    let scan_evs = manager.get_events(
        query.limit,
        query.page,
        query.sort.clone(),
        query.status.clone(),
        query.search.clone(),
    );

    if let Err(e) = scan_evs {
        return Ok(HttpResponse::InternalServerError().body(e.to_string()));
    }

    Ok(HttpResponse::Ok().json(scan_evs.unwrap()))
}
