use actix_web::{get, web::Data, HttpResponse, Responder, Result};
use autopulse_service::manager::PulseManager;
use serde::Serialize;
use std::time::Instant;
use tracing::error;

pub use autopulse_service::manager::Stats;

/// Represents the response format for the `/stats` endpoint.
///
/// This structure is used to serialize the response returned by the `/stats` endpoint,
/// providing both the service statistics and the response time.
#[derive(Serialize)]
pub struct StatsResponse {
    /// Detailed service statistics.
    stats: Stats,
    /// The time taken to retrieve the statistics, measured in milliseconds.
    speed: f64,
}

#[doc(hidden)]
#[get("/stats")]
pub async fn stats(manager: Data<PulseManager>) -> Result<impl Responder> {
    let start = Instant::now();

    match manager.get_stats() {
        Ok(stats) => {
            let elapsed = start.elapsed().as_micros() as f64 / 1000.0;
            Ok(HttpResponse::Ok().json(StatsResponse {
                stats,
                speed: elapsed,
            }))
        }
        Err(e) => {
            error!("failed to get stats: {:?}", e);
            Ok(HttpResponse::InternalServerError().finish())
        }
    }
}
