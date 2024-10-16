use anyhow::{Context, Ok};
use std::{io, path::PathBuf};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter};

pub fn setup_logs(
    log_level: String,
    log_file: Option<PathBuf>,
) -> anyhow::Result<Option<WorkerGuard>> {
    let collector = tracing_subscriber::registry()
        .with(
            EnvFilter::default()
                .add_directive(format!("autopulse={log_level}").parse()?)
                .add_directive("actix_web=info".parse()?),
        )
        .with(fmt::Layer::new().with_writer(io::stdout));

    let mut file_guard = None;

    if let Some(log_file) = log_file {
        let writer = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_file.clone())
            .with_context(|| format!("Failed to open log file: {}", log_file.to_str().unwrap()))?;

        let (non_blocking, guard) = tracing_appender::non_blocking(writer);

        file_guard = Some(guard);

        let collector =
            collector.with(fmt::Layer::new().with_ansi(false).with_writer(non_blocking));

        tracing::subscriber::set_global_default(collector)
            .expect("Unable to set a global subscriber");
    } else {
        tracing::subscriber::set_global_default(collector)
            .expect("Unable to set a global subscriber");
    };

    Ok(file_guard)
}
