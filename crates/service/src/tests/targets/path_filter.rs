use crate::settings::targets::Target;
use autopulse_database::models::{FoundStatus, ProcessStatus, ScanEvent};

fn event(path: &str) -> ScanEvent {
    let now = chrono::Utc::now().naive_utc();

    ScanEvent {
        id: "event-id".to_string(),
        event_source: "trigger".to_string(),
        event_timestamp: now,
        file_path: path.to_string(),
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

#[test]
fn target_filter_include_matches_rewritten_path() {
    let target: Target = serde_json::from_value(serde_json::json!({
        "type": "audiobookshelf",
        "url": "http://audiobookshelf:13378",
        "token": "token",
        "rewrite": { "from": "/downloads/audiobooks", "to": "/books" },
        "filter": { "include": ["^/books/"] }
    }))
    .unwrap();

    assert!(target.should_process_event(&event("/downloads/audiobooks/Book/Chapter 1.m4b")));
}

#[test]
fn target_filter_include_rejects_non_matching_path() {
    let target: Target = serde_json::from_value(serde_json::json!({
        "type": "audiobookshelf",
        "url": "http://audiobookshelf:13378",
        "token": "token",
        "filter": { "include": ["^/books/"] }
    }))
    .unwrap();

    assert!(!target.should_process_event(&event("/movies/Movie.mkv")));
}

#[test]
fn target_filter_exclude_rejects_matching_path() {
    let target: Target = serde_json::from_value(serde_json::json!({
        "type": "audiobookshelf",
        "url": "http://audiobookshelf:13378",
        "token": "token",
        "filter": { "exclude": ["/podcasts/"] }
    }))
    .unwrap();

    assert!(!target.should_process_event(&event("/books/podcasts/Episode 1.mp3")));
}

#[test]
fn target_filter_exclude_wins_over_include() {
    let target: Target = serde_json::from_value(serde_json::json!({
        "type": "audiobookshelf",
        "url": "http://audiobookshelf:13378",
        "token": "token",
        "filter": {
            "include": ["^/books/"],
            "exclude": ["^/books/podcasts/"]
        }
    }))
    .unwrap();

    assert!(!target.should_process_event(&event("/books/podcasts/Episode 1.mp3")));
}

#[test]
fn target_filter_include_and_exclude_deserialize() {
    let target: Target = serde_json::from_value(serde_json::json!({
        "type": "audiobookshelf",
        "url": "http://audiobookshelf:13378",
        "token": "token",
        "filter": {
            "include": ["^/books/"],
            "exclude": ["/samples/"]
        }
    }))
    .unwrap();

    assert!(target.should_process_event(&event("/books/Novel.m4b")));
    assert!(!target.should_process_event(&event("/books/samples/Sample.m4b")));
}

#[test]
fn invalid_target_include_regex_is_rejected() {
    let res: Result<Target, _> = serde_json::from_value(serde_json::json!({
        "type": "audiobookshelf",
        "url": "http://audiobookshelf:13378",
        "token": "token",
        "filter": { "include": ["[unclosed"] }
    }));

    assert!(res.is_err());
    let msg = res.err().unwrap().to_string();
    assert!(msg.contains("invalid filter.include regex"));
}

#[test]
fn invalid_target_exclude_regex_is_rejected() {
    let res: Result<Target, _> = serde_json::from_value(serde_json::json!({
        "type": "audiobookshelf",
        "url": "http://audiobookshelf:13378",
        "token": "token",
        "filter": { "exclude": ["[unclosed"] }
    }));

    assert!(res.is_err());
    let msg = res.err().unwrap().to_string();
    assert!(msg.contains("invalid filter.exclude regex"));
}
