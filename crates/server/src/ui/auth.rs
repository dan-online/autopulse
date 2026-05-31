use actix_session::{Session, SessionExt};
use actix_web::{
    dev::Payload,
    error::{ErrorInternalServerError, InternalError},
    get, post,
    web::{Data, Form},
    Error, FromRequest, HttpRequest, HttpResponse, Responder, Result,
};
use autopulse_service::manager::PulseManager;
use maud::{html, Markup, DOCTYPE};
use serde::Deserialize;
use std::{
    collections::HashMap,
    future::{ready, Ready},
    net::IpAddr,
    sync::Mutex,
    time::{Duration, Instant},
};

use crate::ui::{csrf, layout::Ctx};

const SESSION_USER_KEY: &str = "user";

/// Failed logins allowed from one IP before the lockout window kicks in.
const LOGIN_MAX_ATTEMPTS: u32 = 5;
/// How long an IP stays locked out after exceeding the attempt limit,
/// and the idle window after which a stale counter is forgotten.
const LOGIN_LOCKOUT: Duration = Duration::from_secs(60);

/// Minimal in-memory per-IP failed-login throttle. Stored as actix
/// `app_data` so it is shared across workers. Not persisted: a restart
/// clears all counters, which is acceptable for a brute-force speed bump.
#[derive(Default)]
pub struct LoginLimiter {
    inner: Mutex<HashMap<IpAddr, Attempts>>,
}

struct Attempts {
    count: u32,
    last: Instant,
}

impl LoginLimiter {
    /// Returns `true` if the IP is currently locked out. Also lazily
    /// forgets counters that have been idle past the lockout window.
    fn is_locked(&self, ip: IpAddr) -> bool {
        let mut map = self.inner.lock().unwrap();
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
        let mut map = self.inner.lock().unwrap();
        let entry = map.entry(ip).or_insert(Attempts {
            count: 0,
            last: Instant::now(),
        });
        entry.count += 1;
        entry.last = Instant::now();
    }

    fn reset(&self, ip: IpAddr) {
        self.inner.lock().unwrap().remove(&ip);
    }
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
        let logged_in = matches!(session.get::<String>(SESSION_USER_KEY), Ok(Some(_)));

        if logged_in {
            ready(Ok(Self))
        } else {
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

    // Maybe will match the proxy IP, but I don't wanna import a dependency for this
    let ip = req.peer_addr().map(|addr| addr.ip());

    if let Some(ip) = ip {
        if auth.enabled && limiter.is_locked(ip) {
            return Ok(HttpResponse::TooManyRequests()
                .insert_header(("Retry-After", LOGIN_LOCKOUT.as_secs().to_string()))
                .body("too many login attempts, try again later"));
        }
    }

    // Constant-time credential compare to avoid leaking match progress
    // via timing. When auth is disabled every UI route is open; the
    // login page still works as a no-op.
    let credentials_ok = csrf::validate_eq(&form.username, &auth.username)
        && csrf::validate_eq(&form.password, &auth.password);

    if !auth.enabled || credentials_ok {
        if let Some(ip) = ip {
            limiter.reset(ip);
        }
        session
            .insert(SESSION_USER_KEY, form.username.clone())
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

    // Validate CSRF from the form field (logout is a plain HTML form,
    // not an HTMX request - no headers available).
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
