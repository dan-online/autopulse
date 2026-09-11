use crate::manager::PulseManager;
use autopulse_database::{
    diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl},
    models::NewScanEvent,
    schema::scan_events::dsl::*,
};

async fn persistence_regressions(manager: PulseManager) {
    let mut event = NewScanEvent::default();
    event.file_path = format!("persistence-{}", event.id);
    let initial = manager.add_event(&event).await.unwrap();
    let later = NewScanEvent {
        can_process: event.can_process + chrono::Duration::seconds(60),
        ..event
    };
    let later = manager.add_event(&later).await.unwrap();
    let saved = manager
        .database(move |conn| {
            let found = conn
                .update_found(&initial, "found", initial.created_at)?
                .unwrap();
            assert_eq!(found.can_process, later.can_process);
            let mut complete = initial.clone();
            complete.process_status = "complete".into();
            complete.processed_at = Some(initial.created_at);
            complete.targets_hit = "plex".into();
            let saved = conn.update_process(&initial, &complete)?.unwrap();
            assert_eq!(saved.can_process, later.can_process);
            assert_eq!(saved.found_status, "found");
            assert!(conn.update_process(&initial, &complete)?.is_none());
            Ok(saved)
        })
        .await
        .unwrap();

    let retry = manager.reschedule_event(&saved.id).await.unwrap();
    assert_eq!(retry.process_status, "retry");
    assert_eq!(retry.processed_at, None);
    assert!(retry.targets_hit.is_empty());
    assert!(retry.next_retry_at.is_some());

    let retry = manager
        .database(move |conn| {
            Ok(diesel::update(scan_events.find(retry.id))
                .set(next_retry_at.eq(Some(
                    chrono::Utc::now().naive_utc() + chrono::Duration::seconds(60),
                )))
                .get_result::<autopulse_database::models::ScanEvent>(conn)?)
        })
        .await
        .unwrap();
    let previous = retry.clone();
    let newer = manager.reschedule_event(&retry.id).await.unwrap();
    assert!(newer.next_retry_at < previous.next_retry_at);
    let failed = manager
        .database(move |conn| {
            let mut failed = retry.clone();
            failed.process_status = "failed".into();
            failed.next_retry_at = None;
            Ok(conn
                .update_process(&previous, &failed)?
                .expect("an accelerated deadline must not discard a target result"))
        })
        .await
        .unwrap();
    assert_eq!(failed.next_retry_at, None);
    assert_eq!(failed.process_status, "failed");

    for due_at in [
        None,
        Some(chrono::Utc::now().naive_utc() - chrono::Duration::seconds(60)),
    ] {
        let event_id = failed.id.clone();
        let in_flight = manager
            .database(move |conn| {
                Ok(diesel::update(scan_events.find(event_id))
                    .set((
                        process_status.eq("retry"),
                        next_retry_at.eq(due_at),
                        can_process
                            .eq(chrono::Utc::now().naive_utc() - chrono::Duration::seconds(60)),
                        targets_hit.eq(""),
                        processed_at.eq(None::<chrono::NaiveDateTime>),
                    ))
                    .get_result::<autopulse_database::models::ScanEvent>(conn)?)
            })
            .await
            .unwrap();
        let manual = manager.reschedule_event(&in_flight.id).await.unwrap();
        assert_eq!(manual.next_retry_at, in_flight.next_retry_at);
        let completed = manager
            .database(move |conn| {
                let mut completed = in_flight.clone();
                completed.process_status = "complete".into();
                completed.next_retry_at = None;
                completed.targets_hit = "plex".into();
                completed.processed_at = Some(chrono::Utc::now().naive_utc());
                conn.update_process(&in_flight, &completed)
            })
            .await
            .unwrap()
            .expect("manual retry must not discard an in-flight target result");
        assert_eq!(completed.process_status, "complete");
        assert_eq!(completed.targets_hit, "plex");
    }

    let pending = manager
        .add_event(&NewScanEvent {
            file_path: format!("pending-{}", failed.id),
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(manager.reschedule_event(&pending.id).await.is_err());
    assert_eq!(
        manager
            .get_event(&pending.id)
            .await
            .unwrap()
            .unwrap()
            .process_status,
        "pending"
    );

    manager
        .database(move |conn| {
            diesel::delete(scan_events.filter(id.eq_any([failed.id, pending.id]))).execute(conn)?;
            Ok(())
        })
        .await
        .unwrap();
}

#[tokio::test]
#[cfg(feature = "sqlite")]
async fn sqlite_persistence_regressions() {
    persistence_regressions(crate::tests::util::fresh_manager("persistence")).await;
}

#[tokio::test]
#[cfg(feature = "postgres")]
#[ignore = "requires AUTOPULSE_TEST_POSTGRES_URL; run explicitly in PostgreSQL CI"]
async fn postgres_persistence_regressions() {
    let url = std::env::var("AUTOPULSE_TEST_POSTGRES_URL")
        .expect("PostgreSQL integration test requires AUTOPULSE_TEST_POSTGRES_URL");
    let pool = autopulse_database::conn::get_pool(&url).unwrap();
    autopulse_database::conn::get_conn(&pool)
        .unwrap()
        .migrate()
        .unwrap();
    persistence_regressions(PulseManager::new(
        crate::settings::Settings::default(),
        pool,
    ))
    .await;
}

#[tokio::test]
#[cfg(feature = "sqlite")]
async fn database_checkout_does_not_block_the_executor() {
    let manager = crate::tests::util::fresh_manager("blocking-checkout");
    let held = autopulse_database::conn::get_conn(&manager.pool).unwrap();
    let start = std::time::Instant::now();
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), manager.get_stats())
            .await
            .is_err()
    );
    let elapsed = start.elapsed();
    drop(held);
    assert!(
        elapsed < std::time::Duration::from_secs(1),
        "pool checkout blocked the executor for {elapsed:?}"
    );
    assert_eq!(manager.get_stats().await.unwrap().total, 0);
}

