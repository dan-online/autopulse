use actix_web::{
    error::{ErrorInternalServerError, ErrorNotFound},
    get,
    web::{Data, Path},
    Result,
};
use autopulse_database::models::{ProcessStatus, ScanEvent};
use autopulse_service::manager::PulseManager;
use chrono::NaiveDateTime;
use maud::{html, Markup, PreEscaped};

use crate::ui::{
    auth::{ctx, SessionUser},
    csrf::CsrfToken,
    events_view, layout,
};

#[get("/ui/events/{id}")]
pub async fn event_detail(
    manager: Data<PulseManager>,
    id: Path<String>,
    _user: SessionUser,
    csrf: CsrfToken,
) -> Result<Markup> {
    let ev = manager
        .get_event(&id)
        .map_err(ErrorInternalServerError)?
        .ok_or_else(|| ErrorNotFound("event not found"))?;

    let status: ProcessStatus = ev.process_status.parse().unwrap_or(ProcessStatus::Pending);
    let status_str: &'static str = status.into();

    let targets = ev
        .targets_hit
        .split(',')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();

    let ctx_ = ctx(&manager, &csrf.0);
    let base = ctx_.base;

    // Pipeline step states.
    // found_passed is true when the file was explicitly found OR when
    // the event moved past the found stage (check_path disabled).
    let found_done = ev.found_at.is_some();
    let found_err = ev.found_status == "hash_mismatch";
    let found_passed = found_done || (!found_err && status != ProcessStatus::Pending);

    let found_state = if found_err {
        "error"
    } else if found_passed {
        "done"
    } else {
        "pending"
    };
    let proc_state = if status == ProcessStatus::Complete {
        "done"
    } else if status == ProcessStatus::Failed {
        "error"
    } else if status == ProcessStatus::Retry {
        "retry"
    } else if found_passed {
        "active"
    } else {
        "pending"
    };

    let body = html! {
        // SSE wrapper: persists across content swaps so the connection
        // stays open. innerHTML swap replaces only the inner section.
        div.detail-live
            hx-ext="sse"
            sse-connect={ (base) "/ui/events/stream" }
            hx-trigger="sse:event-row throttle:2s, retry-done"
            hx-get={ (base) "/ui/events/" (ev.id) }
            hx-select="section.detail"
            hx-swap="innerHTML"
        {
            section.detail {
                nav.crumbs {
                    a href={ (base) "/ui/events" } { "\u{2190} Events" }
                }

                div.detail__hero .{ "detail__hero--" (status_str) } {
                    div.detail__hero-top {
                        h1.detail__title { (short(&ev.id)) }
                        span.badge .{ "badge--" (status_str) } { (status_str) }
                    }
                    div.detail__hero-meta {
                        span.detail__meta-pill { (ev.event_source) }
                        @if ev.failed_times > 0 {
                            span.detail__meta-pill.detail__meta-warn {
                                (ev.failed_times)
                                @if ev.failed_times == 1 { " failure" } @else { " failures" }
                            }
                        }
                    }
                }

                div.detail__file {
                    div.detail__file-head { "FILE PATH" }
                    code.detail__file-code { (ev.file_path) }
                }

                div.detail__pipeline {
                    (pipeline_step("Queued", "done"))
                    span.detail__pipe-seg {}
                    (pipeline_step("Found", found_state))
                    span.detail__pipe-seg .is-dim[!found_passed] {}
                    (pipeline_step("Processed", proc_state))
                }

                div.detail__cards {
                    div.detail__card {
                        h3.detail__card-head { "Details" }
                        dl.detail__dl {
                            (kv("ID", html! {
                                code.mono { (ev.id) }
                                " "
                                button.btn--copy type="button"
                                    title="Copy ID"
                                    onclick={"navigator.clipboard.writeText('" (ev.id) "')"} {
                                    (layout::icon(icondata::LuCopy, 13))
                                }
                            }))
                            (kv("Source", html! { (ev.event_source) }))
                            (kv("Found status", html! {
                                span.detail__found .{ "detail__found--" (found_class(&ev.found_status)) } {
                                    (ev.found_status.replace('_', " "))
                                }
                            }))
                            (kv("Hash", html! {
                                @match &ev.file_hash {
                                    Some(h) => code.mono.detail__hash { (h) },
                                    None => span.dim { "\u{2014}" },
                                }
                            }))
                            (kv("Failed times", html! { (ev.failed_times) }))
                            (kv("Targets hit", html! {
                                @if targets.is_empty() {
                                    span.dim { "\u{2014}" }
                                } @else {
                                    ul.tags {
                                        @for t in &targets { li.tag { (t) } }
                                    }
                                }
                            }))
                        }
                    }

                    div.detail__card {
                        h3.detail__card-head { "Timeline" }
                        dl.detail__dl {
                            (kv_ts("Event timestamp", &Some(ev.event_timestamp)))
                            (kv_ts("Created", &Some(ev.created_at)))
                            (kv_ts("Updated", &Some(ev.updated_at)))
                            (kv_ts("Found at", &ev.found_at))
                            (kv_ts("Processed at", &ev.processed_at))
                            (kv_ts("Eligible at", &Some(ev.can_process)))
                            (kv_ts("Next retry at", &ev.next_retry_at))
                        }
                    }
                }

                @if matches!(status, ProcessStatus::Failed | ProcessStatus::Retry | ProcessStatus::Complete) {
                    div.detail__actions {
                        form
                            hx-post={ (base) "/ui/events/" (ev.id) "/retry" }
                            hx-headers=(events_view::retry_hx_headers())
                            hx-swap="none"
                            "hx-on::after-request"="if(event.detail.successful) htmx.trigger(this.closest('.detail-live'),'retry-done')"
                        {
                            button.btn--retry.detail__retry type="submit" {
                                (layout::icon(icondata::LuRefreshCw, 15))
                                " Retry now"
                            }
                        }
                    }
                }
            }
        }
        (relative_time_script())
    };

    Ok(layout::page(
        &ctx_,
        &format!("event {}", short(&ev.id)),
        "events",
        body,
    ))
}

