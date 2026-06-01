use autopulse_database::models::{ProcessStatus, ScanEvent};
use maud::{html, Markup};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};

use crate::ui::csrf::HEADER_NAME;

pub fn retry_hx_headers() -> String {
    // Null-guard so a missing csrf meta tag doesn't crash retry.
    format!(
        r#"js:{{"{HEADER_NAME}": (document.querySelector('meta[name=csrf]')||{{}}).content||''}}"#
    )
}

pub fn event_row(base: &str, ev: &ScanEvent) -> Markup {
    let status: ProcessStatus = ev.process_status.parse().unwrap_or(ProcessStatus::Pending);
    let status_str: &'static str = status.into();

    html! {
        // `click[!event.target.closest('a,button')]` suppresses the row GET
        // when the click lands on a nested link or Retry button.
        tr #{ "evt-" (ev.id) } .event.is-clickable .{ "event--" (status_str) }
            hx-get={ (base) "/ui/events/" (ev.id) }
            hx-trigger="click[!event.target.closest('a,button')]"
            hx-target=".main__inner"
            hx-select=".main__inner"
            hx-swap="outerHTML"
            hx-push-url="true"
        {
            td.cell--ts {
                time.local-ts datetime=(ev.updated_at.format("%Y-%m-%dT%H:%M:%SZ")) {
                    (ev.updated_at.format("%Y-%m-%d %H:%M:%S"))
                }
            }
            td.cell--src   { (ev.event_source) }
            td.cell--path title=(ev.file_path) {
                a.cell--path__link href={ (base) "/ui/events/" (ev.id) } { (ev.file_path) }
            }
            td.cell--status {
                span.badge .{ "badge--" (status_str) } { (status_str) }
            }
            td.cell--failure { "—" }
            td.cell--actions {
                @if matches!(status, ProcessStatus::Failed | ProcessStatus::Retry) {
                    button.btn--retry
                        hx-post={ (base) "/ui/events/" (ev.id) "/retry" }
                        hx-target={ "#evt-" (ev.id) }
                        hx-swap="outerHTML"
                        hx-headers=(retry_hx_headers())
                    { "Retry" }
                } @else { "" }
            }
        }
    }
}

pub fn event_rows(base: &str, events: &[ScanEvent]) -> Markup {
    html! { @for ev in events { (event_row(base, ev)) } }
}

/// Uses `beforeend` into tbody, not `outerHTML` on the row: swapping a
/// bare `<tr>` via `outerHTML` dissolves the parent `<tbody>`, breaking
/// the `#events-body` SSE/filter target.
pub fn load_more(base: &str, status: Option<&str>, search: Option<&str>, next_page: u64) -> Markup {
    // Encode `status` too: an attacker-crafted `?status=%26page%3D999` would
    // otherwise corrupt the infinite-scroll URL.
    let query = match (status, search.filter(|s| !s.is_empty())) {
        (Some(s), Some(q)) => format!(
            "?status={}&search={}&page={next_page}",
            utf8_percent_encode(s, NON_ALPHANUMERIC),
            utf8_percent_encode(q, NON_ALPHANUMERIC)
        ),
        (Some(s), None) => format!(
            "?status={}&page={next_page}",
            utf8_percent_encode(s, NON_ALPHANUMERIC)
        ),
        (None, Some(q)) => format!(
            "?search={}&page={next_page}",
            utf8_percent_encode(q, NON_ALPHANUMERIC)
        ),
        (None, None) => format!("?page={next_page}"),
    };
    html! {
        tr.load-more
            hx-get={ (base) "/ui/events/rows" (query) }
            hx-trigger="revealed"
            hx-target="#events-body"
            hx-swap="beforeend"
            "hx-on::after-request"="this.remove()"
        {
            td.load-more__cell colspan="6" { "Loading more…" }
        }
    }
}

pub fn rows_page(
    base: &str,
    events: &[ScanEvent],
    status: Option<&str>,
    search: Option<&str>,
    page: u64,
    page_size: u8,
) -> Markup {
    html! {
        (event_rows(base, events))
        @if events.len() as u8 == page_size {
            (load_more(base, status, search, page + 1))
        }
    }
}
