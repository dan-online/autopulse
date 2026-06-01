use actix_session::{Session, SessionExt};
use actix_web::{
    dev::Payload,
    error::{ErrorInternalServerError, InternalError},
    get, post,
    web::{Data, Form},
    Error, FromRequest, HttpRequest, HttpResponse, Responder, Result,
};
use autopulse_service::{manager::PulseManager, settings::auth::Auth};
use maud::{html, Markup, DOCTYPE};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    future::{ready, Ready},
    net::IpAddr,
    sync::Mutex,
    time::{Duration, Instant},
};

use crate::ui::{csrf, layout::Ctx};

const SESSION_USER_KEY: &str = "user";
/// Fingerprint of the credentials the session was issued under; any change
/// to `auth.username` / `auth.password` rotates this and invalidates the session.
const SESSION_AUTH_FP_KEY: &str = "auth_fp";

/// NUL separator prevents `("ab","c")` colliding with `("a","bc")`.
fn cred_fingerprint(auth: &Auth) -> String {
    let mut h = Sha256::new();
    h.update(auth.username.as_bytes());
    h.update(b"\0");
    h.update(auth.password.as_bytes());
    base16ct::lower::encode_string(&h.finalize())
}

const LOGIN_MAX_ATTEMPTS: u32 = 5;
/// How long an IP stays locked out after exceeding the attempt limit,
/// and the idle window after which a stale counter is forgotten.
const LOGIN_LOCKOUT: Duration = Duration::from_secs(60);
/// Hard cap so a flood of distinct IPs can't grow the throttle map without
/// bound; LRU eviction once full. O(n) scan per failure stays cheap at 10k.
const LOGIN_TRACKED_IPS_CAP: usize = 10_000;

/// Per-IP failed-login throttle. Wrap in `Data::new` outside the
/// `HttpServer` factory so the Arc is shared across workers.
#[derive(Default)]
pub struct LoginLimiter {
    inner: Mutex<HashMap<IpAddr, Attempts>>,
}

struct Attempts {
    count: u32,
    last: Instant,
}

impl LoginLimiter {
    /// Recover from poisoning instead of 500ing every subsequent login.
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<IpAddr, Attempts>> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn is_locked(&self, ip: IpAddr) -> bool {
        let mut map = self.lock();
        match map.get(&ip) {
            Some(a) if a.count >= LOGIN_MAX_ATTEMPTS && a.last.elapsed() < LOGIN_LOCKOUT => true,
            Some(a) if a.last.elapsed() >= LOGIN_LOCKOUT => {
                map.remove(&ip);
                false
            }
            _ => false,
        }
    }

    fn record_failure(&self, ip: IpAddr) {
        let mut map = self.lock();
        map.retain(|_, a| a.last.elapsed() < LOGIN_LOCKOUT);
        if !map.contains_key(&ip) && map.len() >= LOGIN_TRACKED_IPS_CAP {
            if let Some(oldest) = map.iter().min_by_key(|(_, a)| a.last).map(|(k, _)| *k) {
                map.remove(&oldest);
            }
        }
        let entry = map.entry(ip).or_insert(Attempts {
            count: 0,
            last: Instant::now(),
        });
        entry.count += 1;
        entry.last = Instant::now();
    }

    fn reset(&self, ip: IpAddr) {
        self.lock().remove(&ip);
    }
}

/// `peer_addr` unless it's a trusted proxy, in which case the rightmost
/// untrusted XFF entry. Empty trust list = `peer_addr` always.
fn client_ip(req: &HttpRequest, trusted: &[IpAddr]) -> Option<IpAddr> {
    let peer = req.peer_addr().map(|a| a.ip())?;
    if trusted.is_empty() || !trusted.contains(&peer) {
        return Some(peer);
    }
    if let Some(xff) = req
        .headers()
        .get("X-Forwarded-For")
        .and_then(|h| h.to_str().ok())
    {
        for raw in xff.split(',').rev() {
            if let Ok(ip) = raw.trim().parse::<IpAddr>() {
                if !trusted.contains(&ip) {
                    return Some(ip);
                }
            }
        }
    }
    Some(peer)
}

