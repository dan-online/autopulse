use anyhow::{Context, Ok};
use serde::Deserialize;
use std::path::PathBuf;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::RollingFileAppender;
pub use tracing_appender::rolling::Rotation;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter};

#[derive(Clone, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Trace => write!(f, "trace"),
            Self::Debug => write!(f, "debug"),
            Self::Info => write!(f, "info"),
            Self::Warn => write!(f, "warn"),
            Self::Error => write!(f, "error"),
        }
    }
}

impl std::str::FromStr for LogLevel {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "trace" => Ok(Self::Trace),
            "debug" => Ok(Self::Debug),
            "info" => Ok(Self::Info),
            "warn" => Ok(Self::Warn),
            "error" => Ok(Self::Error),
            _ => Err(anyhow::anyhow!("Invalid log level")),
        }
    }
}

pub fn setup_logs(
    log_level: &LogLevel,
    log_file: &Option<PathBuf>,
    log_file_rollover: Rotation,
    api_logging: bool,
) -> anyhow::Result<Option<WorkerGuard>> {
    let timer = tracing_subscriber::fmt::time::OffsetTime::local_rfc_3339()
        .context("Failed to initialize the timer")?;

    let mut file_guard = None;

    let mut filter =
        EnvFilter::from_default_env().add_directive(format!("autopulse={log_level}").parse()?);

    if api_logging {
        filter = filter
            .add_directive("actix_web=info".parse()?)
            .add_directive("actix_server::builder=info".parse()?);
    }

    let registry = tracing_subscriber::registry().with(filter);

    if let Some(log_file) = log_file {
        let writer = RollingFileAppender::new(
            log_file_rollover,
            log_file.parent().ok_or_else(|| {
                anyhow::anyhow!("Failed to get parent directory of log file")
            })?,
            log_file.file_name().ok_or_else(|| {
                anyhow::anyhow!("Failed to get file name of log file")
            })?,
        );

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
