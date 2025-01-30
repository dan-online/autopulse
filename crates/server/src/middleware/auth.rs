use actix_web_httpauth::extractors::basic::BasicAuth;

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
