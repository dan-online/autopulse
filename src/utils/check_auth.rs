use super::settings::Settings;
use actix_web_httpauth::extractors::basic::BasicAuth;

pub fn check_auth(auth: &BasicAuth, settings: &Settings) -> bool {
    let username = settings.auth.username.clone();
    let password = settings.auth.password.clone();

    if auth.user_id() == username && auth.password().unwrap_or("") == password {
        return true;
    }

    false
}
