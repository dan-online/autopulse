//! automated scanning tool that integrates widely-used media management services with various media servers for seamless media organization
//!
//! ## Quick docs
//!
//! - **[Settings](autopulse_service::settings)**: Settings handler
//!   - **[Triggers](autopulse_service::settings::triggers)**: Create triggers that will be executed by a service when a certain event occurs
//!   - **[Targets](autopulse_service::settings::targets)**: Create targets that will be scanned by a service
//!   - **[Webhooks](autopulse_service::settings::webhooks)**: Send webhooks to services to notify them of an event
//! - **[Database](autopulse_database::conn::AnyConnection)**: Database handler
//!
//! ## About
#![doc = include_str!("../README.md")]

use anyhow::Context;
use autopulse_database::conn::{get_conn, get_pool, AnyConnection};
use autopulse_server::get_server;
use autopulse_service::manager::PulseManager;
use autopulse_service::settings::Settings;
use autopulse_utils::tracing_appender::non_blocking::WorkerGuard;
use autopulse_utils::{setup_logs, Rotation};
use clap::Parser;
use std::sync::Arc;
use tracing::{debug, error, info};

/// Arguments for CLI
///
/// ```
/// $ autopulse --help
/// ```
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Location of configuration file
    #[arg(short, long)]
    pub config: Option<String>,
}

#[doc(hidden)]
#[tokio::main]
async fn run(settings: Settings, _guard: Option<WorkerGuard>) -> anyhow::Result<()> {
    let hostname = settings.app.hostname.clone();
    let port = settings.app.port;
    let database_url = settings.app.database_url.clone();

    AnyConnection::pre_init(&database_url)?;

    let pool = get_pool(&database_url)?;
    let mut conn = get_conn(&pool)?;

    conn.migrate()?;

    // drop conn to prevent deadlocks
    drop(conn);

    let manager = PulseManager::new(settings, pool.clone());
    let manager = Arc::new(manager);

    manager.start().await;
    manager.start_webhooks().await;
    manager.start_notify().await;

    let manager_clone = manager.clone();

    let server = get_server(hostname.clone(), port, manager_clone)?;

    info!("🚀 listening on {}:{}", hostname, port);

    let server_task = tokio::spawn(server);

    let shutdown: tokio::task::JoinHandle<anyhow::Result<()>> = tokio::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};

            let mut sigterm = signal(SignalKind::terminate())?;
            let mut sigint = signal(SignalKind::interrupt())?;

            tokio::select! {
                _ = sigterm.recv() => {
                    debug!("Received SIGTERM");
                }
                _ = sigint.recv() => {
                    debug!("Received SIGINT");
                }
            }
        }

        #[cfg(windows)]
        {
            use tokio::signal::ctrl_c;

            let ctrl_c = ctrl_c();

            tokio::select! {
                _ = ctrl_c => {
                    debug!("Received Ctrl+C");
                }
            }
        }

        info!("💤 shutting down...");

        Ok(())
    });

    shutdown.await??;

    manager.shutdown().await?;
    server_task.abort();

    Ok(())
}

#[doc(hidden)]
fn setup() -> anyhow::Result<(Settings, Option<WorkerGuard>)> {
    let args = Args::parse();

    let settings = Settings::get_settings(args.config).context("failed to load settings");

    match settings {
        Ok(settings) => {
            let guard = setup_logs(
                &settings.app.log_level,
                &settings.opts.log_file,
                settings.opts.log_file_rollover.clone().into(),
                settings.app.api_logging,
            )?;

            Ok((settings, guard))
        }
        Err(e) => {
            // still setup logs if settings failed to load
            setup_logs(
                &autopulse_utils::LogLevel::Info,
                &None,
                Rotation::NEVER,
                false,
            )?;

            Err(e)
        }
    }
}

#[doc(hidden)]
pub fn main() -> anyhow::Result<()> {
    match setup() {
        Ok((settings, guard)) => {
            info!("💫 autopulse v{} starting up...", env!("CARGO_PKG_VERSION"),);

            if let Err(e) = run(settings, guard) {
                error!("{:?}", e);
            }
        }
        Err(e) => {
            error!("{:?}", e);
        }
    }

    Ok(())
}
