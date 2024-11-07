use crate::settings::Settings;
use actix_web_httpauth::extractors::basic::BasicAuth;

pub fn check_auth(auth: &Option<BasicAuth>, settings: &Settings) -> bool {
    if !settings.auth.enabled {
        return true;
    }

    if auth.is_none() {
        return false;
    }

    let auth = auth.as_ref().unwrap();

    let username = settings.auth.username.clone();
    let password = settings.auth.password.clone();

    if auth.user_id() == username && auth.password().unwrap_or("") == password {
        return true;
    }

    false
}
