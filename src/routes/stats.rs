use crate::service::manager::PulseManager;
use actix_web::{get, web::Data, HttpResponse, Responder, Result};
use serde::Serialize;
use std::{sync::Arc, time::Instant};
use tracing::error;

/// Represents the service statistics.
#[derive(Clone, Serialize)]
pub struct Stats {
    /// The total number of events.
    pub total: i64,
    /// The number of file events that have been processed.
    pub processed: i64,
    /// The number of file events that are being retried.
    pub retrying: i64,
    /// The number of file events that have failed.
    pub failed: i64,
    /// The number of file events that are pending.
    pub pending: i64,
}

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
pub async fn stats(manager: Data<Arc<PulseManager>>) -> Result<impl Responder> {
    let start = Instant::now();
    let stats = manager.get_stats();
    let elapsed = start.elapsed().as_micros() as f64 / 1000.0;

    if let Err(e) = stats {
        error!("Failed to get stats: {:?}", e);
        return Ok(HttpResponse::InternalServerError().finish());
    }

    let stats = stats.unwrap();

    let response = StatsResponse {
        stats,
        speed: elapsed,
    };

    Ok(HttpResponse::Ok().json(response))
}
