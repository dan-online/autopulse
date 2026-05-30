use crate::tests::util::fresh_manager;
use crate::{manager::PulseManager, settings::Settings};
use autopulse_database::conn::get_conn;
use autopulse_database::conn::get_pool;
use autopulse_database::models::{FoundStatus, NewScanEvent, ProcessStatus};
use chrono::{Duration, Utc};
use std::sync::{Arc, Barrier};

fn new_event(source: &str, path: &str, can_process_secs: i64) -> NewScanEvent {
    NewScanEvent {
        event_source: source.to_string(),
        file_path: path.to_string(),
        can_process: Utc::now().naive_utc() + Duration::seconds(can_process_secs),
        ..Default::default()
    }
}

#[test]
fn dedupes_pending_event_across_triggers_with_same_path() {
    let m = fresh_manager("dedupe-cross-source");
    let a = m
        .add_event(&new_event("sonarr", "/media/a.mkv", 30))
        .unwrap();
    let b = m
        .add_event(&new_event("notify", "/media/a.mkv", 30))
        .unwrap();
    assert_eq!(
        a.id, b.id,
        "second add for same pending path must return original row"
    );
    assert_eq!(b.event_source, "sonarr", "original event_source preserved");
}

#[test]
fn dedupe_keeps_the_later_can_process_time() {
    let m = fresh_manager("dedupe-can-process");
    let first = m
        .add_event(&new_event("sonarr", "/media/b.mkv", 10))
        .unwrap();
    let second = m
        .add_event(&new_event("notify", "/media/b.mkv", 60))
        .unwrap();
    assert!(second.can_process >= first.can_process);
    assert!(second.can_process > Utc::now().naive_utc() + Duration::seconds(30));
}

#[test]
fn dedupe_does_not_regress_found_status() {
    let m = fresh_manager("dedupe-found");
    let mut found = new_event("sonarr", "/media/c.mkv", 30);
    found.found_status = FoundStatus::Found.into();
    let inserted = m.add_event(&found).unwrap();
    assert_eq!(inserted.found_status, "found");

    let notfound = new_event("notify", "/media/c.mkv", 30);
    let after = m.add_event(&notfound).unwrap();
    assert_eq!(
        after.found_status, "found",
        "must not downgrade found→not_found on coalesce"
    );
}

#[test]
fn dedupe_preserves_later_file_hash_when_existing_row_has_none() {
    let m = fresh_manager("dedupe-file-hash");
    let first = m
        .add_event(&new_event("notify", "/media/hash.mkv", 30))
        .unwrap();
    assert_eq!(first.file_hash, None);

    let mut with_hash = new_event("manual", "/media/hash.mkv", 30);
    with_hash.file_hash = Some("sha256:abc123".to_string());
    let after = m.add_event(&with_hash).unwrap();

    assert_eq!(after.id, first.id, "same pending path should coalesce");
    assert_eq!(
        after.file_hash,
        Some("sha256:abc123".to_string()),
        "a later hash should fill an empty existing hash"
    );
}

#[test]
fn concurrent_same_path_add_event_coalesces_without_unique_errors() {
    const THREADS: usize = 32;

    for attempt in 0..8 {
        let m = fresh_manager(&format!("dedupe-concurrent-{attempt}"));
        let barrier = Arc::new(Barrier::new(THREADS));
        let mut handles = Vec::with_capacity(THREADS);

        for thread in 0..THREADS {
            let m = m.clone();
            let barrier = barrier.clone();
            handles.push(std::thread::spawn(move || {
                barrier.wait();
                m.add_event(&new_event(
                    &format!("source-{thread}"),
                    "/media/concurrent.mkv",
                    thread as i64,
                ))
            }));
        }

        let mut ids = vec![];
        for handle in handles {
            ids.push(
                handle
                    .join()
                    .expect("worker thread should not panic")
                    .unwrap()
                    .id,
            );
        }

        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 1, "all concurrent callers should get one row id");
    }
}

#[test]
fn postgres_upsert_conflict_target_matches_partial_index() {
    let Ok(url) = std::env::var("AUTOPULSE_TEST_POSTGRES_URL") else {
        return;
    };

    let pool = get_pool(&url).expect("postgres test database pool should initialize");
    get_conn(&pool)
        .expect("postgres test database connection should initialize")
        .migrate()
        .expect("postgres test database migrations should apply");

    let mut settings = Settings::default();
    settings.app.database_url = url;
    let m = PulseManager::new(settings, pool);
    let unique_path = format!(
        "/media/postgres-{}.mkv",
        Utc::now()
            .timestamp_nanos_opt()
            .expect("current timestamp should fit in nanos")
    );

    let first = m.add_event(&new_event("sonarr", &unique_path, 30)).unwrap();
    let second = m.add_event(&new_event("notify", &unique_path, 60)).unwrap();

    assert_eq!(first.id, second.id, "postgres upsert should coalesce");
    assert!(second.can_process > first.can_process);
}

#[test]
fn retry_event_coalesces_with_new_arrival() {
    let m = fresh_manager("dedupe-retry");
    let inserted = m
        .add_event(&new_event("sonarr", "/media/r.mkv", 30))
        .unwrap();

    // Simulate the runner moving the row into Retry.
    use autopulse_database::diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl};
    use autopulse_database::schema::scan_events::dsl::{process_status, scan_events};
    diesel::update(scan_events.find(&inserted.id))
        .set(process_status.eq::<String>(ProcessStatus::Retry.into()))
        .execute(&mut get_conn(&m.pool).unwrap())
        .unwrap();

    let again = m
        .add_event(&new_event("notify", "/media/r.mkv", 60))
        .unwrap();
    assert_eq!(
        again.id, inserted.id,
        "Retry row must coalesce per user decision"
    );
    assert!(again.can_process > inserted.can_process);
}

#[test]
fn completed_event_does_not_coalesce_with_new_event() {
    let m = fresh_manager("dedupe-complete");
    let inserted = m
        .add_event(&new_event("sonarr", "/media/d.mkv", 0))
        .unwrap();

    use autopulse_database::diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl};
    use autopulse_database::schema::scan_events::dsl::{process_status, scan_events};
    diesel::update(scan_events.find(&inserted.id))
        .set(process_status.eq::<String>(ProcessStatus::Complete.into()))
        .execute(&mut get_conn(&m.pool).unwrap())
        .unwrap();

    let again = m
        .add_event(&new_event("notify", "/media/d.mkv", 0))
        .unwrap();
    assert_ne!(again.id, inserted.id, "completed row must not be reopened");
}

#[test]
fn failed_event_does_not_coalesce_with_new_event() {
    let m = fresh_manager("dedupe-failed");
    let inserted = m
        .add_event(&new_event("sonarr", "/media/e.mkv", 0))
        .unwrap();

    use autopulse_database::diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl};
    use autopulse_database::schema::scan_events::dsl::{process_status, scan_events};
    diesel::update(scan_events.find(&inserted.id))
        .set(process_status.eq::<String>(ProcessStatus::Failed.into()))
        .execute(&mut get_conn(&m.pool).unwrap())
        .unwrap();

    let again = m
        .add_event(&new_event("notify", "/media/e.mkv", 0))
        .unwrap();
    assert_ne!(again.id, inserted.id, "failed row must not be reopened");
}
