//! automated scanning tool that integrates widely-used media management services with various media servers for seamless media organization
//!
//! ## Quick docs
//!
//! - **[Triggers](service::triggers)**: Create triggers that will be executed by a service when a certain event occurs
//! - **[Targets](service::targets)**: Create targets that will be scanned by a service
//! - **[Webhooks](service::webhooks)**: Send webhooks to services to notify them of an event
//! - **[Settings](utils::settings)**: Settings handler
//! - **[Database](db::conn::AnyConnection)**: Database handler
//!
//! ## About
#![doc = include_str!("../README.md")]

use actix_web::{middleware::Logger, web::Data, App, HttpServer};
use actix_web_httpauth::extractors::basic;
use anyhow::Context;
use clap::Parser;
use db::conn::{get_conn, get_pool};
use db::migration::run_db_migrations;
use routes::stats::stats;
use routes::status::status;
use routes::triggers::trigger_post;
use routes::{index::hello, triggers::trigger_get};
use service::manager::PulseManager;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;
use utils::cli::Args;
use utils::settings::Settings;

/// Web server routes
pub mod routes;

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
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let settings = Settings::get_settings(args.config).with_context(|| "Failed to get settings")?;

    let filter = format!(
        "autopulse={},actix_web=info,actix_server=info",
        settings.app.log_level
    );

    tracing_subscriber::fmt().with_env_filter(filter).init();

    info!("💫 autopulse starting up...");

    let hostname = settings.app.hostname.clone();
    let port = settings.app.port;
    let database_url = settings.app.database_url.clone();

    // TODO: Move to pre-init
    if database_url.starts_with("sqlite://") {
        let path = database_url.split("sqlite://").collect::<Vec<&str>>()[1];
        let path = PathBuf::from(path);
        let parent = path.parent().unwrap();

        if !std::path::Path::new(&path).exists() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create database directory: {}", parent.display())
            })?;
        }

        std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o777)).with_context(
            || {
                format!(
                    "Failed to set permissions on database directory: {}",
                    parent.display()
                )
            },
        )?;
    }

    let pool = get_pool(database_url)?;
    let conn = &mut get_conn(&pool);

    run_db_migrations(conn);
    conn.init()?;

    let manager = PulseManager::new(settings, pool.clone());
    let manager = Arc::new(manager);

    let manager_task = manager.start();
    let webhook_task = manager.start_webhooks();
    let notify_task = manager.start_notify();

    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .service(hello)
            .service(trigger_get)
            .service(trigger_post)
            .service(status)
            .service(stats)
            .app_data(basic::Config::default().realm("Restricted area"))
            .app_data(Data::new(manager.clone()))
    })
    .bind((hostname, port))?
    .run()
    .await
    .with_context(|| "Failed to start server")?;

    info!("Shutting down...");

    manager_task.abort();
    webhook_task.abort();
    notify_task.abort();

    Ok(())
}
