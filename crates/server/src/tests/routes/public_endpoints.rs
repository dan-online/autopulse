use crate::routes::{index::hello, stats::stats, status::status};
use actix_web::{
    http::StatusCode,
    test::{self, TestRequest},
    web::Data,
    App,
};
use actix_web_httpauth::extractors::basic;
use autopulse_database::conn::{get_conn, get_pool};
use autopulse_database::models::NewScanEvent;
use autopulse_service::{manager::PulseManager, settings::Settings};
use std::time::{SystemTime, UNIX_EPOCH};

fn test_manager() -> PulseManager {
    let unique_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let database_url = format!("sqlite:///tmp/autopulse-server-public-endpoints-{unique_id}.db");

    let mut settings = Settings::default();
    settings.app.database_url = database_url.clone();

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

fn insert_test_event(manager: &PulseManager) -> String {
    manager
        .add_event(&NewScanEvent::default())
        .expect("test scan event should insert")
        .id
}

#[actix_web::test]
async fn root_endpoint_is_public() {
    let manager = test_manager();
    let app = test::init_service(
        App::new()
            .service(hello)
            .app_data(basic::Config::default().realm("Restricted area"))
            .app_data(Data::new(manager)),
    )
    .await;

    let response = test::call_service(&app, TestRequest::get().uri("/").to_request()).await;

    assert_eq!(response.status(), StatusCode::OK);
}

#[actix_web::test]
async fn stats_endpoint_is_public() {
    let manager = test_manager();
    let app = test::init_service(
        App::new()
            .service(stats)
            .app_data(basic::Config::default().realm("Restricted area"))
            .app_data(Data::new(manager)),
    )
    .await;

    let response = test::call_service(&app, TestRequest::get().uri("/stats").to_request()).await;

    assert_eq!(response.status(), StatusCode::OK);
}

#[actix_web::test]
async fn status_endpoint_still_requires_auth() {
    let manager = test_manager();
    let app = test::init_service(
        App::new()
            .service(status)
            .app_data(basic::Config::default().realm("Restricted area"))
            .app_data(Data::new(manager)),
    )
    .await;

    let response =
        test::call_service(&app, TestRequest::get().uri("/status/test-id").to_request()).await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn status_endpoint_accepts_valid_basic_auth() {
    let manager = test_manager();
    let event_id = insert_test_event(&manager);
    let app = test::init_service(
        App::new()
            .service(status)
            .app_data(basic::Config::default().realm("Restricted area"))
            .app_data(Data::new(manager)),
    )
    .await;

    let response = test::call_service(
        &app,
        TestRequest::get()
            .uri(&format!("/status/{event_id}"))
            .insert_header(("Authorization", test_auth_header()))
            .to_request(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
}