/// Live relative-time updater. Runs once on load, refreshes every second,
/// and re-runs after any HTMX swap (SSE-triggered content refresh).
/// Guards against duplicate intervals and self-clears when navigated away.
fn relative_time_script() -> Markup {
    PreEscaped(
        r#"<script>
(function(){
if(window.__detailRelTimer)clearInterval(window.__detailRelTimer);
var S=1e3,M=6e4,H=36e5,D=864e5;
function rel(d){
  var df=Date.now()-d.getTime(),a=Math.abs(df),p=df>0,v,u;
  if(a<5*S) return 'just now';
  if(a<M){v=Math.round(a/S);u=v===1?'second':'seconds';}
  else if(a<H){v=Math.round(a/M);u=v===1?'minute':'minutes';}
  else if(a<D){v=Math.round(a/H);u=v===1?'hour':'hours';}
  else{v=Math.round(a/D);u=v===1?'day':'days';}
  return p?v+' '+u+' ago':'in '+v+' '+u;
}
function upd(){
  var els=document.querySelectorAll('time.detail__ts[datetime]');
  if(!els.length){clearInterval(window.__detailRelTimer);window.__detailRelTimer=0;return;}
  els.forEach(function(el){
    var sp=el.querySelector('.detail__ts-rel');
    if(!sp)return;
    var d=new Date(el.getAttribute('datetime'));
    if(isNaN(d.getTime()))return;
    sp.textContent='('+rel(d)+')';
  });
}
upd();
window.__detailRelTimer=setInterval(upd,1000);
if(!window.__detailSwapBound){
  document.body.addEventListener('htmx:afterSwap',upd);
  window.__detailSwapBound=true;
}
})();
</script>"#
            .to_string(),
    )
}

fn pipeline_step(label: &str, state: &str) -> Markup {
    html! {
        div.detail__pipe-step
            .is-done[state == "done"]
            .is-active[state == "active"]
            .is-error[state == "error"]
            .is-retry[state == "retry"]
        {
            div.detail__pipe-dot {
                @match state {
                    "done"  => (layout::icon(icondata::LuCheck, 10)),
                    "error" => (layout::icon(icondata::LuX, 10)),
                    "retry" => (layout::icon(icondata::LuRefreshCw, 9)),
                    _       => {},
                }
            }
            span.detail__pipe-label { (label) }
        }
    }
}

fn kv(label: &str, value: Markup) -> Markup {
    html! {
        div.detail__kv {
            dt.detail__kv-dt { (label) }
            dd.detail__kv-dd { (value) }
        }
    }
}

/// All `NaiveDateTime` values in the database are stored as UTC
/// (via `chrono::Utc::now().naive_utc()`), so the `Z` suffix is correct.
fn kv_ts(label: &str, ts: &Option<NaiveDateTime>) -> Markup {
    kv(
        label,
        match ts {
            Some(t) => html! {
                time.detail__ts datetime=(t.format("%Y-%m-%dT%H:%M:%SZ")) {
                    code.mono { (t.format("%Y-%m-%d %H:%M:%S")) }
                    " "
                    span.detail__ts-rel {}
                }
            },
            None => html! { span.dim { "\u{2014}" } },
        },
    )
}

fn found_class(status: &str) -> &'static str {
    match status {
        "found" => "found",
        "not_found" => "not_found",
        "hash_mismatch" => "hash_mismatch",
        _ => "unknown",
    }
}

fn short(id: &str) -> &str {
    id.get(..8).unwrap_or(id)
}

pub fn detail_href(base: &str, ev: &ScanEvent) -> String {
    format!("{base}/ui/events/{}", ev.id)
}
