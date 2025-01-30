pub mod routes;
mod middleware {
    pub mod auth;
}

use std::sync::Arc;

use actix_web::{dev::Server, middleware::Logger, web::Data, App, HttpServer};
use actix_web_httpauth::extractors::basic;
use autopulse_service::manager::PulseManager;
use routes::{
    index::hello, list::list, login::login, stats::stats, status::status, triggers::trigger_get,
    triggers::trigger_post,
};

pub fn get_server(
    hostname: String,
    port: u16,
    manager_clone: Arc<PulseManager>,
) -> anyhow::Result<Server> {
    Ok(HttpServer::new(move || {
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
            .app_data(Data::new(manager_clone.clone()))
    })
    .disable_signals()
    .bind((hostname, port))?
    .run())
}
