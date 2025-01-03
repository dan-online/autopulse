//! automated scanning tool that integrates widely-used media management services with various media servers for seamless media organization
//!
//! ## Quick docs
//!
//! - **[Triggers](service::triggers)**: Create triggers that will be executed by a service when a certain event occurs
//! - **[Targets](service::targets)**: Create targets that will be scanned by a service
//! - **[Webhooks](service::webhooks)**: Send webhooks to services to notify them of an event
//! - **[Settings](settings)**: Settings handler
//! - **[Database](db::conn::AnyConnection)**: Database handler
//!
//! ## About
#![doc = include_str!("../README.md")]

use actix_web::{middleware::Logger, web::Data, App, HttpServer};
use actix_web_httpauth::extractors::basic;
use anyhow::Context;
use clap::Parser;
use db::conn::{get_conn, get_pool, AnyConnection};
use routes::list::list;
use routes::login::login;
use routes::stats::stats;
use routes::status::status;
use routes::triggers::trigger_post;
use routes::{index::hello, triggers::trigger_get};
use service::manager::PulseManager;
use settings::Settings;
use std::sync::Arc;
use tokio::signal::unix::{signal, SignalKind};
use tracing::{debug, error, info};
use tracing_appender::non_blocking::WorkerGuard;
use utils::cli::Args;
use utils::logs::setup_logs;

#[doc(hidden)]
mod tests;

/// Web server routes
pub mod routes;

/// Settings configuration
///
/// Used to configure the service.
///
/// Can be defined in 2 ways:
/// - Config file
///   - `config.{json,toml,yaml,json5,ron,ini}` in the current directory
/// - Environment variables
///   - `AUTOPULSE__{SECTION}__{KEY}` (e.g. `AUTOPULSE__APP__DATABASE_URL`)
///
/// See [Settings] for all options
pub mod settings;

/// Database handler
pub mod db;

/// Core of autopulse
///
/// Includes:
/// - `Triggers`
/// - `Webhooks`
/// - `Targets`
pub mod service;

/// Internal utility functions
pub mod utils;

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

    let manager_task = manager.start();
    let webhook_task = manager.start_webhooks();
    let notify_task = manager.start_notify()?;

    info!("🚀 Listening on {}:{}", hostname, port);

    let server = HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .service(hello)
            .service(trigger_get)
            .service(trigger_post)
            .service(status)
            .service(stats)
            .service(login)
            .service(list)
            .app_data(basic::Config::default().realm("Restricted area"))
            .app_data(Data::new(manager.clone()))
    })
    .bind((hostname, port))?
    .run();

    let mut sigterm = signal(SignalKind::terminate()).unwrap();
    let mut sigint = signal(SignalKind::interrupt()).unwrap();

    tokio::select! {
        _ = sigterm.recv() => {
            debug!("Received SIGTERM, shutting down...");
        }

        _ = sigint.recv() => {
            debug!("Received SIGINT, shutting down...");
        }

        res = server => {
            debug!("server stopped");
            res?;
        }
        res = manager_task => {
            debug!("manager stopped");
            res?;
        }
        res = webhook_task => {
            debug!("webhook stopped");
            res?;
        }
        res = notify_task => {
            debug!("notify stopped");
            if let Err(e) = res? {
                error!("notify error: {:?}", e);
            }
        }
    }

    info!("Shutting down...");

    Ok(())
}

#[doc(hidden)]
pub fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let settings = Settings::get_settings(args.config).with_context(|| "Failed to get settings")?;

    let guard = setup_logs(
        settings.app.log_level.clone(),
        settings.opts.log_file.clone(),
    )?;

    info!("💫 autopulse starting up...");

    run(settings, guard)
}
