use crate::settings::targets::{plex::Plex, TargetProcess};
use autopulse_database::models::{FoundStatus, ProcessStatus, ScanEvent};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const LIBRARIES: &str = "GET /plex/library/sections";
const SCAN: &str = "GET /plex/library/sections/1/refresh?path=%2Fmedia%2FShow";
const TRASH: &str = "PUT /plex/library/sections/1/emptyTrash";

fn libraries(refreshing: Value) -> Value {
    json!({"MediaContainer": {"Directory": [
        {"title": "TV", "key": "1", "refreshing": refreshing, "scannedAt": 100,
         "Location": [{"path": "/media"}]},
        {"title": "Movies", "key": "2", "refreshing": false, "scannedAt": 100,
         "Location": [{"path": "/other"}]}
    ]}})
}

fn scanned_libraries() -> Value {
    let mut result = libraries(json!(false));
    for library in result["MediaContainer"]["Directory"]
        .as_array_mut()
        .unwrap()
    {
        library["scannedAt"] = json!(101);
    }
    result
}

fn event(id: &str) -> ScanEvent {
    let now = chrono::Utc::now().naive_utc();
    ScanEvent {
        id: id.to_string(),
        event_source: "sonarr".to_string(),
        event_timestamp: now,
        file_path: format!("/downloads/Show/{id}.mkv"),
        file_hash: None,
        process_status: ProcessStatus::Pending.into(),
        found_status: FoundStatus::Found.into(),
        failed_times: 0,
        next_retry_at: None,
        targets_hit: String::new(),
        found_at: Some(now),
        processed_at: None,
        created_at: now,
        updated_at: now,
        can_process: now,
    }
}

