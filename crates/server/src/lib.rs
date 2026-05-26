use actix_session::{config::PersistentSession, storage::CookieSessionStore, SessionMiddleware};
use actix_web::{
    cookie::{time::Duration, Key, SameSite},
    dev::Server,
    middleware::Logger,
    web::Data,
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
    // UI session-signing key persisted in app_state. Survives restarts;
    // rotate via `DELETE FROM app_state WHERE key = 'ui_session_key_v1'`.
    let session_key: Key = ui::session_key::load_or_create(&manager.pool)?;
    let secure_cookies = manager.settings.app.secure_cookies;

    // Shared across workers so the per-IP login throttle is global, not
    // per-worker.
    let login_limiter = Data::new(ui::auth::LoginLimiter::default());

    Ok(HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .wrap(
                SessionMiddleware::builder(CookieSessionStore::default(), session_key.clone())
                    .cookie_name("autopulse_sid".to_string())
                    // Set `app.secure_cookies = true` when serving over TLS.
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
            .configure(ui::configure)
            .app_data(basic::Config::default().realm("Restricted area"))
            .app_data(login_limiter.clone())
            .app_data(Data::new(manager.clone()))
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
