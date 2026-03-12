use actix_web::{
    dev::Payload, error::InternalError, web::Data, Error, FromRequest, HttpRequest, HttpResponse,
};
use actix_web_httpauth::extractors::basic::BasicAuth;
use autopulse_service::manager::PulseManager;
use std::{future::Future, pin::Pin};

pub struct AuthenticatedUser;

impl FromRequest for AuthenticatedUser {
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        let manager = match req.app_data::<Data<PulseManager>>().cloned() {
            Some(m) => m,
            // Fail closed: missing PulseManager is a server misconfiguration, not a bypass
            None => {
                return Box::pin(async {
                    Err(InternalError::from_response(
                        "Server misconfigured",
                        HttpResponse::InternalServerError()
                            .json("Server misconfigured: missing application data"),
                    )
                    .into())
                });
            }
        };

        if !manager.settings.auth.enabled {
            return Box::pin(async { Ok(Self) });
        }

        let username = manager.settings.auth.username.clone();
        let password = manager.settings.auth.password.clone();

        let fut = BasicAuth::from_request(req, payload);

        Box::pin(async move {
            let credentials = fut.await.ok();

            if let Some(ref creds) = credentials {
                if creds.user_id() == username && creds.password().unwrap_or("") == password {
                    return Ok(Self);
                }
            }

            Err(
                InternalError::from_response(
                    "Authentication required",
                    HttpResponse::Unauthorized().json("Authentication required"),
                )
                .into(),
            )
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
