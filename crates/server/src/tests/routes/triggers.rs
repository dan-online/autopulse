use crate::routes::triggers::trigger_get;
use actix_web::{
    test::{self, TestRequest},
    web::Data,
    App,
};
use actix_web_httpauth::extractors::basic;
use autopulse_database::conn::{get_conn, get_pool};
use autopulse_service::manager::PulseManager;
use autopulse_service::settings::triggers::autoscan::Autoscan;
use autopulse_service::settings::triggers::Trigger;
use autopulse_service::settings::Settings;
use autopulse_utils::Rewrite;
use std::time::{SystemTime, UNIX_EPOCH};

// `Rewrite::single` is `#[cfg(test)]`-gated inside autopulse-utils, so build
// the value via Deserialize to avoid touching the crate's private fields.
fn rewrite(from: &str, to: &str) -> Rewrite {
    serde_json::from_value(serde_json::json!({ "from": from, "to": to }))
        .expect("rewrite JSON should deserialize")
}

fn test_manager() -> PulseManager {
    let unique_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let database_url = format!("sqlite:///tmp/autopulse-server-autoscan-rewrite-{unique_id}.db");

    let mut settings = Settings::default();
    settings.app.database_url = database_url.clone();
    settings.triggers.insert(
        "a_train".to_string(),
        Trigger::Autoscan(Autoscan {
            rewrite: Some(rewrite("/downloads", "/media")),
            ..Default::default()
        }),
    );

    let pool = get_pool(&database_url).expect("test database pool should initialize");
    get_conn(&pool)
        .expect("test database connection should initialize")
        .migrate()
        .expect("test database migrations should apply");

    PulseManager::new(settings, pool)
}

fn test_auth_header() -> String {
    Settings::default().auth.to_auth_encoded()
}

#[actix_web::test]
async fn autoscan_trigger_applies_rewrite_to_dir() {
    let manager = test_manager();
    let app = test::init_service(
        App::new()
            .service(trigger_get)
            .app_data(basic::Config::default().realm("Restricted area"))
            .app_data(Data::new(manager)),
    )
    .await;

    let response = test::call_service(
        &app,
        TestRequest::get()
            .uri("/triggers/a_train?dir=/downloads/show/episode.mkv")
            .insert_header(("Authorization", test_auth_header()))
            .to_request(),
    )
    .await;

    assert!(
        response.status().is_success(),
        "status={}",
        response.status()
    );

    let body: serde_json::Value = test::read_body_json(response).await;
    let path = body["file_path"].as_str().expect("file_path in response");
    assert_eq!(path, "/media/show/episode.mkv", "rewrite must be applied");
}
