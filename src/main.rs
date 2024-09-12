use actix_web::{middleware::Logger, web::Data, App, HttpServer};
use actix_web_httpauth::extractors::basic;
use anyhow::Context;
use db::conn::{get_conn, get_pool};
use db::migration::run_db_migrations;
use routes::stats::stats;
use routes::status::status;
use routes::triggers::trigger_post;
use routes::{index::hello, triggers::trigger_get};
use service::manager::PulseManager;
use tracing::info;
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
    let settings = Settings::get_settings().with_context(|| "Failed to get settings")?;

    let filter = format!(
        "autopulse={},actix_web=info,actix_server=info",
        settings.app.log_level
    );

    tracing_subscriber::fmt().with_env_filter(filter).init();

    let hostname = settings.app.hostname.clone();
    let port = settings.app.port;
    let database_url = settings.app.database_url.clone();

    info!("💫 autopulse starting up...");

    let pool = get_pool(database_url)?;
    let conn = &mut get_conn(&pool);

    run_db_migrations(conn);
    conn.init()?;

    let manager = PulseManager::new(settings, pool.clone());

    let manager_task = manager.start();

    // Not a fan but the performance hit is negligible
    let manager_clone = manager.clone();

    let notify_task = tokio::spawn(async move {
        manager_clone.start_notify().await;
    });

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
    notify_task.abort();

    Ok(())
}
