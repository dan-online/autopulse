use actix_web::{
    error::{ErrorBadRequest, ErrorInternalServerError},
    get, post,
    web::{Data, Form, Query},
    HttpResponse, Result,
};
use autopulse_database::models::NewScanEvent;
use autopulse_service::{
    manager::PulseManager,
    settings::{rewrite::Rewrite, webhooks::EventType},
};
use maud::{html, Markup};
use serde::Deserialize;

use crate::ui::{
    auth::{ctx, SessionUser},
    csrf::{self, CsrfToken},
    layout,
};

#[derive(Deserialize, Default)]
pub struct AddQuery {
    pub error: Option<String>,
    pub path: Option<String>,
    pub hash: Option<String>,
    pub trigger: Option<String>,
}

const BUILTIN_TRIGGER: &str = "manual";

struct Resolved {
    name: String,
    rewrite: Option<Rewrite>,
    wait: u64,
}

/// Resolves the picked trigger's rewrite + timer, falling back to the
/// built-in `manual` (with no rewrite) when no trigger is configured.
fn resolve_trigger(manager: &PulseManager, name: Option<&str>) -> Resolved {
    let default_wait = manager.settings.opts.default_timer_wait;
    let pick = name
        .filter(|s| !s.is_empty())
        .and_then(|n| manager.settings.triggers.get(n).map(|t| (n.to_string(), t)))
        .or_else(|| {
            manager
                .settings
                .triggers
                .get(BUILTIN_TRIGGER)
                .map(|t| (BUILTIN_TRIGGER.to_string(), t))
        });

    match pick {
        Some((name, trigger)) => Resolved {
            name,
            rewrite: trigger.get_rewrite().cloned(),
            wait: trigger.get_timer(None).wait.unwrap_or(default_wait),
        },
        None => Resolved {
            name: BUILTIN_TRIGGER.to_string(),
            rewrite: None,
            wait: default_wait,
        },
    }
}

/// All trigger names in config plus a synthetic `manual` when none is set.
fn trigger_names(manager: &PulseManager) -> Vec<String> {
    let mut names: Vec<String> = manager.settings.triggers.keys().cloned().collect();
    if !names.iter().any(|n| n == BUILTIN_TRIGGER) {
        names.push(BUILTIN_TRIGGER.to_string());
    }
    names.sort();
    names
}

fn render_form(
    manager: &PulseManager,
    csrf: &str,
    q: &AddQuery,
    preview: Markup,
) -> Result<Markup> {
    let ctx_ = ctx(manager, csrf);
    let base = ctx_.base;
    let path = q.path.as_deref().unwrap_or("");
    let preview_url = format!("{base}/ui/add/preview");
    let triggers = trigger_names(manager);
    let selected = resolve_trigger(manager, q.trigger.as_deref()).name;

    let body = html! {
        section.add {
            header.page-head {
                h1.page-title { "Add scan event" }
                span.page-meta { (selected) }
            }
            @if let Some(err) = &q.error {
                p.login__error { (err) }
            }

            .add-grid {
                form.form method="post" action={ (base) "/ui/add" } {
                    input type="hidden" name="csrf" value=(csrf);

                    label.form__field {
                        span.form__label { "Trigger" }
                        select name="trigger"
                            hx-get=(preview_url)
                            hx-trigger="change"
                            hx-target="#rewrite-preview"
                            hx-include="closest form"
                        {
                            @for name in &triggers {
                                option value=(name) selected[name == &selected] { (name) }
                            }
                        }
                        span.form__hint { "Event source for this scan; its rewrite/timer applies." }
                    }

                    label.form__field {
                        span.form__label { "File path" }
                        input type="text" name="path" required
                            placeholder="/downloads/Show/Season 01/episode.mkv"
                            value=(path)
                            autofocus
                            hx-get=(preview_url)
                            hx-trigger="keyup changed delay:250ms, change"
                            hx-target="#rewrite-preview"
                            hx-include="closest form";
                        span.form__hint { "The path as the trigger reports it (before rewrite)." }
                    }

                    label.form__field {
                        span.form__label { "Hash" span.form__optional { "optional" } }
                        input type="text" name="hash"
                            placeholder="sha256 — leave blank to skip verification"
                            value=(q.hash.as_deref().unwrap_or(""));
                        span.form__hint { "If set, autopulse waits until the file on disk matches this checksum before scanning." }
                    }

                    div.form__actions {
                        a.btn--ghost href={ (base) "/ui/events" } { "Cancel" }
                        button.btn--primary type="submit" { "Add scan" }
                    }
                }

                aside.add__side {
                    .panel {
                        .panel__head { "Rewrite preview" }
                        #rewrite-preview .panel__body {
                            (preview)
                        }
                    }
                }
            }
        }
    };

    Ok(layout::page(&ctx_, "add scan", "add", body))
}

