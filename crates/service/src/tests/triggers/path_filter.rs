use crate::settings::triggers::Trigger;

#[test]
fn sonarr_trigger_excludes_path_via_regex() {
    let trigger: Trigger = serde_json::from_value(serde_json::json!({
        "type": "sonarr",
        "filter": { "exclude": ["/tv/Excluded Show/", "\\.sample\\.mkv$"] }
    }))
    .unwrap();

    assert!(!trigger.should_process_path("/tv/Excluded Show/S01E01.mkv"));
    assert!(!trigger.should_process_path("/tv/Other/S01E02.sample.mkv"));
    assert!(trigger.should_process_path("/tv/Other/S01E02.mkv"));
}
