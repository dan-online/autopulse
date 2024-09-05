use actix_web::{middleware::Logger, web::Data, App, HttpServer};
use actix_web_httpauth::extractors::basic;
use anyhow::Context;
use db::migration::run_db_migrations;
use diesel::r2d2;
use diesel::sql_query;
use diesel::Connection;
use diesel::PgConnection;
use diesel::RunQueryDsl;
use routes::stats::stats;
use routes::status::status;
use routes::triggers::trigger_post;
use routes::{index::hello, triggers::trigger_get};
use service::PulseService;
use tracing::info;
use tracing::warn;
use tracing::Level;
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

pub type DbPool = r2d2::Pool<r2d2::ConnectionManager<PgConnection>>;

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    let settings = Settings::get_settings().with_context(|| "Failed to get settings")?;

    let hostname = settings.app.hostname.clone();
    let port = settings.app.port;
    let database_url = settings.app.database_url.clone();

    info!("💫 autopulse starting up...");

    if let Err(err) = PgConnection::establish(&database_url) {
        if let diesel::ConnectionError::BadConnection(err_msg) = &err {
            if err_msg.contains("database \"autopulse\" does not exist") {
                warn!("database does not exist. creating database...");

                let uri = database_url
                    .split("/")
                    .take(3)
                    .collect::<Vec<&str>>()
                    .join("/")
                    + "/postgres";

                let mut conn =
                    PgConnection::establish(&uri).expect("Failed to connect to PostgreSQL");

                sql_query("CREATE DATABASE autopulse")
                    .execute(&mut conn)
                    .expect("Failed to create database");

                info!("database created successfully");
            } else {
                return Err(err).with_context(|| "Failed to establish connection to database");
            }
        }
    }

    let manager = r2d2::ConnectionManager::<PgConnection>::new(database_url);
    let pool = r2d2::Pool::builder()
        .build(manager)
        .with_context(|| "Failed to create connection pool")?;

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
