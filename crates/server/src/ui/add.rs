use actix_web::{
    error::{ErrorBadRequest, ErrorInternalServerError},
    get, post,
    web::{Data, Form, Query},
    HttpResponse, Result,
};
use autopulse_database::models::NewScanEvent;
use autopulse_service::{
    manager::PulseManager,
    settings::triggers::{manual::Manual, Trigger},
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
}

const BUILTIN_TRIGGER: &str = "manual";

fn manual_trigger(manager: &PulseManager) -> Manual {
    match manager.settings.triggers.get(BUILTIN_TRIGGER) {
        Some(Trigger::Manual(m) | Trigger::Bazarr(m)) => m.clone(),
        _ => Manual {
            rewrite: None,
            timer: None,
            excludes: vec![],
        },
    }
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

    let body = html! {
        section.add {
            header.page-head {
                h1.page-title { "Add scan event" }
                span.page-meta { "manual trigger" }
            }
            @if let Some(err) = &q.error {
                p.login__error { (err) }
            }

            .add-grid {
                form.form method="post" action={ (base) "/ui/add" } {
                    input type="hidden" name="csrf" value=(csrf);

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

fn preview_path(manager: &PulseManager, path: &str) -> (String, bool) {
    let m = manual_trigger(manager);
    if let Some(rw) = &m.rewrite {
        return (rw.rewrite_path(path.to_string()), true);
    }
    (path.to_string(), false)
}

fn preview_fragment(manager: &PulseManager, path: &str, exists: bool) -> Markup {
    let path = path.trim();

    let (rewritten, had_rewrite) = preview_path(manager, path);
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
async fn preview(manager: &PulseManager, path: &str) -> Markup {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return html! {
            p.preview__hint {
                "Type a path to preview how it'll be rewritten before autopulse sends it to your targets."
            }
        };
    }

    let (rewritten, _) = preview_path(manager, trimmed);
    let exists = actix_web::web::block(move || std::path::Path::new(&rewritten).exists())
        .await
        .unwrap_or(false);

    preview_fragment(manager, path, exists)
}

#[get("/ui/add/preview")]
pub async fn add_preview(
    manager: Data<PulseManager>,
    q: Query<AddQuery>,
    _user: SessionUser,
) -> Markup {
    let path = q.path.as_deref().unwrap_or("");
    preview(&manager, path).await
}

#[get("/ui/add")]
pub async fn add_page(
    manager: Data<PulseManager>,
    q: Query<AddQuery>,
    _user: SessionUser,
    csrf: CsrfToken,
) -> Result<Markup> {
    let path = q.path.as_deref().unwrap_or("");
    let preview = preview(&manager, path).await;

    render_form(&manager, &csrf.0, &q, preview)
}

#[derive(Deserialize)]
pub struct AddForm {
    pub csrf: String,
    pub path: String,
    pub hash: Option<String>,
}

#[post("/ui/add")]
pub async fn add_post(
    manager: Data<PulseManager>,
    _user: SessionUser,
    csrf: CsrfToken,
    form: Form<AddForm>,
) -> Result<HttpResponse> {
    // Plain HTML form: CSRF is in the hidden input from `render_form`.
    if manager.settings.auth.enabled && !csrf::validate_eq(&form.csrf, &csrf.0) {
        return Err(ErrorBadRequest("CSRF token mismatch"));
    }

    let inner = manual_trigger(&manager);

    let mut file_path = form.path.trim().to_string();
    if file_path.is_empty() {
        return Err(ErrorBadRequest("path is required"));
    }
    if let Some(rewrite) = &inner.rewrite {
        file_path = rewrite.rewrite_path(file_path);
    }

    let hash = form
        .hash
        .as_ref()
        .map(|h| h.trim().to_string())
        .filter(|h| !h.is_empty());

    let wait = inner
        .timer
        .clone()
        .unwrap_or_default()
        .wait
        .unwrap_or(manager.settings.opts.default_timer_wait) as i64;

    let new_scan_event = NewScanEvent {
        event_source: BUILTIN_TRIGGER.to_string(),
        file_path,
        file_hash: hash,
        can_process: chrono::Utc::now().naive_utc() + chrono::Duration::seconds(wait),
        ..Default::default()
    };

    let ev = manager
        .add_event(&new_scan_event)
        .map_err(ErrorInternalServerError)?;

    let base = &manager.settings.app.base_path;
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("{base}/ui/events/{}", ev.id)))
        .finish())
}
