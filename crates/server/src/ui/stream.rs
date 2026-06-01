use actix_web::{
    get,
    web::{Data, Query},
    Responder,
};
use actix_web_lab::sse::{self, Sse};
use autopulse_database::models::ScanEvent;
use autopulse_service::manager::{EventBroadcast, PulseManager};
use serde::Deserialize;
use std::{convert::Infallible, time::Duration};
use tokio::sync::mpsc;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_stream::wrappers::{BroadcastStream, ReceiverStream};
use tokio_stream::StreamExt as _;

use crate::ui::{auth::SessionUser, events_view};

#[derive(Deserialize)]
pub struct StreamQuery {
    pub status: Option<String>,
    pub search: Option<String>,
    /// Switches the stream to detail mode: emits only `event-row-{id}`
    /// frames matching this id; no list rows, no resync.
    pub id: Option<String>,
}

/// 15s keep-alive defeats the ~100s idle disconnect on Cloudflare / nginx.
#[get("/ui/events/stream")]
pub async fn events_stream(
    manager: Data<PulseManager>,
    _user: SessionUser,
    q: Query<StreamQuery>,
) -> impl Responder {
    let rx = manager.subscribe();
    let base = manager.settings.app.base_path.clone();
    let detail_id = q.id.clone().filter(|s| !s.is_empty());
    let status = q.status.clone().filter(|s| !s.is_empty());
    let search = q
        .search
        .clone()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_ascii_lowercase());

    let (tx, rx_out) = mpsc::channel::<Result<sse::Event, Infallible>>(64);
    tokio::spawn(async move {
        let mut input = BroadcastStream::new(rx);
        loop {
            // `tx.closed()` exits the pump on client disconnect without
            // waiting for the next broadcast.
            let msg = tokio::select! {
                msg = input.next() => msg,
                () = tx.closed() => return,
            };
            let Some(msg) = msg else { return };
            let frames = build_frames(
                &base,
                msg,
                status.as_deref(),
                search.as_deref(),
                detail_id.as_deref(),
            );
            for frame in frames {
                if tx.send(Ok(frame)).await.is_err() {
                    return;
                }
            }
        }
    });

    Sse::from_stream(ReceiverStream::new(rx_out)).with_keep_alive(Duration::from_secs(15))
}

fn build_frames(
    base: &str,
    msg: Result<EventBroadcast, BroadcastStreamRecvError>,
    status: Option<&str>,
    search_lower: Option<&str>,
    detail_id: Option<&str>,
) -> Vec<sse::Event> {
    match msg {
        Ok(b) => {
            let id = b.event.id.clone();
            if let Some(want) = detail_id {
                if id == want {
                    vec![sse::Event::Data(
                        sse::Data::new("").event(format!("event-row-{id}")),
                    )]
                } else {
                    vec![]
                }
            } else {
                let matches = matches_filter(&b.event, status, search_lower);
                let row_html = if matches {
                    let row = events_view::event_row(base, &b.event).into_string();
                    format!(r#"<tr id="evt-{id}" hx-swap-oob="delete"></tr>{row}"#)
                } else {
                    format!(r#"<tr id="evt-{id}" hx-swap-oob="delete"></tr>"#)
                };
                vec![sse::Event::Data(
                    sse::Data::new(row_html).event("event-row"),
                )]
            }
        }
        Err(BroadcastStreamRecvError::Lagged(n)) => {
            if detail_id.is_some() {
                vec![]
            } else {
                vec![sse::Event::Data(
                    sse::Data::new(format!("<!-- lagged {n} events; resync -->")).event("resync"),
                )]
            }
        }
    }
}

fn matches_filter(ev: &ScanEvent, status: Option<&str>, search_lower: Option<&str>) -> bool {
    if status.is_some_and(|s| ev.process_status != s) {
        return false;
    }
    if let Some(q) = search_lower {
        if !ev.file_path.to_ascii_lowercase().contains(q) {
            return false;
        }
    }
    true
}
