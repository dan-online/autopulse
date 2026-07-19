use crate::routes::triggers::{trigger_get, trigger_post, trigger_post_rest};
use actix_web::{
    http::StatusCode,
    test::{self, TestRequest},
    web::Data,
    App,
};
use actix_web_httpauth::extractors::basic;
use autopulse_database::conn::{get_conn, get_pool};
use autopulse_service::manager::PulseManager;
use autopulse_service::settings::triggers::a_train::ATrain;
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

fn unique_db_url(label: &str) -> String {
    let unique_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    format!("sqlite:///tmp/autopulse-server-{label}-{unique_id}.db")
}

fn manager_with(trigger_name: &str, trigger: Trigger, label: &str) -> PulseManager {
    let database_url = unique_db_url(label);

    let mut settings = Settings::default();
    settings.app.database_url = database_url.clone();
    settings.triggers.insert(trigger_name.to_string(), trigger);

    let pool = get_pool(&database_url).expect("test database pool should initialize");
    get_conn(&pool)
        .expect("test database connection should initialize")
        .migrate()
        .expect("test database migrations should apply");

    PulseManager::new(settings, pool)
}

/// Manager preconfigured for the autoscan + sonarr-filter regression tests
/// that already lived in this file before A-Train support landed.
fn test_manager() -> PulseManager {
    let database_url = unique_db_url("autoscan-rewrite");

    let mut settings = Settings::default();
    settings.app.database_url = database_url.clone();
    settings.triggers.insert(
        "a_train".to_string(),
        Trigger::Autoscan(Autoscan {
            rewrite: Some(rewrite("/downloads", "/media")),
            ..Default::default()
        }),
    );
    settings.triggers.insert(
        "sonarr".to_string(),
        serde_json::from_value(serde_json::json!({
            "type": "sonarr",
            "rewrite": { "from": "/TV", "to": "/media/tv" },
            "filter": { "exclude": ["S01E02"] }
        }))
        .expect("sonarr trigger JSON should deserialize"),
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

fn atrain_manager(label: &str) -> PulseManager {
    manager_with(
        "a-train",
        Trigger::Atrain(ATrain {
            rewrite: Some(rewrite("/mnt/gdrive", "/media")),
            ..Default::default()
        }),
        label,
    )
}

#[actix_web::test]
async fn webhook_trigger_returns_only_queued_paths_after_filtering() {
    let manager = test_manager();
    let app = test::init_service(
        App::new()
            .service(trigger_post)
            .app_data(basic::Config::default().realm("Restricted area"))
            .app_data(Data::new(manager)),
    )
    .await;

    let response = test::call_service(
        &app,
        TestRequest::post()
            .uri("/triggers/sonarr")
            .insert_header(("Authorization", test_auth_header()))
            .set_json(serde_json::json!({
                "eventType": "Download",
                "episodeFiles": [
                    { "relativePath": "Season 1/Westworld.S01E01.mkv" },
                    { "relativePath": "Season 1/Westworld.S01E02.mkv" }
                ],
                "series": {
                    "path": "/TV/Westworld"
                }
            }))
            .to_request(),
    )
    .await;

    assert!(
        response.status().is_success(),
        "status={}",
        response.status()
    );

    let body: serde_json::Value = test::read_body_json(response).await;
    let events = body.as_array().expect("response should be an array");
    assert_eq!(events.len(), 1, "filtered path should not be queued");

    let path = events[0]["file_path"]
        .as_str()
        .expect("file_path in response");
    assert_eq!(path, "/media/tv/Westworld/Season 1/Westworld.S01E01.mkv");
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

#[actix_web::test]
async fn autoscan_trigger_rejects_json_post_body() {
    let manager = test_manager();
    let app = test::init_service(
        App::new()
            .service(trigger_post)
            .app_data(basic::Config::default().realm("Restricted area"))
            .app_data(Data::new(manager)),
    )
    .await;

    let response = test::call_service(
        &app,
        TestRequest::post()
            .uri("/triggers/a_train")
            .insert_header(("Authorization", test_auth_header()))
            .set_json(serde_json::json!({ "dir": "/downloads/show" }))
            .to_request(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[actix_web::test]
async fn atrain_trigger_accepts_post_with_trailing_drive_id() {
    // A-Train hardcodes `/triggers/a-train/{drive_id}` as the POST URL and
    // sends `{ "created": [...], "deleted": [...] }` as the body. Verify
    // the trailing-segment route reaches the atrain trigger and that the
    // configured rewrite is applied to each path.
    let manager = atrain_manager("atrain-post");

    let app = test::init_service(
        App::new()
            .service(trigger_post_rest)
            .app_data(basic::Config::default().realm("Restricted area"))
            .app_data(Data::new(manager)),
    )
    .await;

    let response = test::call_service(
        &app,
        TestRequest::post()
            .uri("/triggers/a-train/0A1xxxxxxxxxUk9PVA")
            .insert_header(("Authorization", test_auth_header()))
            .set_json(serde_json::json!({
                "created": ["/mnt/gdrive/Movies/Interstellar (2014)"],
                "deleted": ["/mnt/gdrive/Movies/Mortal Kombat (2021)"],
            }))
            .to_request(),
    )
    .await;

    assert!(
        response.status().is_success(),
        "status={}",
        response.status()
    );

    let body: serde_json::Value = test::read_body_json(response).await;
    let events = body.as_array().expect("response should be a JSON array");
    assert_eq!(events.len(), 2, "one event per created+deleted path");

    let paths: Vec<&str> = events
        .iter()
        .map(|ev| ev["file_path"].as_str().expect("file_path in response"))
        .collect();
    assert!(
        paths.contains(&"/media/Movies/Interstellar (2014)"),
        "rewrite must be applied to created path; got {paths:?}"
    );
    assert!(
        paths.contains(&"/media/Movies/Mortal Kombat (2021)"),
        "rewrite must be applied to deleted path; got {paths:?}"
    );
}

#[actix_web::test]
async fn atrain_trailing_route_requires_auth() {
    let manager = atrain_manager("atrain-auth");
    let app = test::init_service(
        App::new()
            .service(trigger_post_rest)
            .app_data(basic::Config::default().realm("Restricted area"))
            .app_data(Data::new(manager)),
    )
    .await;

    let response = test::call_service(
        &app,
        TestRequest::post()
            .uri("/triggers/a-train/0A1xxxxxxxxxUk9PVA")
            .set_json(serde_json::json!({
                "created": ["/mnt/gdrive/Movies/Interstellar (2014)"],
                "deleted": [],
            }))
            .to_request(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn atrain_trailing_route_rejects_malformed_json() {
    let manager = atrain_manager("atrain-malformed-json");
    let app = test::init_service(
        App::new()
            .service(trigger_post_rest)
            .app_data(basic::Config::default().realm("Restricted area"))
            .app_data(Data::new(manager)),
    )
    .await;

    let response = test::call_service(
        &app,
        TestRequest::post()
            .uri("/triggers/a-train/0A1xxxxxxxxxUk9PVA")
            .insert_header(("Authorization", test_auth_header()))
            .insert_header(("Content-Type", "application/json"))
            .set_payload(r#"{ "created": ["/mnt/gdrive/Movies"], "deleted": "#)
            .to_request(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[actix_web::test]
async fn atrain_literal_payload_queues_events() {
    let manager = atrain_manager("atrain-literal");
    let app = test::init_service(
        App::new()
            .service(trigger_post_rest)
            .app_data(basic::Config::default().realm("Restricted area"))
            .app_data(Data::new(manager)),
    )
    .await;
    let payload = r#"{
  "created": [
    "/mnt/gdrive/Movies/Interstellar (2014)",
    "/mnt/gdrive/TV/Legion/Season 1"
  ],
  "deleted": [
    "/mnt/gdrive/Movies/Mortal Kombat (2021)"
  ]
}"#;

    let response = test::call_service(
        &app,
        TestRequest::post()
            .uri("/triggers/a-train/0A1xxxxxxxxxUk9PVA")
            .insert_header(("Authorization", test_auth_header()))
            .insert_header(("Content-Type", "application/json"))
            .set_payload(payload)
            .to_request(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = test::read_body_json(response).await;
    let events = body.as_array().expect("response should be a JSON array");
    assert_eq!(
        events.len(),
        3,
        "payload has two created and one deleted path"
    );

    let paths: Vec<&str> = events
        .iter()
        .map(|ev| ev["file_path"].as_str().expect("file_path in response"))
        .collect();
    assert!(paths.contains(&"/media/Movies/Interstellar (2014)"));
    assert!(paths.contains(&"/media/TV/Legion/Season 1"));
    assert!(paths.contains(&"/media/Movies/Mortal Kombat (2021)"));
}

#[actix_web::test]
async fn trailing_segment_route_rejects_non_atrain_triggers() {
    // The 2-segment POST route exists only for A-Train. A POST that lands on
    // it for a sonarr (or any non-atrain) trigger must 404 rather than be
    // silently accepted as a 1-segment sonarr POST.
    let manager = manager_with(
        "sonarr",
        serde_json::from_value::<Trigger>(serde_json::json!({ "type": "sonarr" }))
            .expect("sonarr trigger JSON should deserialize"),
        "rest-guard-sonarr",
    );

    let app = test::init_service(
        App::new()
            .service(trigger_post_rest)
            .app_data(basic::Config::default().realm("Restricted area"))
            .app_data(Data::new(manager)),
    )
    .await;

    let response = test::call_service(
        &app,
        TestRequest::post()
            .uri("/triggers/sonarr/garbage")
            .insert_header(("Authorization", test_auth_header()))
            .set_json(serde_json::json!({ "eventType": "Test" }))
            .to_request(),
    )
    .await;

    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "non-atrain triggers must not honor the trailing-segment route"
    );
}
