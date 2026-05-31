use actix_web::{get, web::Data, Responder};
use actix_web_lab::sse::{self, Sse};
use autopulse_service::manager::PulseManager;
use std::{convert::Infallible, time::Duration};
use tokio::sync::mpsc;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_stream::wrappers::{BroadcastStream, ReceiverStream};
use tokio_stream::StreamExt as _;

use crate::ui::{auth::SessionUser, events_view};

/// SSE feed of state transitions. For each broadcast we emit:
/// 1. `event-row` — the rendered row, with an OOB `hx-swap-oob="delete"`
///    marker so updates replace any existing row in place (CSP-safe vs. inline JS).
/// 2. `event-row-{id}` — empty, used by the detail page as a per-id trigger.
///
/// On `Lagged`, emit `resync` so HTMX re-fetches rather than dropping events.
/// 15s keep-alive defeats Cloudflare/nginx ~100s idle disconnect.
#[get("/ui/events/stream")]
pub async fn events_stream(manager: Data<PulseManager>, _user: SessionUser) -> impl Responder {
    let rx = manager.subscribe();
    let base = manager.settings.app.base_path.clone();

    // Each broadcast expands to multiple SSE frames, so pump through a bounded
    // mpsc instead of `.map` (which is 1:1).
    let (tx, rx_out) = mpsc::channel::<Result<sse::Event, Infallible>>(64);
    tokio::spawn(async move {
        let mut input = BroadcastStream::new(rx);
        while let Some(msg) = input.next().await {
            let frames: Vec<sse::Event> = match msg {
                Ok(b) => {
                    let id = b.event.id.clone();
                    let row = events_view::event_row(&base, &b.event).into_string();
                    let row_html = format!(r#"<tr id="evt-{id}" hx-swap-oob="delete"></tr>{row}"#);
                    vec![
                        sse::Event::Data(sse::Data::new(row_html).event("event-row")),
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
