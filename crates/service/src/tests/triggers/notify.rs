#![cfg(test)]
mod tests {
    use crate::settings::triggers::notify::Notify;
    use autopulse_utils::generate_uuid;
    use notify_debouncer_full::notify::{
        event::{AccessKind, AccessMode, CreateKind},
        EventKind,
    };
    use std::{env, fs::create_dir, io::Write, time::Duration};
    use tokio::sync::mpsc;
    use tokio::time::timeout;

    fn test_notifier(path: &std::path::Path, debounce_secs: u64) -> Notify {
        Notify {
            paths: vec![path.to_string_lossy().to_string()],
            rewrite: None,
            recursive: None,
            excludes: vec![],
            filters: None,
            timer: Default::default(),
            backend: Default::default(),
            debounce: Some(debounce_secs),
        }
    }

    #[tokio::test]
    async fn test_debouncer_emits_create_event() -> anyhow::Result<()> {
        let path = env::temp_dir().join(generate_uuid());
        create_dir(&path)?;

        let notifier = test_notifier(&path, 1);
        let (_, mut rx) = notifier.async_watcher()?;

        let file = path.join("test.txt");
        std::fs::File::create(&file)?;

        let _ = timeout(Duration::from_secs(5), async {
            if let Some(result) = rx.recv().await {
                let events = result.map_err(|e| anyhow::anyhow!("{e:?}"))?;
                let has_create = events.iter().any(|debounced_event| {
                    debounced_event.event.kind == EventKind::Create(CreateKind::File)
                });
                assert!(has_create, "expected a Create(File) event");
                return Ok(());
            }
            anyhow::bail!("Event not received within timeout");
        })
        .await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_watcher_filters_close_write() -> anyhow::Result<()> {
        let path = env::temp_dir().join(generate_uuid());
        create_dir(&path)?;

        let notifier = test_notifier(&path, 1);
        let (tx, mut rx) = mpsc::unbounded_channel();

        let notifier_clone = notifier.clone();
        let watcher_task = tokio::spawn(async move { notifier_clone.watcher(tx).await });

        // Small delay to let the watcher start
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Create a file, write to it, then close it (drop)
        let file_path = path.join("write_test.txt");
        {
            let mut file = std::fs::File::create(&file_path)?;
            file.write_all(b"hello world")?;
            file.flush()?;
        } // file closed here — should trigger CloseWrite on Linux

        let result = timeout(Duration::from_secs(5), async {
            let mut events = vec![];
            // Collect events for a few seconds
            loop {
                match tokio::time::timeout(Duration::from_secs(3), rx.recv()).await {
                    Ok(Some(event)) => events.push(event),
                    _ => break,
                }
            }
            events
        })
        .await?;

        // Should have at least one event (Create or CloseWrite)
        assert!(
            !result.is_empty(),
            "expected at least one event from watcher"
        );

        // Verify that we got either a Create or a CloseWrite event
        let has_expected_event = result.iter().any(|(_, kind)| {
            matches!(
                kind,
                EventKind::Create(_) | EventKind::Access(AccessKind::Close(AccessMode::Write))
            )
        });
        assert!(
            has_expected_event,
            "expected a Create or CloseWrite event, got: {result:?}"
        );

        watcher_task.abort();
        Ok(())
    }

    #[tokio::test]
    async fn test_watcher_debounces_rapid_writes() -> anyhow::Result<()> {
        let path = env::temp_dir().join(generate_uuid());
        create_dir(&path)?;

        let notifier = test_notifier(&path, 1);
        let (tx, mut rx) = mpsc::unbounded_channel();

        let notifier_clone = notifier.clone();
        let watcher_task = tokio::spawn(async move { notifier_clone.watcher(tx).await });

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Simulate rapid writes (like a file copy) — many writes to the same file
        let file_path = path.join("rapid_write.txt");
        {
            let mut file = std::fs::File::create(&file_path)?;
            for i in 0..100 {
                file.write_all(format!("chunk {i}\n").as_bytes())?;
                file.flush()?;
            }
        } // file closed here

        // Collect all events over a window longer than the debounce timeout
        let result = timeout(Duration::from_secs(5), async {
            let mut events = vec![];
            loop {
                match tokio::time::timeout(Duration::from_secs(3), rx.recv()).await {
                    Ok(Some(event)) => events.push(event),
                    _ => break,
                }
            }
            events
        })
        .await?;

        // With debouncing + CloseWrite filtering, we should get far fewer events
        // than the 100 writes we performed. Without the fix this would be ~100+ events.
        assert!(
            result.len() < 10,
            "expected debounced events to be fewer than 10, got {}",
            result.len()
        );

        // Should still have at least one event
        assert!(
            !result.is_empty(),
            "expected at least one event after rapid writes"
        );

        watcher_task.abort();
        Ok(())
    }

    #[tokio::test]
    async fn test_watcher_filters_out_data_modify() -> anyhow::Result<()> {
        let path = env::temp_dir().join(generate_uuid());
        create_dir(&path)?;

        let notifier = test_notifier(&path, 1);
        let (tx, mut rx) = mpsc::unbounded_channel();

        let notifier_clone = notifier.clone();
        let watcher_task = tokio::spawn(async move { notifier_clone.watcher(tx).await });

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Create and write to file
        let file_path = path.join("modify_test.txt");
        {
            let mut file = std::fs::File::create(&file_path)?;
            file.write_all(b"test data")?;
            file.flush()?;
        }

        // Collect events
        let result = timeout(Duration::from_secs(5), async {
            let mut events = vec![];
            loop {
                match tokio::time::timeout(Duration::from_secs(3), rx.recv()).await {
                    Ok(Some(event)) => events.push(event),
                    _ => break,
                }
            }
            events
        })
        .await?;

        // None of the forwarded events should be Modify(Data(_))
        let has_modify_data = result.iter().any(|(_, kind)| {
            matches!(
                kind,
                EventKind::Modify(notify_debouncer_full::notify::event::ModifyKind::Data(_))
            )
        });
        assert!(
            !has_modify_data,
            "should not forward Modify(Data) events, got: {result:?}"
        );

        watcher_task.abort();
        Ok(())
    }

    #[tokio::test]
    async fn test_watcher_emits_remove_event() -> anyhow::Result<()> {
        let path = env::temp_dir().join(generate_uuid());
        create_dir(&path)?;

        let notifier = test_notifier(&path, 1);
        let (tx, mut rx) = mpsc::unbounded_channel();

        let notifier_clone = notifier.clone();
        let watcher_task = tokio::spawn(async move { notifier_clone.watcher(tx).await });

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Create a file, wait for debounce window to pass, then delete it
        let file_path = path.join("delete_test.txt");
        std::fs::File::create(&file_path)?;
        tokio::time::sleep(Duration::from_secs(2)).await;
        std::fs::remove_file(&file_path)?;

        let result = timeout(Duration::from_secs(5), async {
            let mut events = vec![];
            loop {
                match tokio::time::timeout(Duration::from_secs(3), rx.recv()).await {
                    Ok(Some(event)) => events.push(event),
                    _ => break,
                }
            }
            events
        })
        .await?;

        // Should have a Remove event for the deleted file
        let has_remove = result
            .iter()
            .any(|(_, kind)| matches!(kind, EventKind::Remove(_)));
        assert!(
            has_remove,
            "expected a Remove event after file deletion, got: {result:?}"
        );

        watcher_task.abort();
        Ok(())
    }
}