/// On miss, returns 303 → `/ui/login` instead of the JSON 401 that
/// `AuthenticatedUser` produces for API routes.
pub struct SessionUser;

impl FromRequest for SessionUser {
    type Error = Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let manager = match req.app_data::<Data<PulseManager>>() {
            Some(m) => m,
            None => {
                return ready(Err(ErrorInternalServerError("missing PulseManager")));
            }
        };

        if !manager.settings.auth.enabled {
            return ready(Ok(Self));
        }

        let session = req.get_session();
        let has_user = matches!(session.get::<String>(SESSION_USER_KEY), Ok(Some(_)));
        let stored_fp = session.get::<String>(SESSION_AUTH_FP_KEY).ok().flatten();
        let current_fp = cred_fingerprint(&manager.settings.auth);
        let fp_ok = stored_fp
            .as_deref()
            .is_some_and(|fp| csrf::validate_eq(fp, &current_fp));

        if has_user && fp_ok {
            ready(Ok(Self))
        } else {
            session.purge();
            let base = manager.settings.app.base_path.clone();
            ready(Err(InternalError::from_response(
                "login required",
                HttpResponse::SeeOther()
                    .insert_header(("Location", format!("{base}/ui/login")))
                    .finish(),
            )
            .into()))
        }
    }
}

#[derive(Deserialize)]
pub struct LoginQuery {
    pub error: Option<String>,
}

fn login_page_markup(base: &str, error: Option<&str>) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "autopulse · sign in" }
                link rel="icon" type="image/webp" href={ (base) "/ui/static/logo.webp" };
                link rel="stylesheet" href={ (base) "/ui/static/app.css?v=" (crate::ui::layout::ASSETS_V) };
            }
            body.login {
                form.login__form method="post" action={ (base) "/ui/login" } {
                    img.login__logo src={ (base) "/ui/static/logo.webp" } alt="autopulse";
                    h1 { "autopulse" }
                    @if let Some(e) = error {
                        p.login__error {
                            @match e {
                                "invalid" => "Invalid username or password",
                                "locked" => "Too many attempts — try again later",
                                _ => "Login failed",
                            }
                        }
                    }
                    label { "Username"
                        input name="username" required autofocus autocomplete="username";
                    }
                    label { "Password"
                        input name="password" type="password" required autocomplete="current-password";
                    }
                    button type="submit" { "Sign in" }
                }
            }
        }
    }
}

#[get("/ui/login")]
pub async fn login_page(
    manager: Data<PulseManager>,
    query: actix_web::web::Query<LoginQuery>,
) -> impl Responder {
    let base = &manager.settings.app.base_path;
    login_page_markup(base, query.error.as_deref())
}

#[derive(Deserialize)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

#[post("/ui/login")]
pub async fn login_post(
    req: HttpRequest,
    manager: Data<PulseManager>,
    limiter: Data<LoginLimiter>,
    session: Session,
    form: Form<LoginForm>,
) -> Result<HttpResponse> {
    let base = manager.settings.app.base_path.clone();
    let auth = &manager.settings.auth;

    let ip = client_ip(&req, &manager.settings.app.trusted_proxies);

    if let Some(ip) = ip {
        if auth.enabled && limiter.is_locked(ip) {
            return Ok(HttpResponse::TooManyRequests()
                .insert_header(("Retry-After", LOGIN_LOCKOUT.as_secs().to_string()))
                .body("too many login attempts, try again later"));
        }
    }

    // `&` not `&&`: short-circuiting on a wrong username would leak match
    // progress via latency, undoing `validate_eq`'s constant-time work.
    let credentials_ok = csrf::validate_eq(&form.username, &auth.username)
        & csrf::validate_eq(&form.password, &auth.password);

    if !auth.enabled || credentials_ok {
        if let Some(ip) = ip {
            limiter.reset(ip);
        }
        session
            .insert(SESSION_USER_KEY, form.username.clone())
            .map_err(ErrorInternalServerError)?;
        session
            .insert(SESSION_AUTH_FP_KEY, cred_fingerprint(auth))
            .map_err(ErrorInternalServerError)?;
        session
            .insert(csrf::SESSION_KEY, csrf::fresh_token()?)
            .map_err(ErrorInternalServerError)?;

        return Ok(HttpResponse::SeeOther()
            .insert_header(("Location", format!("{base}/ui/events")))
            .finish());
    }

    if let Some(ip) = ip {
        limiter.record_failure(ip);
    }

    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("{base}/ui/login?error=invalid")))
        .finish())
}

