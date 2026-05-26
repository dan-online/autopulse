use actix_web::{
    error::ErrorInternalServerError,
    get, post,
    web::{Data, Path, Query},
    HttpRequest, Result,
};
use autopulse_service::manager::PulseManager;
use maud::{html, Markup};
use serde::Deserialize;

use crate::ui::{
    auth::{ctx, SessionUser},
    csrf::{self, CsrfToken},
    events_view, layout,
};

const PAGE_SIZE: u8 = 25;

#[derive(Deserialize)]
pub struct EventsQuery {
    pub status: Option<String>,
    pub search: Option<String>,
    #[serde(default = "default_page")]
    pub page: u64,
}
fn default_page() -> u64 {
    1
}

/// Swap unit for filter switches and search results. Stat-card clicks and
/// the search input both target this section via `outerHTML`. The search
/// input uses `hx-preserve` so HTMX keeps the focused DOM element across
/// swaps instead of destroying and recreating it.
fn events_section(manager: &PulseManager, q: &EventsQuery) -> Result<Markup> {
    // Normalize empty strings to None so `?status=` (sent by the hidden
    // carrier input when no filter is active) doesn't filter for "".
    let status = q.status.as_deref().filter(|s| !s.is_empty());
    let search = q.search.as_deref().filter(|s| !s.is_empty());

    let events = manager
        .get_events(
            PAGE_SIZE,
            q.page,
            None,
            status.map(String::from),
            search.map(String::from),
        )
        .map_err(ErrorInternalServerError)?;
    let total = manager
        .count_events(status.map(String::from), search.map(String::from))
        .map_err(ErrorInternalServerError)?;

    let base = manager.settings.app.base_path.as_str();
    let stats = manager.get_stats().map_err(ErrorInternalServerError)?;

    Ok(html! {
        section.events #events-section {
            // Carries the active status filter for the search input's
            // hx-include so search requests always compose both filters.
            input #events-status type="hidden" name="status"
                value=(status.unwrap_or(""));

            header.page-head {
                h1.page-title { "Scan events" }
                span.page-meta {
                    (status.unwrap_or("all")) " · "
                    span data-num=(total) { (total) }
                }
            }

            (stats_cards(base, &stats, status, search))

            // hx-preserve keeps the focused input across outerHTML swaps
            // so typing doesn't lose cursor position or focus.
            .events__search {
                input #events-search .search__input
                    hx-preserve
                    type="search"
                    name="search"
                    placeholder="Search file paths\u{2026}"
                    value=(search.unwrap_or(""))
                    hx-get={ (base) "/ui/events" }
                    hx-trigger="input changed delay:300ms, search"
                    hx-target="#events-section"
                    hx-swap="outerHTML"
                    hx-replace-url="true"
                    hx-include="#events-status"
                    autocomplete="off"
                ;
            }

            .events__table-wrap {
              .events__table-scroll {
                table.events__table {
                    thead { tr {
                        th { "When" } th { "Source" } th { "Path" }
                        th { "Status" } th { "Failure" } th {}
                    } }
                    tbody #events-body
                        hx-ext="sse"
                        sse-connect={ (base) "/ui/events/stream" }
                        sse-swap="event-row"
                        hx-swap="afterbegin"
                        hx-trigger="sse:resync"
                        hx-get={ (base) "/ui/events/rows" }
                        hx-target="this"
                    {
                        (events_view::rows_page(base, &events, status, search, q.page, PAGE_SIZE))
                    }
                }
              }
            }
        }
    })
}

fn filter_query(status: Option<&str>, search: Option<&str>) -> String {
    let search = search.filter(|s| !s.is_empty());
    match (status, search) {
        (Some(v), Some(q)) => {
            format!(
                "?status={v}&search={}",
                percent_encoding::utf8_percent_encode(q, percent_encoding::NON_ALPHANUMERIC)
            )
        }
        (Some(v), None) => format!("?status={v}"),
        (None, Some(q)) => {
            format!(
                "?search={}",
                percent_encoding::utf8_percent_encode(q, percent_encoding::NON_ALPHANUMERIC)
            )
        }
        (None, None) => String::new(),
    }
}

