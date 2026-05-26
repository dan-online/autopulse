use actix_web::{get, web::Data, Responder};
use actix_web_lab::sse::{self, Sse};
use autopulse_service::manager::PulseManager;
use std::{convert::Infallible, time::Duration};
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_stream::{wrappers::BroadcastStream, StreamExt as _};

use crate::ui::{auth::SessionUser, events_view};

/// On channel lag (slow consumer during burst), emits `event: resync`
/// so HTMX re-fetches the rows partial instead of losing events.
///
/// 15s keep-alive prevents Cloudflare/nginx ~100s idle disconnect.
#[get("/ui/events/stream")]
pub async fn events_stream(manager: Data<PulseManager>, _user: SessionUser) -> impl Responder {
    let rx = manager.subscribe();
    let base = manager.settings.app.base_path.clone();

    let stream = BroadcastStream::new(rx).map(move |msg| {
        let frame = match msg {
            Ok(b) => {
                let id = &b.event.id;
                let row = events_view::event_row(&base, &b.event).into_string();
                // Remove existing row before HTMX prepends the updated one,
                // preventing duplicates when an event updates rather than being new.
                let html = format!(
                    r#"<script>document.getElementById('evt-{id}')?.remove()</script>{row}"#
                );
                sse::Data::new(html).event("event-row")
            }
            Err(BroadcastStreamRecvError::Lagged(n)) => {
                sse::Data::new(format!("<!-- lagged {n} events; resync -->")).event("resync")
            }
        };
        Ok::<_, Infallible>(sse::Event::Data(frame))
    });

    Sse::from_stream(stream).with_keep_alive(Duration::from_secs(15))
}