async fn process(
    empty_trash: Option<bool>,
    events: &[&ScanEvent],
    responses: Vec<(&'static str, u16, Value)>,
) -> Vec<String> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/plex/", listener.local_addr().unwrap());
    let expected = responses
        .iter()
        .map(|(r, _, _)| r.to_string())
        .collect::<Vec<_>>();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let received = Arc::clone(&requests);
    let server = tokio::spawn(async move {
        for (_, status, body) in responses {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buffer = Vec::new();
            loop {
                let mut chunk = [0; 1024];
                let count = stream.read(&mut chunk).await.unwrap();
                assert_ne!(count, 0);
                buffer.extend_from_slice(&chunk[..count]);
                if buffer.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            let request = String::from_utf8(buffer).unwrap();
            assert!(request
                .to_ascii_lowercase()
                .contains("x-plex-token: test-token\r\n"));
            assert!(request.to_ascii_lowercase().contains("x-test: custom\r\n"));
            received.lock().unwrap().push(
                request
                    .lines()
                    .next()
                    .unwrap()
                    .strip_suffix(" HTTP/1.1")
                    .unwrap()
                    .to_string(),
            );
            let body = body.to_string();
            stream.write_all(format!(
                "HTTP/1.1 {status} Response\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            ).as_bytes()).await.unwrap();
        }
    });

    let mut config = json!({
        "url": url, "token": "test-token",
        "rewrite": {"from": "/downloads", "to": "/media"},
        "request": {"timeout": 1, "headers": {"X-Test": "custom"}}
    });
    if let Some(enabled) = empty_trash {
        config["empty_trash"] = json!(enabled);
    }
    let plex: Plex = serde_json::from_value(config).unwrap();
    let result = plex.process(events).await.unwrap();
    server.abort();
    assert_eq!(*requests.lock().unwrap(), expected);
    result
}

#[tokio::test]
async fn empty_trash_is_opt_in() {
    for enabled in [None, Some(false)] {
        let ev = event("one");
        assert_eq!(
            process(
                enabled,
                &[&ev],
                vec![
                    (LIBRARIES, 200, libraries(json!(false))),
                    (SCAN, 200, json!({})),
                ]
            )
            .await,
            vec!["one"]
        );
    }
}

#[tokio::test]
async fn empties_only_scanned_library_once_after_batch_finishes_scanning() {
    let first = event("one");
    let second = event("two");
    let mut result = process(
        Some(true),
        &[&first, &second],
        vec![
            (LIBRARIES, 200, libraries(json!(false))),
            (SCAN, 200, json!({})),
            (SCAN, 200, json!({})),
            (LIBRARIES, 200, libraries(json!(true))),
            (LIBRARIES, 200, libraries(json!(false))),
            (TRASH, 200, json!({})),
        ],
    )
    .await;
    result.sort();
    assert_eq!(result, vec!["one", "two"]);
}

#[tokio::test]
async fn cleanup_failure_leaves_all_affected_events_retryable() {
    let first = event("one");
    let second = event("two");
    assert!(process(
        Some(true),
        &[&first, &second],
        vec![
            (LIBRARIES, 200, libraries(json!(false))),
            (SCAN, 200, json!({})),
            (SCAN, 200, json!({})),
            (LIBRARIES, 200, scanned_libraries()),
            (TRASH, 500, json!({})),
        ]
    )
    .await
    .is_empty());
}

#[tokio::test]
async fn waits_for_queued_scan_to_start_before_emptying_trash() {
    let ev = event("one");
    assert_eq!(
        process(
            Some(true),
            &[&ev],
            vec![
                (LIBRARIES, 200, libraries(json!(false))),
                (SCAN, 200, json!({})),
                (LIBRARIES, 200, libraries(json!(false))),
                (LIBRARIES, 200, libraries(json!(true))),
                (LIBRARIES, 200, scanned_libraries()),
                (TRASH, 200, json!({})),
            ]
        )
        .await,
        vec!["one"]
    );
}

#[tokio::test]
async fn recognizes_scan_that_finishes_between_polls() {
    let ev = event("one");
    assert_eq!(
        process(
            Some(true),
            &[&ev],
            vec![
                (LIBRARIES, 200, libraries(json!(false))),
                (SCAN, 200, json!({})),
                (LIBRARIES, 200, scanned_libraries()),
                (TRASH, 200, json!({})),
            ]
        )
        .await,
        vec!["one"]
    );
}

#[tokio::test]
async fn failed_scan_prevents_library_cleanup() {
    let ev = event("one");
    assert!(process(
        Some(true),
        &[&ev],
        vec![
            (LIBRARIES, 200, libraries(json!(false))),
            (SCAN, 500, json!({})),
        ]
    )
    .await
    .is_empty());
}

#[tokio::test]
async fn partially_failed_batch_does_not_empty_library_trash() {
    let first = event("one");
    let second = event("two");
    assert!(process(
        Some(true),
        &[&first, &second],
        vec![
            (LIBRARIES, 200, libraries(json!(false))),
            (SCAN, 200, json!({})),
            (SCAN, 500, json!({})),
        ]
    )
    .await
    .is_empty());
}

#[tokio::test]
async fn unknown_scan_status_prevents_cleanup_and_success() {
    let mut missing_status = libraries(json!(false));
    missing_status["MediaContainer"]["Directory"][0]
        .as_object_mut()
        .unwrap()
        .remove("refreshing");
    for (status, body) in [
        (500, json!({})),
        (200, json!({"MediaContainer": {"Directory": []}})),
        (200, libraries(Value::Null)),
        (200, missing_status),
    ] {
        let ev = event("one");
        assert!(process(
            Some(true),
            &[&ev],
            vec![
                (LIBRARIES, 200, libraries(json!(false))),
                (SCAN, 200, json!({})),
                (LIBRARIES, status, body),
            ]
        )
        .await
        .is_empty());
    }
}

#[tokio::test]
async fn scan_failure_only_blocks_cleanup_for_its_own_library() {
    let first = event("one");
    let mut second = event("two");
    second.file_path = "/other/Movie/two.mkv".into();
    assert_eq!(
        process(
            Some(true),
            &[&first, &second],
            vec![
                (LIBRARIES, 200, libraries(json!(false))),
                (SCAN, 500, json!({})),
                (
                    "GET /plex/library/sections/2/refresh?path=%2Fother%2FMovie",
                    200,
                    json!({})
                ),
                (LIBRARIES, 200, scanned_libraries()),
                ("PUT /plex/library/sections/2/emptyTrash", 200, json!({})),
            ]
        )
        .await,
        vec!["two"]
    );
}

#[tokio::test]
async fn unmatched_path_does_not_empty_trash() {
    let mut ev = event("one");
    ev.file_path = "/unmatched/one.mkv".into();
    assert!(process(
        Some(true),
        &[&ev],
        vec![(LIBRARIES, 200, libraries(json!(false))),]
    )
    .await
    .is_empty());
}