#[derive(Deserialize)]
pub struct LogoutForm {
    pub csrf: String,
}

#[post("/ui/logout")]
pub async fn logout_post(
    manager: Data<PulseManager>,
    session: Session,
    form: Form<LogoutForm>,
) -> Result<HttpResponse> {
    let base = manager.settings.app.base_path.clone();

    if manager.settings.auth.enabled {
        let stored = session
            .get::<String>(csrf::SESSION_KEY)
            .map_err(ErrorInternalServerError)?
            .unwrap_or_default();

        if !csrf::validate_eq(&form.csrf, &stored) {
            return Ok(HttpResponse::Forbidden().body("CSRF token invalid"));
        }
    }

    session.purge();

    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("{base}/ui/login")))
        .finish())
}

pub fn ctx<'a>(manager: &'a PulseManager, csrf_token: &'a str) -> Ctx<'a> {
    Ctx {
        base: manager.settings.app.base_path.as_str(),
        csrf: csrf_token,
    }
}

#[cfg(test)]
mod tests {
    use super::client_ip;
    use actix_web::test::TestRequest;
    use std::net::IpAddr;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn empty_trusted_list_returns_peer_addr() {
        let req = TestRequest::default()
            .peer_addr("203.0.113.7:1234".parse().unwrap())
            .insert_header(("X-Forwarded-For", "10.0.0.5"))
            .to_http_request();
        assert_eq!(client_ip(&req, &[]), Some(ip("203.0.113.7")));
    }

    #[test]
    fn untrusted_peer_returns_peer_addr() {
        let req = TestRequest::default()
            .peer_addr("203.0.113.7:1234".parse().unwrap())
            .insert_header(("X-Forwarded-For", "10.0.0.5, 198.51.100.1"))
            .to_http_request();
        let trusted = [ip("10.0.0.1")];
        assert_eq!(client_ip(&req, &trusted), Some(ip("203.0.113.7")));
    }

    #[test]
    fn trusted_peer_takes_rightmost_untrusted_from_xff() {
        let req = TestRequest::default()
            .peer_addr("10.0.0.1:443".parse().unwrap())
            .insert_header(("X-Forwarded-For", "203.0.113.7, 10.0.0.5"))
            .to_http_request();
        let trusted = [ip("10.0.0.1"), ip("10.0.0.5")];
        assert_eq!(client_ip(&req, &trusted), Some(ip("203.0.113.7")));
    }

    #[test]
    fn trusted_peer_with_no_xff_falls_back_to_peer() {
        let req = TestRequest::default()
            .peer_addr("10.0.0.1:443".parse().unwrap())
            .to_http_request();
        let trusted = [ip("10.0.0.1")];
        assert_eq!(client_ip(&req, &trusted), Some(ip("10.0.0.1")));
    }

    #[test]
    fn malformed_xff_entries_are_skipped() {
        let req = TestRequest::default()
            .peer_addr("10.0.0.1:443".parse().unwrap())
            .insert_header(("X-Forwarded-For", "not-an-ip, 203.0.113.7, also-bogus"))
            .to_http_request();
        let trusted = [ip("10.0.0.1")];
        assert_eq!(client_ip(&req, &trusted), Some(ip("203.0.113.7")));
    }

    #[test]
    fn no_peer_addr_returns_none() {
        let req = TestRequest::default().to_http_request();
        assert_eq!(client_ip(&req, &[]), None);
    }
}
