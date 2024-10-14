use anyhow::{Context, Ok};
use std::path::PathBuf;
use tracing_appender::non_blocking::WorkerGuard;

pub fn setup_logs(
    log_level: String,
    log_file: Option<PathBuf>,
) -> anyhow::Result<Option<WorkerGuard>> {
    let filter = format!("autopulse={},actix_web=info", log_level);

    let log_tracer = tracing_subscriber::fmt().with_env_filter(filter);

    if let Some(log_file) = log_file {
        let writer = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_file.clone())
            .with_context(|| format!("Failed to open log file: {}", log_file.to_str().unwrap()))?;

        let (non_blocking, guard) = tracing_appender::non_blocking(writer);

        log_tracer.with_writer(non_blocking).init();

        return Ok(Some(guard));
    }

    log_tracer.init();

    Ok(None)
}