fn stats_cards(
    base: &str,
    stats: &autopulse_service::manager::Stats,
    status: Option<&str>,
    search: Option<&str>,
) -> Markup {
    let cards: [(_, i64, _, icondata::Icon, Option<&str>); 5] = [
        ("Pending", stats.pending, "Waiting in queue", icondata::LuLayers, Some("pending")),
        ("Retrying", stats.retrying, "Failed 1/+ processors", icondata::LuRefreshCw, Some("retry")),
        ("Processed", stats.processed, "Sent to processors", icondata::LuPackageCheck, Some("complete")),
        ("Failed", stats.failed, "Failed to process", icondata::LuCircleAlert, Some("failed")),
        ("Total", stats.total, "Total scan events", icondata::LuCopy, None),
    ];

    html! {
        .stats
            hx-ext="sse"
            sse-connect={ (base) "/ui/events/stream" }
            hx-trigger="sse:event-row throttle:5s"
            hx-get={ (base) "/ui/events/stats" }
            hx-swap="outerHTML"
        {
            @for (label, value, sub, ico, filter) in cards {
                a.stat
                    .is-active[status == filter]
                    hx-get={ (base) "/ui/events" (filter_query(filter, search)) }
                    hx-target="#events-section"
                    hx-swap="outerHTML"
                    hx-push-url="true"
                    href={ (base) "/ui/events" (filter_query(filter, search)) }
                {
                    .stat__body {
                        span.stat__label { (label) }
                        span.stat__value data-num=(value) { (value) }
                        span.stat__sub { (sub) }
                    }
                    span.stat__icon { (layout::icon(ico, 22)) }
                }
            }
        }
    }
}

#[get("/ui/events")]
pub async fn events_page(
    manager: Data<PulseManager>,
    q: Query<EventsQuery>,
    _user: SessionUser,
    csrf: CsrfToken,
    req: HttpRequest,
) -> Result<Markup> {
    let section = events_section(&manager, &q)?;
    if req.headers().contains_key("HX-Request") {
        Ok(section)
    } else {
        let ctx = ctx(&manager, &csrf.0);
        Ok(layout::page(&ctx, "events", "events", section))
    }
}

/// HTMX fragment: just the stat cards, for live SSE refresh.
#[get("/ui/events/stats")]
pub async fn events_stats(
    manager: Data<PulseManager>,
    q: Query<EventsQuery>,
    _user: SessionUser,
) -> Result<Markup> {
    let status = q.status.as_deref().filter(|s| !s.is_empty());
    let search = q.search.as_deref().filter(|s| !s.is_empty());
    let base = manager.settings.app.base_path.as_str();
    let stats = manager.get_stats().map_err(ErrorInternalServerError)?;
    Ok(stats_cards(base, &stats, status, search))
}

/// HTMX rows fragment for infinite-scroll appends and SSE resync.
/// Direct browser hits fall back to the full events page.
#[get("/ui/events/rows")]
pub async fn events_rows(
    manager: Data<PulseManager>,
    q: Query<EventsQuery>,
    _user: SessionUser,
    csrf: CsrfToken,
    req: HttpRequest,
) -> Result<Markup> {
    if !req.headers().contains_key("HX-Request") {
        let ctx = ctx(&manager, &csrf.0);
        return Ok(layout::page(
            &ctx,
            "events",
            "events",
            events_section(&manager, &q)?,
        ));
    }
    let status = q.status.as_deref().filter(|s| !s.is_empty());
    let search = q.search.as_deref().filter(|s| !s.is_empty());
    let events = manager
        .get_events(
            PAGE_SIZE,
            q.page,
            None,
            status.map(String::from),
            search.map(String::from),
        )
        .map_err(ErrorInternalServerError)?;
    let base = manager.settings.app.base_path.as_str();
    Ok(events_view::rows_page(
        base, &events, status, search, q.page, PAGE_SIZE,
    ))
}

/// `failed_times` preserved — manual retry is an impulse, not history-erase.
#[post("/ui/events/{id}/retry")]
pub async fn event_retry(
    manager: Data<PulseManager>,
    id: Path<String>,
    _user: SessionUser,
    csrf: CsrfToken,
    req: HttpRequest,
) -> Result<Markup> {
    csrf::require_header(&req, &csrf)?;
    let ev = manager
        .reschedule_event(&id)
        .map_err(ErrorInternalServerError)?;
    let base = manager.settings.app.base_path.as_str();
    Ok(events_view::event_row(base, &ev))
}
