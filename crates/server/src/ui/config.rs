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
                // Strip non-alphanumerics so `X-Api-Key`, `X_API_KEY`, `apiKey`
                // all normalize to the same `xapikey` and match `apikey`.
                let kn: String = k
                    .to_ascii_lowercase()
                    .chars()
                    .filter(|c| c.is_ascii_alphanumeric())
                    .collect();
                let secret = [
                    "password",
                    "token",
                    "secret",
                    "apikey",
                    "authorization",
                    "authkey",
                    "authtoken",
                    "cookie",
                ]
                .iter()
                .any(|s| kn.contains(s));

                // Only redact string leaves: prevents `secure_cookies: bool`
                // and similarly-named struct fields from being clobbered.
                if (secret && val.is_string()) || (kn == "url" && in_webhooks) {
                    *val = Value::String(MASK.into());
                } else if kn == "databaseurl" {
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

/// SQLite URLs have no credentials and pass through unchanged.
fn mask_db_url(s: &str) -> String {
    if let Some((scheme, rest)) = s.split_once("://") {
        if let Some((userinfo, hostpart)) = rest.split_once('@') {
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

#[cfg(test)]
mod tests {
    use super::{redact, MASK};
    use serde_json::json;

    fn redacted(v: serde_json::Value) -> serde_json::Value {
        let mut v = v;
        redact(&mut v, false);
        v
    }

    #[test]
    fn redacts_obvious_secret_fields() {
        let out = redacted(json!({
            "password": "hunter2",
            "token": "abc",
            "api_key": "xyz",
            "Authorization": "Bearer foo",
        }));
        assert_eq!(out["password"], MASK);
        assert_eq!(out["token"], MASK);
        assert_eq!(out["api_key"], MASK);
        assert_eq!(out["Authorization"], MASK);
    }

    #[test]
    fn redacts_custom_credential_headers() {
        let out = redacted(json!({
            "headers": {
                "X-Api-Key": "secret",
                "X-Auth-Token": "secret",
                "X_AUTH_KEY": "secret",
                "Cookie": "sid=abc",
                "Set-Cookie": "sid=abc",
            }
        }));
        let h = &out["headers"];
        assert_eq!(h["X-Api-Key"], MASK);
        assert_eq!(h["X-Auth-Token"], MASK);
        assert_eq!(h["X_AUTH_KEY"], MASK);
        assert_eq!(h["Cookie"], MASK);
        assert_eq!(h["Set-Cookie"], MASK);
    }

    #[test]
    fn does_not_clobber_bool_fields_with_credential_substrings() {
        // `secure_cookies` would match "cookie" by substring; the
        // `is_string()` guard keeps it intact.
        let out = redacted(json!({ "secure_cookies": true, "is_secret": false }));
        assert_eq!(out["secure_cookies"], true);
        assert_eq!(out["is_secret"], false);
    }

    #[test]
    fn webhook_url_masked_only_inside_webhooks() {
        let out = redacted(json!({
            "targets": { "plex": { "url": "http://plex:32400" } },
            "webhooks": { "discord": { "url": "https://discord.com/api/..." } },
        }));
        assert_eq!(out["targets"]["plex"]["url"], "http://plex:32400");
        assert_eq!(out["webhooks"]["discord"]["url"], MASK);
    }

    #[test]
    fn database_url_masks_credentials_only() {
        let out = redacted(json!({
            "database_url": "postgres://user:secret@localhost:5432/db",
        }));
        assert_eq!(
            out["database_url"],
            format!("postgres://user:{MASK}@localhost:5432/db")
        );
    }
}
