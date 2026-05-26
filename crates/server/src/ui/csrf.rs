use actix_session::SessionExt;
use actix_web::{
    dev::Payload, error::ErrorForbidden, http::Method, web::Data, Error, FromRequest, HttpRequest,
};
use autopulse_service::manager::PulseManager;
use std::future::{ready, Ready};

pub const SESSION_KEY: &str = "csrf";

pub const HEADER_NAME: &str = "X-CSRF-Token";

pub const FORM_FIELD: &str = "csrf";

pub fn fresh_token() -> Result<String, Error> {
    let mut bytes = [0u8; 32];

    getrandom::fill(&mut bytes)
        .map_err(|e| actix_web::error::ErrorInternalServerError(format!("OS RNG failed: {e}")))?;

    Ok(base16ct::lower::encode_string(&bytes))
}

/// Read-only extractor; handlers validate via header or form field using
/// `validate_eq`. Split so GET handlers can render the token and POST
/// handlers can validate it.
pub struct CsrfToken(pub String);

impl FromRequest for CsrfToken {
    type Error = Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let manager = match req.app_data::<Data<PulseManager>>() {
            Some(m) => m,
            None => {
                return ready(Err(actix_web::error::ErrorInternalServerError(
                    "missing PulseManager",
                )));
            }
        };

        // Auth disabled in config → no CSRF either.
        if !manager.settings.auth.enabled {
            return ready(Ok(CsrfToken(String::new())));
        }

        let session = req.get_session();

        match session.get::<String>(SESSION_KEY) {
            Ok(Some(t)) => ready(Ok(CsrfToken(t))),
            Ok(None) => {
                // Lazy-init: session exists (user logged in before CSRF
                // was deployed) but has no token yet. Generate one so
                // GET pages render and POST validation works.
                match fresh_token() {
                    Ok(t) => match session.insert(SESSION_KEY, t.clone()) {
                        Ok(()) => ready(Ok(CsrfToken(t))),
                        Err(e) => ready(Err(actix_web::error::ErrorInternalServerError(format!(
                            "failed to persist CSRF token: {e}"
                        )))),
                    },
                    Err(e) => ready(Err(e)),
                }
            }
            Err(_) => ready(Err(ErrorForbidden("session error — log in again"))),
        }
    }
}

/// Validates `X-CSRF-Token` header against session token. 403 on mismatch.
pub fn require_header(req: &HttpRequest, stored: &CsrfToken) -> Result<(), Error> {
    // Auth-disabled → nothing to validate.
    if stored.0.is_empty() {
        return Ok(());
    }
    // Non-mutating methods never need to validate.
    if matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS) {
        return Ok(());
    }
    let presented = req
        .headers()
        .get(HEADER_NAME)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    if validate_eq(presented, &stored.0) {
        Ok(())
    } else {
        Err(ErrorForbidden("CSRF token missing or invalid"))
    }
}

pub fn validate_eq(presented: &str, stored: &str) -> bool {
    let a = presented.as_bytes();
    let b = stored.as_bytes();

    if a.len() != b.len() {
        return false;
    }

    let mut acc: u8 = 0;

    for (x, y) in a.iter().zip(b.iter()) {
        acc |= x ^ y;
    }

    acc == 0
}
