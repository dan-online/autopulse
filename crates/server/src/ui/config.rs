use actix_web::{error::ErrorInternalServerError, get, web::Data, Result};
use autopulse_service::manager::PulseManager;
use maud::{html, Markup};
use serde_json::Value;

use crate::ui::{
    auth::{ctx, SessionUser},
    csrf::CsrfToken,
    layout,
};

const MASK: &str = "••••••••";

/// Read-only config viewer. Secrets redacted before rendering.
#[get("/ui/config")]
pub async fn config_page(
    manager: Data<PulseManager>,
    _user: SessionUser,
    csrf: CsrfToken,
) -> Result<Markup> {
    // Settings is Serialize; go via serde_json::Value so we can walk and
    // redact generically rather than hand-listing every secret field.
    let mut value = serde_json::to_value(&*manager.settings).map_err(ErrorInternalServerError)?;
    redact(&mut value, false);

    let raw = serde_json::to_string_pretty(&value).map_err(ErrorInternalServerError)?;

    let ctx_ = ctx(&manager, &csrf.0);
    let body = html! {
        section.config {
            header.page-head {
                h1.page-title { "Configuration" }
                span.page-meta { "read-only · secrets redacted" }
            }

            .config-grid {
                .panel {
                    .panel__head { "Parsed config" }
                    .panel__body {
                        (render_value(&value, 0, &[]))
                    }
                }
                .panel {
                    .panel__head { "Raw" }
                    .panel__body.panel__body--flush {
                        pre.config-raw { code { (raw) } }
                    }
                }
            }
        }
    };

    Ok(layout::page(&ctx_, "config", "config", body))
}

/// Path-aware so we can mask webhook URLs (which are credentials)
/// without masking target URLs (which are useful to see).
fn redact(v: &mut Value, in_webhooks: bool) {
    match v {
        Value::Object(map) => {
            for (k, val) in map.iter_mut() {
                let kl = k.to_lowercase();
                let secret = [
                    "password",
                    "token",
                    "secret",
                    "apikey",
                    "api_key",
                    "authorization",
                ]
                .iter()
                .any(|s| kl.contains(s));

                if secret || (kl == "url" && in_webhooks) {
                    *val = Value::String(MASK.into());
                } else if kl == "database_url" {
                    if let Some(s) = val.as_str() {
                        *val = Value::String(mask_db_url(s));
                    }
                } else {
                    redact(val, in_webhooks || k == "webhooks");
                }
            }
        }
        Value::Array(arr) => arr.iter_mut().for_each(|x| redact(x, in_webhooks)),
        _ => {}
    }
}

/// SQLite paths have no creds and pass through unchanged.
fn mask_db_url(s: &str) -> String {
    // split scheme://rest
    if let Some((scheme, rest)) = s.split_once("://") {
        if let Some((userinfo, hostpart)) = rest.split_once('@') {
            // userinfo may be user or user:pass — mask whatever it is
            let user = userinfo.split_once(':').map(|(u, _)| u).unwrap_or(userinfo);
            return format!("{scheme}://{user}:{MASK}@{hostpart}");
        }
    }
    s.to_string()
}

const DOCS: &str = "https://autopulse.dancodes.online/autopulse_service/settings";

/// Map a config path to its rustdoc page on autopulse.dancodes.online.
fn doc_for(path: &[&str]) -> Option<String> {
    // The docs site serves canonical URLs without the `.html` extension
    // (the `.html` form 308-redirects to these), so link directly.
    let url = match path {
        ["app"] => format!("{DOCS}/app/struct.App"),
        ["auth"] => format!("{DOCS}/auth/struct.Auth"),
        ["opts"] => format!("{DOCS}/opts/struct.Opts"),
        ["triggers"] | ["triggers", _] => format!("{DOCS}/triggers/enum.Trigger"),
        ["targets"] | ["targets", _] => format!("{DOCS}/targets/enum.Target"),
        ["webhooks"] | ["webhooks", _] => format!("{DOCS}/webhooks/enum.Webhook"),
        ["anchors"] => format!("{DOCS}/struct.Settings#structfield.anchors"),
        // sub-options: flat sections have a doc page per struct, with a
        // `#structfield.<name>` anchor for each field. (triggers/targets/
        // webhooks sub-fields are skipped — their struct depends on the
        // `type` and nested sub-structs live in other modules, so a
        // precise anchor isn't reliably derivable.)
        ["app", f] => format!("{DOCS}/app/struct.App#structfield.{f}"),
        ["auth", f] => format!("{DOCS}/auth/struct.Auth#structfield.{f}"),
        ["opts", f] => format!("{DOCS}/opts/struct.Opts#structfield.{f}"),
        _ => return None,
    };
    Some(url)
}

fn doc_link(path: &[&str]) -> Markup {
    match doc_for(path) {
        Some(url) => html! {
            a.cfg-doc href=(url) target="_blank" rel="noopener noreferrer" title="View documentation" {
                "↗"
            }
        },
        None => html! {},
    }
}

/// Top two levels open by default.
fn render_value(v: &Value, depth: usize, path: &[&str]) -> Markup {
    match v {
        Value::Null => html! { span.cfg-null { "null" } },
        Value::Bool(b) => html! { span.cfg-bool { (b) } },
        Value::Number(n) => html! { span.cfg-num { (n.to_string()) } },
        Value::String(s) => html! { span.cfg-str { (s) } },
        Value::Array(a) if a.is_empty() => html! { span.cfg-empty { "empty" } },
        Value::Object(m) if m.is_empty() => html! { span.cfg-empty { "empty" } },
        Value::Array(a) => html! {
            ul.cfg-tree {
                @for (i, item) in a.iter().enumerate() {
                    li.cfg-row {
                        @if item.is_object() || item.is_array() {
                            details.cfg-group open[depth < 2] {
                                summary.cfg-key { "[" (i) "]" }
                                (render_value(item, depth + 1, path))
                            }
                        } @else {
                            span.cfg-key { "[" (i) "]" }
                            (render_value(item, depth + 1, path))
                        }
                    }
                }
            }
        },
        Value::Object(m) => html! {
            ul.cfg-tree {
                @for (k, val) in m {
                    @let child = { let mut c = path.to_vec(); c.push(k.as_str()); c };
                    li.cfg-row {
                        @if val.is_object() || val.is_array() {
                            details.cfg-group open[depth < 2] {
                                summary.cfg-key { (k) (doc_link(&child)) }
                                (render_value(val, depth + 1, &child))
                            }
                        } @else {
                            span.cfg-key { (k) }
                            (render_value(val, depth + 1, &child))
                            (doc_link(&child))
                        }
                    }
                }
            }
        },
    }
}
