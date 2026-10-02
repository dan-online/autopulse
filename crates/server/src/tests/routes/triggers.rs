use crate::routes::triggers::{trigger_get, trigger_post};
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
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

// `Rewrite::single` is `#[cfg(test)]`-gated inside autopulse-utils, so build
// the value via Deserialize to avoid touching the crate's private fields.
fn rewrite(from: &str, to: &str) -> Rewrite {
    serde_json::from_value(serde_json::json!({ "from": from, "to": to }))
        .expect("rewrite JSON should deserialize")
}

fn test_manager() -> PulseManager {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let unique_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let database_url =
        format!("sqlite:///tmp/autopulse-server-autoscan-rewrite-{pid}-{unique_id}-{seq}.db");

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
    settings.triggers.insert(
        "sportarr".to_string(),
        serde_json::from_value(serde_json::json!({
            "type": "sportarr",
            "rewrite": { "from": "/Sports", "to": "/media/sports" }
        }))
        .expect("sportarr trigger JSON should deserialize"),
    );

    for name in ["tdarr", "manual"] {
        settings.triggers.insert(
            name.to_string(),
            serde_json::from_value(serde_json::json!({
                "type": "tdarr",
                "rewrite": { "from": "^/tdarr", "to": "/media" },
                "filter": { "exclude": ["/excluded/"] }
            }))
            .expect("tdarr trigger JSON should deserialize"),
        );
    }

    let pool = get_pool(&database_url).expect("test database pool should initialize");
    get_conn(&pool)
        .expect("test database connection should initialize")
        .migrate()
        .expect("test database migrations should apply");

    PulseManager::new(settings, pool)
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

#[actix_web::test]
async fn manual_and_autoscan_query_contract() {
    let mut manager = test_manager();
    std::sync::Arc::make_mut(&mut manager.settings)
        .triggers
        .insert(
            "manual".to_string(),
            serde_json::from_value(serde_json::json!({ "type": "manual" })).unwrap(),
        );
    let app = test::init_service(
        App::new()
            .service(trigger_get)
            .app_data(basic::Config::default().realm("Restricted area"))
            .app_data(Data::new(manager.clone())),
    )
    .await;

    for (uri, path, hash) in [
        (
            "/triggers/manual?path=/media/movie.mkv&hash=1234567890",
            "/media/movie.mkv",
            Some("1234567890"),
        ),
        (
            "/triggers/a_train?dir=/downloads/Show/",
            "/media/Show/",
            None,
        ),
    ] {
        let response = test::call_service(
            &app,
            TestRequest::get()
                .uri(uri)
                .insert_header(("Authorization", test_auth_header()))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), actix_web::http::StatusCode::OK);
        let body: serde_json::Value = test::read_body_json(response).await;
        assert_eq!(body["file_path"], path);
        assert_eq!(body["file_hash"], serde_json::json!(hash));
        let stored = manager
            .get_event(body["id"].as_str().unwrap())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.file_path, path);
        assert_eq!(stored.file_hash.as_deref(), hash);
    }

    let response = test::call_service(
        &app,
        TestRequest::get()
            .uri("/triggers/manual?dir=/media/Show/")
            .insert_header(("Authorization", test_auth_header()))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), actix_web::http::StatusCode::BAD_REQUEST);
    assert_eq!(test::read_body(response).await, "Invalid query parameters");
}

