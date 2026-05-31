use actix_web::{get, web::Data, Responder};
use actix_web_lab::sse::{self, Sse};
use autopulse_service::manager::PulseManager;
use std::{convert::Infallible, time::Duration};
use tokio::sync::mpsc;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_stream::wrappers::{BroadcastStream, ReceiverStream};
use tokio_stream::StreamExt as _;

use crate::ui::{auth::SessionUser, events_view};

/// SSE feed of event updates.
///
/// For each broadcast message the stream emits two frames:
/// 1. `event: event-row` carrying the rendered row HTML, consumed by
///    the events list (prepended via `hx-swap="afterbegin"`).
/// 2. `event: event-row-{id}` carrying an empty payload, used by the
///    detail page as a *targeted* trigger so it only reloads itself
///    when its own event updates (not on every other event).
///
/// The list-row payload includes an out-of-band delete marker
/// (`hx-swap-oob="delete"`) for the same `id`, so updates replace any
/// existing row in place rather than duplicating it. This is the
/// CSP-safe equivalent of an inline `<script>` remove.
///
/// On channel lag (slow consumer during burst), emits `event: resync`
/// so HTMX re-fetches the rows partial instead of losing events.
///
/// 15s keep-alive prevents Cloudflare/nginx ~100s idle disconnect.
#[get("/ui/events/stream")]
pub async fn events_stream(manager: Data<PulseManager>, _user: SessionUser) -> impl Responder {
    let rx = manager.subscribe();
    let base = manager.settings.app.base_path.clone();

    // Each broadcast message expands to multiple SSE frames, so we pump
    // through a bounded mpsc instead of using `.map` (which is 1:1).
    // When the client disconnects, the receiver is dropped, the next
    // `send` returns Err, and the pump task exits cleanly.
    let (tx, rx_out) = mpsc::channel::<Result<sse::Event, Infallible>>(64);
    tokio::spawn(async move {
        let mut input = BroadcastStream::new(rx);
        while let Some(msg) = input.next().await {
            let frames: Vec<sse::Event> = match msg {
                Ok(b) => {
                    let id = b.event.id.clone();
                    let row = events_view::event_row(&base, &b.event).into_string();
                    // OOB delete targets any existing #evt-{id} row (no-op
                    // if absent); the second <tr> is the main payload
                    // that the tbody's hx-swap="afterbegin" prepends.
                    let row_html = format!(r#"<tr id="evt-{id}" hx-swap-oob="delete"></tr>{row}"#);
                    vec![
                        sse::Event::Data(sse::Data::new(row_html).event("event-row")),
                        // Per-id signal so only the matching detail view
                        // reloads. Empty payload is intentional: the
                        // detail page uses this purely as a trigger.
                        sse::Event::Data(sse::Data::new("").event(format!("event-row-{id}"))),
                    ]
                }
                Err(BroadcastStreamRecvError::Lagged(n)) => {
                    vec![sse::Event::Data(
                        sse::Data::new(format!("<!-- lagged {n} events; resync -->"))
                            .event("resync"),
                    )]
                }
            };
            for frame in frames {
                if tx.send(Ok(frame)).await.is_err() {
                    return;
                }
            }
        }
    });

    Sse::from_stream(ReceiverStream::new(rx_out)).with_keep_alive(Duration::from_secs(15))
}
