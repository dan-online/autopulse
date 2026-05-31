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
    // Empty strings come from the hidden status carrier; treat as "no filter".
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

    let filter_qs = filter_query(status, search);

    Ok(html! {
        // One EventSource per tab; the stream URL carries the active filter
        // so the server only emits rows that belong in this view.
        section.events #events-section
            hx-ext="sse"
            sse-connect={ (base) "/ui/events/stream" (filter_qs) }
        {
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

            // hx-preserve keeps focus/value across the section's outerHTML swaps.
            // The trigger filter skips 1-2 char inputs (cheap LIKE-scan suppression)
            // while still firing on empty to clear the filter.
            .events__search {
                input #events-search .search__input
                    hx-preserve
                    type="search"
                    name="search"
                    placeholder="Search file paths\u{2026}"
                    value=(search.unwrap_or(""))
                    hx-get={ (base) "/ui/events" }
                    hx-trigger="input changed delay:500ms[this.value.length===0||this.value.length>=3], search"
                    hx-target="#events-section"
                    hx-swap="outerHTML"
                    hx-replace-url="true"
                    hx-include="#events-status"
                    autocomplete="off"
                ;
            }

            // Resync handler split off tbody so its innerHTML swap doesn't
            // collide with tbody's afterbegin sse-swap (which would prepend
            // a duplicate copy of every row on every reconnect).
            div #events-resync
                hx-trigger="sse:resync"
                hx-get={ (base) "/ui/events/rows" (filter_qs) }
                hx-target="#events-body"
                hx-swap="innerHTML"
                hidden {}

            .events__table-wrap {
              .events__table-scroll {
                table.events__table {
                    thead { tr {
                        th { "When" } th { "Source" } th { "Path" }
                        th { "Status" } th { "Failure" } th {}
                    } }
                    tbody #events-body
                        sse-swap="event-row"
                        hx-swap="afterbegin"
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
    // Encode `status` too: it's normally an enum, but `?status=%26evil%3D1`
    // would otherwise inject query params into every link on the page.
    use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
    let search = search.filter(|s| !s.is_empty());
    match (status, search) {
        (Some(v), Some(q)) => format!(
            "?status={}&search={}",
            utf8_percent_encode(v, NON_ALPHANUMERIC),
            utf8_percent_encode(q, NON_ALPHANUMERIC)
        ),
        (Some(v), None) => format!("?status={}", utf8_percent_encode(v, NON_ALPHANUMERIC)),
        (None, Some(q)) => format!("?search={}", utf8_percent_encode(q, NON_ALPHANUMERIC)),
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
        (
            "Pending",
            stats.pending,
            "Waiting in queue",
            icondata::LuLayers,
            Some("pending"),
        ),
        (
            "Retrying",
            stats.retrying,
            "Failed 1/+ processors",
            icondata::LuRefreshCw,
            Some("retry"),
        ),
        (
            "Processed",
            stats.processed,
            "Sent to processors",
            icondata::LuPackageCheck,
            Some("complete"),
        ),
        (
            "Failed",
            stats.failed,
            "Failed to process",
            icondata::LuCircleAlert,
            Some("failed"),
        ),
        (
            "Total",
            stats.total,
            "Total scan events",
            icondata::LuCopy,
            None,
        ),
    ];

    html! {
        .stats
            hx-trigger="sse:event-row throttle:5s"
            // Carry the active filter so the periodic refresh keeps the
            // selected card's `.is-active` highlight (counts themselves
            // are global; the filter just drives the highlight state).
            hx-get={ (base) "/ui/events/stats" (filter_query(status, search)) }
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
