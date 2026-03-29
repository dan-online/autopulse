use crate::middleware::auth::AuthenticatedUser;
use actix_web::web::{self, Data};
use actix_web::{get, HttpResponse};
use actix_web::{Responder, Result};
use autopulse_service::manager::PulseManager;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(default)]
pub struct ListQuery {
    /// The number of items to retrieve per page. (default: 10)
    limit: u8,
    /// The page number to retrieve. (default: 1)
    page: u64,
    /// The field to sort the results by. Can be one of `id`, `file_path`, `process_status`, `event_source`, `created_at`, or `updated_at`.
    sort: Option<String>,
    /// Filter the scan events by process status. Can be one of `pending`, `complete`, `retry`, or `failed`.
    status: Option<String>,
    /// Filter the scan events by a search query.
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
    manager: Data<PulseManager>,
    _auth: AuthenticatedUser,
    query: web::Query<ListQuery>,
) -> Result<impl Responder> {
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
