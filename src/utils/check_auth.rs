use actix_web_httpauth::extractors::basic::BasicAuth;

use super::settings::Settings;

pub fn check_auth(auth: &BasicAuth, settings: &Settings) -> bool {
    let username = settings.auth.username.clone();
    let password = settings.auth.password.clone();

    println!(
        "'{}'\n'{}'\n'{}'\n'{}'",
        username,
        password,
        auth.user_id(),
        auth.password().unwrap()
    );

    if auth.user_id() == username && auth.password().unwrap() == password {
        return true;
    }

    false
}