#[actix_web::test]
async fn sportarr_trigger_parses_download_webhook() {
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
            .uri("/triggers/sportarr")
            .insert_header(("Authorization", test_auth_header()))
            .set_json(serde_json::json!({
                "eventType": "Download",
                "episodeFile": { "relativePath": "Season 2026/NFL.2026.08.04.mkv" },
                "series": {
                    "path": "/Sports/NFL"
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
    assert_eq!(events.len(), 1);

    let path = events[0]["file_path"]
        .as_str()
        .expect("file_path in response");
    assert_eq!(path, "/media/sports/NFL/Season 2026/NFL.2026.08.04.mkv");
}

// Live Sportarr Rename payload: the batch directory is `series.path`.
#[actix_web::test]
async fn sportarr_trigger_parses_real_rename_webhook() {
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
            .uri("/triggers/sportarr")
            .insert_header(("Authorization", test_auth_header()))
            .set_json(serde_json::json!({
                "eventType": "Rename",
                "title": "Renamed 1 file(s)",
                "message": "Scope: NFL",
                "applicationUrl": "",
                "instanceName": "Sportarr",
                "series": { "title": "", "path": "/Sports/NFL" },
                "renamedCount": 1
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
    assert_eq!(events.len(), 1);

    let path = events[0]["file_path"]
        .as_str()
        .expect("file_path in response");
    assert_eq!(path, "/media/sports/NFL");
}

// Live Sportarr SeriesDelete payload: `series.path` is absent.
#[actix_web::test]
async fn sportarr_trigger_parses_real_series_delete_webhook() {
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
            .uri("/triggers/sportarr")
            .insert_header(("Authorization", test_auth_header()))
            .set_json(serde_json::json!({
                "eventType": "SeriesDelete",
                "title": "Event deleted: NFL 2026-08-05 Team C vs Team D",
                "message": "The event was removed from the library.",
                "applicationUrl": "",
                "instanceName": "Sportarr",
                "series": { "id": 2, "title": "NFL 2026-08-05 Team C vs Team D" }
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
    assert_eq!(events.len(), 0);
}

#[actix_web::test]
async fn tdarr_trigger_parses_flow_container_change() {
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
            .uri("/triggers/tdarr")
            .insert_header(("Authorization", test_auth_header()))
            .set_json(serde_json::json!({
                "path": "/tdarr/Show/episode.mp4",
                "original_path": "/tdarr/Show/episode.mkv"
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
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["file_path"], "/media/Show/episode.mp4");
    assert_eq!(events[0]["found_status"], "not_found");
    assert_eq!(events[1]["file_path"], "/media/Show/episode.mkv");
    assert_eq!(events[1]["found_status"], "found");
}

#[actix_web::test]
async fn tdarr_manual_override_accepts_td01_dir_and_manual_path() {
    let manager = test_manager();
    let app = test::init_service(
        App::new()
            .service(trigger_post)
            .service(trigger_get)
            .app_data(basic::Config::default().realm("Restricted area"))
            .app_data(Data::new(manager)),
    )
    .await;

    let response = test::call_service(
        &app,
        TestRequest::post()
            .uri("/triggers/manual?dir=/tdarr/Show%20%26%20Co%20%23%20%2B/Season%201/")
            .insert_header(("Authorization", test_auth_header()))
            .insert_header(("Content-Type", "application/json"))
            .to_request(),
    )
    .await;

    assert!(
        response.status().is_success(),
        "status={}",
        response.status()
    );
    let body: serde_json::Value = test::read_body_json(response).await;
    assert_eq!(body["file_path"], "/media/Show & Co # +/Season 1/");
    assert_eq!(body["event_source"], "manual");
    assert_eq!(body["found_status"], "not_found");

    let response = test::call_service(
        &app,
        TestRequest::get()
            .uri("/triggers/manual?path=/tdarr/movie.mp4&hash=1234567890")
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
    assert_eq!(body["file_path"], "/media/movie.mp4");
    assert_eq!(body["file_hash"], "1234567890");
}

#[actix_web::test]
async fn tdarr_trigger_rejects_body_without_paths() {
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
            .uri("/triggers/tdarr")
            .insert_header(("Authorization", test_auth_header()))
            .set_json(serde_json::json!({}))
            .to_request(),
    )
    .await;

    assert_eq!(
        response.status(),
        actix_web::http::StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(test::read_body(response).await, "Unable to parse request");
}

#[actix_web::test]
async fn tdarr_trigger_filters_original_path_after_rewrite_and_query_paths() {
    let manager = test_manager();
    let app = test::init_service(
        App::new()
            .service(trigger_post)
            .service(trigger_get)
            .app_data(basic::Config::default().realm("Restricted area"))
            .app_data(Data::new(manager)),
    )
    .await;

    let response = test::call_service(
        &app,
        TestRequest::post()
            .uri("/triggers/tdarr")
            .insert_header(("Authorization", test_auth_header()))
            .set_json(serde_json::json!({
                "path": "/tdarr/movie.mp4",
                "original_path": "/tdarr/excluded/movie.mkv"
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
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["file_path"], "/media/movie.mp4");

    let response = test::call_service(
        &app,
        TestRequest::get()
            .uri("/triggers/tdarr?dir=/tdarr/excluded/")
            .insert_header(("Authorization", test_auth_header()))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), actix_web::http::StatusCode::NO_CONTENT);
}
