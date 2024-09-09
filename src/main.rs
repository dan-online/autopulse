use actix_web::{middleware::Logger, web::Data, App, HttpServer};
use actix_web_httpauth::extractors::basic;
use anyhow::Context;
use db::migration::run_db_migrations;
use routes::stats::stats;
use routes::status::status;
use routes::triggers::trigger_post;
use routes::{index::hello, triggers::trigger_get};
use service::PulseService;
use tracing::info;
use tracing::Level;
use utils::conn::get_pool;
use utils::settings::Settings;

pub mod routes {
    pub mod index;
    pub mod stats;
    pub mod status;
    pub mod triggers;
}
pub mod utils {
    pub mod check_auth;
    pub mod checksum;
    pub mod conn;
    pub mod join_path;
    pub mod settings;
}
pub mod db {
    pub mod migration;
    pub mod models;
    pub mod schema;
}
pub mod service;

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    let settings = Settings::get_settings().with_context(|| "Failed to get settings")?;

    tracing_subscriber::fmt()
        .with_max_level(match settings.app.log_level {
            ref level if level == "debug" => Level::DEBUG,
            ref level if level == "info" => Level::INFO,
            ref level if level == "warn" => Level::WARN,
            ref level if level == "error" => Level::ERROR,
            _ => Level::INFO,
        })
        .init();

    let hostname = settings.app.hostname.clone();
    let port = settings.app.port;
    let database_url = settings.app.database_url.clone();

    info!("💫 autopulse starting up...");

    let pool = get_pool(database_url)?;

    run_db_migrations(&mut pool.get().expect("Failed to get connection"));

    let service = PulseService::new(settings.clone(), pool.clone());

    service.start();

    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .service(hello)
            .service(trigger_get)
            .service(trigger_post)
            .service(status)
            .service(stats)
            .app_data(basic::Config::default().realm("Restricted area"))
            .app_data(Data::new(settings.clone()))
            .app_data(Data::new(pool.clone()))
            .app_data(Data::new(service.clone()))
    })
    .bind((hostname, port))?
    .run()
    .await
    .with_context(|| "Failed to start server")?;

    Ok(())
}
