use actix_web_httpauth::extractors::basic::BasicAuth;

use super::settings::Settings;

pub fn check_auth(auth: &BasicAuth, settings: &Settings) -> bool {
    let username = settings.username.clone();
    let password = settings.password.clone();

    if auth.user_id() == username && auth.password().unwrap() == password {
        return true;
    }

    false
}
