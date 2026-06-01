use actix_session::{config::PersistentSession, storage::CookieSessionStore, SessionMiddleware};
use actix_web::{
    cookie::{time::Duration, Key, SameSite},
    dev::Server,
    middleware::Logger,
    web::{self, Data},
    App, HttpServer,
};
use actix_web_httpauth::extractors::basic;
use autopulse_service::manager::PulseManager;
use routes::{
    config::config_template, index::hello, list::list, login::login, stats::stats, status::status,
    triggers::trigger_get, triggers::trigger_post,
};

pub mod routes;
pub mod ui;

mod middleware {
    pub mod auth;
}

pub fn get_server(hostname: &str, port: &u16, manager: PulseManager) -> anyhow::Result<Server> {
    let session_key: Key = ui::session_key::load_or_create(&manager.pool)?;
    let secure_cookies = manager.settings.app.secure_cookies;
    let base_path = manager.settings.app.base_path.clone();

    let login_limiter = Data::new(ui::auth::LoginLimiter::default());

    Ok(HttpServer::new(move || {
        let app = App::new()
            .wrap(Logger::default())
            .wrap(
                SessionMiddleware::builder(CookieSessionStore::default(), session_key.clone())
                    .cookie_name("autopulse_sid".to_string())
                    .cookie_secure(secure_cookies)
                    .cookie_same_site(SameSite::Strict)
                    .cookie_http_only(true)
                    .session_lifecycle(PersistentSession::default().session_ttl(Duration::days(7)))
                    .build(),
            )
            .service(hello)
            .service(trigger_get)
            .service(trigger_post)
            .service(status)
            .service(stats)
            .service(login)
            .service(list)
            .service(config_template)
            .app_data(basic::Config::default().realm("Restricted area"))
            .app_data(login_limiter.clone())
            .app_data(Data::new(manager.clone()));

        // Mount UI under base_path so a pass-through reverse proxy (no
        // strip-prefix) works without rewrites.
        if base_path.is_empty() {
            app.configure(ui::configure)
        } else {
            app.service(web::scope(&base_path).configure(ui::configure))
        }
    })
    .bind((hostname, *port))?
    .run())
}

#[cfg(test)]
mod tests {
    mod middleware {
        mod check_auth;
    }

    #[cfg(feature = "sqlite")]
    mod routes {
        mod public_endpoints;
    }
}
