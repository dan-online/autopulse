use actix_web::{
    dev::Payload, error::ErrorUnauthorized, web::Data, Error, FromRequest, HttpRequest,
};
use actix_web_httpauth::extractors::basic::BasicAuth;
use autopulse_service::manager::PulseManager;
use std::{future::Future, pin::Pin};

pub struct AuthenticatedUser;

impl FromRequest for AuthenticatedUser {
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        let manager = req.app_data::<Data<PulseManager>>().cloned();

        if !manager.as_ref().is_some_and(|m| m.settings.auth.enabled) {
            return Box::pin(async { Ok(Self) });
        }

        // We know manager is Some because the guard above passed
        let auth_cfg = &manager.as_ref().unwrap().settings.auth;
        let username = auth_cfg.username.clone();
        let password = auth_cfg.password.clone();

        let fut = BasicAuth::from_request(req, payload);

        Box::pin(async move {
            let credentials = fut.await.ok();

            if let Some(ref creds) = credentials {
                if creds.user_id() == username && creds.password().unwrap_or("") == password {
                    return Ok(Self);
                }
            }

            Err(ErrorUnauthorized("Unauthorized"))
        })
    }
}

#[cfg(test)]
pub fn check_auth(
    auth: &Option<BasicAuth>,
    enabled: &bool,
    username: &String,
    password: &String,
) -> bool {
    if !enabled {
        return true;
    }

    if auth.is_none() {
        return false;
    }

    let auth = auth.as_ref().unwrap();

    if auth.user_id() == username && auth.password().unwrap_or("") == password {
        return true;
    }

    false
}
