use actix_web::{middleware::Logger, web::Data, App, HttpServer};
use actix_web_httpauth::extractors::basic;
use anyhow::Context;
use db::migration::run_db_migrations;
use routes::stats::stats;
use routes::status::status;
use routes::triggers::trigger_post;
use routes::{index::hello, triggers::trigger_get};
use service::manager::PulseManager;
use tracing::info;
use utils::conn::{get_conn, get_pool};
use utils::settings::Settings;

pub mod routes {
    pub mod index;
    pub mod stats;
    pub mod status;
    pub mod triggers;
}
pub mod utils;
pub mod db {
    pub mod migration;
    pub mod models;
    pub mod schema;
}
pub mod service;

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

    let service_task = manager.start();

    // Not a fan but the performance hit is negligible
    let service_clone = manager.clone();

    let notify_task = tokio::spawn(async move {
        service_clone.start_notify().await;
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

    service_task.abort();
    notify_task.abort();

    Ok(())
}