#[test]
#[cfg(feature = "sqlite")]
fn cancelled_database_work_keeps_its_slot_until_finished() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(2)
        .build()
        .unwrap();
    runtime.block_on(async {
        let manager = crate::tests::util::fresh_manager("blocking-cancel");
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let first_manager = manager.clone();
        let first = tokio::spawn(async move {
            first_manager
                .database(move |_| {
                    started_tx.send(()).unwrap();
                    release_rx.recv_timeout(std::time::Duration::from_secs(5))?;
                    Ok(())
                })
                .await
        });
        started_rx.await.unwrap();
        first.abort();
        assert!(first.await.unwrap_err().is_cancelled());
        let (entered_tx, mut entered_rx) = tokio::sync::oneshot::channel();
        let second = tokio::spawn(async move {
            manager
                .database(move |_| {
                    entered_tx.send(()).unwrap();
                    Ok(())
                })
                .await
        });
        let entered_early =
            tokio::time::timeout(std::time::Duration::from_millis(50), &mut entered_rx)
                .await
                .is_ok();
        // A prematurely released permit would let the second checkout occupy
        // the only spare blocking thread while the first closure still runs.
        let spare_thread = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            tokio::task::spawn_blocking(|| ()),
        )
        .await;
        release_tx.send(()).unwrap();
        second.await.unwrap().unwrap();
        assert!(!entered_early);
        spare_thread.unwrap().unwrap();
    });
}

#[tokio::test]
#[cfg(feature = "sqlite")]
async fn pagination_breaks_ties_by_id_and_rejects_overflow() {
    let manager = crate::tests::util::fresh_manager("pagination");
    manager
        .database(|conn| {
            for event_id in ["c", "a", "b"] {
                conn.insert_and_return(&NewScanEvent {
                    id: event_id.into(),
                    file_path: event_id.into(),
                    ..Default::default()
                })?;
            }
            diesel::update(scan_events)
                .set(created_at.eq(chrono::Utc::now().naive_utc()))
                .execute(conn)?;
            Ok(())
        })
        .await
        .unwrap();
    for (sort, expected) in [
        (None, ["c", "b", "a"]),
        (Some("-created_at".to_string()), ["a", "b", "c"]),
    ] {
        for (page, expected_id) in expected.iter().enumerate() {
            let rows = manager
                .get_events(1, page as u64 + 1, sort.clone(), None, None)
                .await
                .unwrap();
            assert_eq!(&rows[0].id, expected_id);
        }
    }
    assert!(manager
        .get_events(100, u64::MAX, None, None, None)
        .await
        .is_err());
    assert!(manager
        .get_events(1, i64::MAX as u64 + 2, None, None, None)
        .await
        .is_err());
}
