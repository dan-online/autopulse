//! Maud + HTMX server-rendered UI mounted at `/ui/*`.

pub mod add;
pub mod auth;
pub mod config;
pub mod csrf;
pub mod detail;
pub mod events;
pub mod events_view;
pub mod layout;
pub mod session_key;
pub mod static_assets;
pub mod stream;

use actix_web::{
    get,
    web::{Data, ServiceConfig},
    HttpResponse, Responder,
};
use autopulse_service::manager::PulseManager;

/// Bare `/ui` (and `/ui/`) → the events view. No auth gate here; the
/// target route redirects to `/ui/login` itself if needed.
#[get("/ui")]
async fn ui_root(manager: Data<PulseManager>) -> impl Responder {
    redirect_to_events(&manager)
}

#[get("/ui/")]
async fn ui_root_slash(manager: Data<PulseManager>) -> impl Responder {
    redirect_to_events(&manager)
}

fn redirect_to_events(manager: &PulseManager) -> HttpResponse {
    let base = &manager.settings.app.base_path;
    HttpResponse::SeeOther()
        .insert_header(("Location", format!("{base}/ui/events")))
        .finish()
}

/// Route order matters: `/ui/events/stream` and `/ui/events/rows` must
/// register before `/ui/events/{id}` so the `{id}` catch-all doesn't
/// swallow them.
pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(ui_root)
        .service(ui_root_slash)
        .service(static_assets::serve_static)
        .service(auth::login_page)
        .service(auth::login_post)
        .service(auth::logout_post)
        .service(events::events_page)
        .service(events::events_rows)
        .service(events::events_stats)
        .service(stream::events_stream)
        .service(events::event_retry)
        .service(detail::event_detail)
        .service(add::add_preview)
        .service(add::add_page)
        .service(add::add_post)
        .service(config::config_page);
}
