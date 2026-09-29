use crate::settings::targets::{plex::Plex, TargetProcess};
use autopulse_database::models::{FoundStatus, ProcessStatus, ScanEvent};
use futures::SinkExt;
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

async fn respond(
    listener: &tokio::net::TcpListener,
    status: u16,
    body: Value,
    received: &Arc<Mutex<Vec<String>>>,
) {
    let (mut stream, _) = listener.accept().await.unwrap();
    let mut buffer = Vec::new();
    loop {
        let mut chunk = [0; 1024];
        let count = stream.read(&mut chunk).await.unwrap();
        assert_ne!(count, 0);
        buffer.extend_from_slice(&chunk[..count]);
        if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
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
    stream
        .write_all(
            format!(
                "HTTP/1.1 {status} Response\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .as_bytes(),
        )
        .await
        .unwrap();
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
    let watcher_count = if empty_trash == Some(true) {
        responses
            .iter()
            .filter_map(|(request, _, _)| {
                request
                    .strip_prefix("GET /plex/library/sections/")
                    .and_then(|path| path.split_once("/refresh?").map(|(section, _)| section))
            })
            .collect::<std::collections::HashSet<_>>()
            .len()
    } else {
        0
    };
    let completed_sections = responses
        .iter()
        .filter_map(|(request, _, _)| {
            request
                .strip_prefix("PUT /plex/library/sections/")
                .and_then(|path| path.strip_suffix("/emptyTrash"))
                .map(str::to_owned)
        })
        .collect::<Vec<_>>();
    let server = tokio::spawn(async move {
        let mut responses = responses.into_iter();
        if watcher_count > 0 {
            let (_, status, body) = responses.next().unwrap();
            respond(&listener, status, body, &received).await;
        }

        let mut websockets = Vec::new();
        for _ in 0..watcher_count {
            let (stream, _) = listener.accept().await.unwrap();
            let mut websocket = tokio_tungstenite::accept_async(stream).await.unwrap();
            for section in &completed_sections {
                let uuid = format!("scan-{section}");
                for event in ["started", "updated", "ended"] {
                    let context =
                        (event != "started").then(|| json!({"librarySectionID": section}));
                    websocket
                        .send(tokio_tungstenite::tungstenite::Message::Text(
                            json!({"NotificationContainer": {
                                "type": "activity",
                                "ActivityNotification": [{
                                    "event": event,
                                    "uuid": uuid,
                                    "Activity": {
                                        "type": "library.update.section",
                                        "Context": context
                                    }
                                }]
                            }})
                            .to_string()
                            .into(),
                        ))
                        .await
                        .unwrap();
                }
            }
            websocket
                .send(tokio_tungstenite::tungstenite::Message::Text(
                    json!({"NotificationContainer": {
                        "type": "status",
                        "StatusNotification": [{
                            "title": "Library scan complete",
                            "notificationName": "LIBRARY_UPDATE"
                        }]
                    }})
                    .to_string()
                    .into(),
                ))
                .await
                .unwrap();
            websockets.push(websocket);
        }

        for (_, status, body) in responses {
            respond(&listener, status, body, &received).await;
        }
        // Keep the notification connection alive until all HTTP work is complete.
        drop(websockets);
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
    tokio::time::timeout(std::time::Duration::from_secs(2), server)
        .await
        .expect("mock Plex server did not receive the expected requests")
        .unwrap();
    let requests = requests.lock().unwrap();
    let (mut actual_trash, actual_other): (Vec<_>, Vec<_>) = requests
        .iter()
        .cloned()
        .partition(|request| request.ends_with("/emptyTrash"));
    let (mut expected_trash, expected_other): (Vec<_>, Vec<_>) = expected
        .into_iter()
        .partition(|request| request.ends_with("/emptyTrash"));
    actual_trash.sort();
    expected_trash.sort();
    assert_eq!(actual_other, expected_other);
    assert_eq!(actual_trash, expected_trash);
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
            (TRASH, 200, json!({})),
        ],
    )
    .await;
    result.sort();
    assert_eq!(result, vec!["one", "two"]);
}

#[tokio::test]
async fn waits_for_all_scanned_sections_then_empties_each_once() {
    let movies = event("movies");
    let mut series = event("series");
    series.file_path = "/other/Series/series.mkv".into();

    let mut result = process(
        Some(true),
        &[&movies, &series],
        vec![
            (LIBRARIES, 200, libraries(json!(false))),
            (SCAN, 200, json!({})),
            (
                "GET /plex/library/sections/2/refresh?path=%2Fother%2FSeries",
                200,
                json!({}),
            ),
            (TRASH, 200, json!({})),
            ("PUT /plex/library/sections/2/emptyTrash", 200, json!({})),
        ],
    )
    .await;
    result.sort();
    assert_eq!(result, vec!["movies", "series"]);
}

#[tokio::test]
async fn cleanup_failure_does_not_change_completed_scan_status() {
    let first = event("one");
    let second = event("two");
    let mut result = process(
        Some(true),
        &[&first, &second],
        vec![
            (LIBRARIES, 200, libraries(json!(false))),
            (SCAN, 200, json!({})),
            (SCAN, 200, json!({})),
            (TRASH, 500, json!({})),
        ],
    )
    .await;
    result.sort();
    assert_eq!(result, vec!["one", "two"]);
}

#[tokio::test]
async fn waits_for_scan_completion_notification_before_emptying_trash() {
    let ev = event("one");
    assert_eq!(
        process(
            Some(true),
            &[&ev],
            vec![
                (LIBRARIES, 200, libraries(json!(false))),
                (SCAN, 200, json!({})),
                (TRASH, 200, json!({})),
            ]
        )
        .await,
        vec!["one"]
    );
}

#[tokio::test]
async fn recognizes_fast_scan_from_buffered_websocket_events() {
    let ev = event("one");
    assert_eq!(
        process(
            Some(true),
            &[&ev],
            vec![
                (LIBRARIES, 200, libraries(json!(false))),
                (SCAN, 200, json!({})),
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
    assert_eq!(
        process(
            Some(true),
            &[&first, &second],
            vec![
                (LIBRARIES, 200, libraries(json!(false))),
                (SCAN, 200, json!({})),
                (SCAN, 500, json!({})),
            ]
        )
        .await,
        vec!["one"]
    );
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
