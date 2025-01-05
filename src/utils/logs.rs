use crate::settings::app::LogLevel;
use anyhow::{Context, Ok};
use std::path::PathBuf;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter};

pub fn setup_logs(
    log_level: &LogLevel,
    log_file: &Option<PathBuf>,
) -> anyhow::Result<Option<WorkerGuard>> {
    let timer = tracing_subscriber::fmt::time::OffsetTime::local_rfc_3339()
        .context("Failed to initialize the timer")?;

    let mut file_guard = None;

    let filter = EnvFilter::from_default_env()
        .add_directive(format!("autopulse={log_level}").parse()?)
        .add_directive("actix_web=info".parse()?)
        .add_directive("actix_server::builder=info".parse()?);

    let registry = tracing_subscriber::registry().with(filter);

    if let Some(log_file) = log_file {
        let writer = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)
            .with_context(|| format!("Failed to open log file: {}", log_file.to_string_lossy()))?;

        let (non_blocking, guard) = tracing_appender::non_blocking(writer);
        file_guard = Some(guard);

        let file_layer = fmt::layer()
            .with_writer(non_blocking)
            .with_ansi(false)
            .with_timer(timer.clone());

        let registry = registry.with(file_layer);

        let console_layer = fmt::layer()
            .with_writer(std::io::stdout)
            .with_ansi(true)
            .with_timer(timer);

        registry.with(console_layer).init();
    } else {
        let console_layer = fmt::layer()
            .with_writer(std::io::stdout)
            .with_ansi(true)
            .with_timer(timer);

        registry.with(console_layer).init();
    }
    Ok(file_guard)
}