fn preview_path(resolved: &Resolved, path: &str) -> (String, bool) {
    if let Some(rw) = &resolved.rewrite {
        return (rw.rewrite_path(path.to_string()), true);
    }
    (path.to_string(), false)
}

fn preview_fragment(resolved: &Resolved, path: &str, exists: bool) -> Markup {
    let path = path.trim();

    let (rewritten, had_rewrite) = preview_path(resolved, path);
    let changed = rewritten != path;

    html! {
        @if changed {
            .preview__row {
                span.preview__tag { "input" }
                code.preview__path.preview__path--from { (path) }
            }
            .preview__arrow { "↓" }
            .preview__row {
                span.preview__tag.preview__tag--accent { "rewritten" }
                code.preview__path.preview__path--to { (rewritten) }
            }
        } @else {
            .preview__row {
                span.preview__tag { "path" }
                code.preview__path.preview__path--to { (rewritten) }
            }
            p.preview__note {
                @if had_rewrite { "Rewrite rule didn't match — path used unchanged." }
                @else { "No rewrite configured for this trigger — path used as-is." }
            }
        }
        .preview__disk.{ "preview__disk--" (if exists { "ok" } else { "miss" }) } {
            @if exists { "✓ Found on disk — autopulse can see this file" }
            @else { "✗ Not found on disk at this path on the autopulse host" }
        }
    }
}

/// Probes filesystem on the blocking pool; `Path::exists()` would stall
/// an actix worker on slow mounts.
async fn preview(manager: &PulseManager, q: &AddQuery) -> Markup {
    let trimmed = q.path.as_deref().unwrap_or("").trim();
    if trimmed.is_empty() {
        return html! {
            p.preview__hint {
                "Type a path to preview how it'll be rewritten before autopulse sends it to your targets."
            }
        };
    }

    let resolved = resolve_trigger(manager, q.trigger.as_deref());
    let (rewritten, _) = preview_path(&resolved, trimmed);
    let exists = actix_web::web::block(move || std::path::Path::new(&rewritten).exists())
        .await
        .unwrap_or(false);

    preview_fragment(&resolved, trimmed, exists)
}

#[get("/ui/add/preview")]
pub async fn add_preview(
    manager: Data<PulseManager>,
    q: Query<AddQuery>,
    _user: SessionUser,
) -> Markup {
    preview(&manager, &q).await
}

#[get("/ui/add")]
pub async fn add_page(
    manager: Data<PulseManager>,
    q: Query<AddQuery>,
    _user: SessionUser,
    csrf: CsrfToken,
) -> Result<Markup> {
    let preview = preview(&manager, &q).await;
    render_form(&manager, &csrf.0, &q, preview)
}

#[derive(Deserialize)]
pub struct AddForm {
    pub csrf: String,
    pub path: String,
    pub hash: Option<String>,
    pub trigger: Option<String>,
}

#[post("/ui/add")]
pub async fn add_post(
    manager: Data<PulseManager>,
    _user: SessionUser,
    csrf: CsrfToken,
    form: Form<AddForm>,
) -> Result<HttpResponse> {
    if manager.settings.auth.enabled && !csrf::validate_eq(&form.csrf, &csrf.0) {
        return Err(ErrorBadRequest("CSRF token mismatch"));
    }

    let resolved = resolve_trigger(&manager, form.trigger.as_deref());

    let mut file_path = form.path.trim().to_string();
    if file_path.is_empty() {
        return Err(ErrorBadRequest("path is required"));
    }
    if let Some(rewrite) = &resolved.rewrite {
        file_path = rewrite.rewrite_path(file_path);
    }

    let hash = form
        .hash
        .as_ref()
        .map(|h| h.trim().to_string())
        .filter(|h| !h.is_empty());

    let new_scan_event = NewScanEvent {
        event_source: resolved.name.clone(),
        file_path,
        file_hash: hash,
        can_process: chrono::Utc::now().naive_utc()
            + chrono::Duration::seconds(resolved.wait as i64),
        ..Default::default()
    };

    let ev = manager
        .add_event(&new_scan_event)
        .map_err(ErrorInternalServerError)?;

    // Mirror /triggers/manual: legacy callers fire the New webhook after
    // add_event, and operators rely on it to drive notifications.
    manager
        .webhooks
        .add_event(
            EventType::New,
            Some(resolved.name.clone()),
            std::slice::from_ref(&ev.file_path),
        )
        .await;

    let base = &manager.settings.app.base_path;
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("{base}/ui/events/{}", ev.id)))
        .finish())
}
