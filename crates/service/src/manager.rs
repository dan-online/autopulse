use super::runner::PulseRunner;

use crate::settings::triggers::Trigger;
use crate::settings::webhooks::{EventType, WebhookManager};
use crate::settings::Settings;

use autopulse_database::diesel::sql_types::BigInt;
use autopulse_database::diesel::QueryableByName;
use autopulse_database::schema::scan_events::{
    can_process, created_at, event_source, file_path, id, next_retry_at, processed_at, targets_hit,
    updated_at,
};
use autopulse_database::{
    conn::{get_conn, DbPool},
    diesel::{
        self, EscapeExpressionMethods, ExpressionMethods, QueryDsl, RunQueryDsl,
        TextExpressionMethods,
    },
    models::{FoundStatus, NewScanEvent, ProcessStatus, ScanEvent},
    schema::scan_events::{dsl::scan_events, found_status, process_status},
};
use notify_debouncer_full::notify;
use serde::Serialize;
use std::str::FromStr;
use std::sync::Arc;
use tokio::{select, sync::broadcast};
use tracing::{debug, error, info, warn};

/// Escape LIKE metacharacters so user input is matched literally.
fn escape_like_pattern(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// Represents the service statistics.
#[derive(Clone, Serialize, QueryableByName)]
pub struct Stats {
    /// The total number of events.
    #[diesel(sql_type = BigInt)]
    pub total: i64,
    /// The number of file events that have been processed.
    #[diesel(sql_type = BigInt)]
    pub processed: i64,
    /// The number of file events that are being retried.
    #[diesel(sql_type = BigInt)]
    pub retrying: i64,
    /// The number of file events that have failed.
    #[diesel(sql_type = BigInt)]
    pub failed: i64,
    /// The number of file events that are pending.
    #[diesel(sql_type = BigInt)]
    pub pending: i64,
}

/// In-process broadcast envelope, one per state transition. `ScanEvent` is
/// `Clone` and small enough that channel cloning is cheap.
#[derive(Clone, Debug)]
pub struct EventBroadcast {
    pub kind: EventType,
    pub event: ScanEvent,
    pub at: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone)]
pub struct PulseManager {
    pub settings: Arc<Settings>,
    pub pool: Arc<DbPool>,
    pub webhooks: Arc<WebhookManager>,
    /// In-process broadcast bus for state transitions; cloned `PulseManager`s
    /// share this channel.
    pub bus: broadcast::Sender<EventBroadcast>,
}

impl PulseManager {
    pub fn new(settings: Settings, pool: DbPool) -> Self {
        let settings = Arc::new(settings);
        let pool = Arc::new(pool);
        let webhooks = Arc::new(WebhookManager::new(settings.clone()));

        // Capacity 1024: absorbs a Sonarr season-import burst (~50
        // events) with headroom; failure mode under genuine overload
        // is the `Lagged` branch in the SSE handler, which triggers a
        // client-side resync.
        let (bus, _) = broadcast::channel(1024);

        Self {
            settings,
            pool,
            webhooks,
            bus,
        }
    }

    /// Subscribe to the broadcast bus; one receiver per consumer.
    pub fn subscribe(&self) -> broadcast::Receiver<EventBroadcast> {
        self.bus.subscribe()
    }

    /// Publish a state transition. Additive alongside the webhook bus; the
    /// `send` error (no subscribers) is intentionally swallowed.
    pub fn publish(&self, kind: EventType, event: &ScanEvent) {
        let _ = self.bus.send(EventBroadcast {
            kind,
            event: event.clone(),
            at: chrono::Utc::now(),
        });
    }

    /// Manual retry. Pending is excluded so we never clobber an event the
    /// runner is mid-pipeline (would dispatch duplicate target scans — the
    /// thing this service exists to prevent).
    ///
    /// Complete events also clear `targets_hit` and `processed_at`: every
    /// target is in `targets_hit`, so the runner's "skip already-hit" filter
    /// would otherwise produce a no-op. Failed/Retry keep their partial
    /// `targets_hit` so retry only redoes the targets that actually failed.
    ///
    /// `failed_times` is preserved — manual retry is an impulse, not an
    /// erasure of history.
    pub fn reschedule_event(&self, ev_id: &str) -> anyhow::Result<ScanEvent> {
        let now = chrono::Utc::now().naive_utc();
        let current: ScanEvent = scan_events
            .find(ev_id)
            .first::<ScanEvent>(&mut get_conn(&self.pool)?)
            .map_err(|e| match e {
                diesel::result::Error::NotFound => anyhow::anyhow!("event {ev_id} not found"),
                other => other.into(),
            })?;

        let status = ProcessStatus::from_str(&current.process_status)
            .map_err(|()| anyhow::anyhow!("event {ev_id} has unknown process_status"))?;

        let updated: ScanEvent = match status {
            ProcessStatus::Complete => diesel::update(scan_events.find(ev_id))
                .set((
                    process_status.eq::<String>(ProcessStatus::Retry.into()),
                    next_retry_at.eq(Some(now)),
                    updated_at.eq(now),
                    targets_hit.eq(String::new()),
                    processed_at.eq::<Option<chrono::NaiveDateTime>>(None),
                ))
                .get_result(&mut get_conn(&self.pool)?)?,
            ProcessStatus::Failed | ProcessStatus::Retry => diesel::update(scan_events.find(ev_id))
                .set((
                    process_status.eq::<String>(ProcessStatus::Retry.into()),
                    next_retry_at.eq(Some(now)),
                    updated_at.eq(now),
                ))
                .get_result(&mut get_conn(&self.pool)?)?,
            ProcessStatus::Pending => {
                anyhow::bail!("event {ev_id} is not in a retryable state")
            }
        };

        self.publish(EventType::Retrying, &updated);
        Ok(updated)
    }

    pub fn get_stats(&self) -> anyhow::Result<Stats> {
        diesel::sql_query(
            "SELECT \
                COUNT(*) as total, \
                COALESCE(SUM(CASE WHEN process_status = 'complete' THEN 1 ELSE 0 END), 0) as processed, \
                COALESCE(SUM(CASE WHEN process_status = 'retry' THEN 1 ELSE 0 END), 0) as retrying, \
                COALESCE(SUM(CASE WHEN process_status = 'failed' THEN 1 ELSE 0 END), 0) as failed, \
                COALESCE(SUM(CASE WHEN process_status = 'pending' THEN 1 ELSE 0 END), 0) as pending \
            FROM scan_events",
        )
        .get_result::<Stats>(&mut get_conn(&self.pool)?)
        .map_err(Into::into)
    }

    pub fn add_event(&self, ev: &NewScanEvent) -> anyhow::Result<ScanEvent> {
        let mut check = scan_events
            .filter(file_path.eq(&ev.file_path))
            .filter(process_status.eq::<String>(ProcessStatus::Pending.into()))
            .filter(event_source.eq(&ev.event_source))
            .into_boxed();

        if ev.found_status == FoundStatus::Found.to_string() {
            check = check.filter(found_status.eq(&ev.found_status));
        }

        let result = if let Ok(existing) = check.first::<ScanEvent>(&mut get_conn(&self.pool)?) {
            diesel::update(&existing)
                .set((
                    updated_at.eq(chrono::Utc::now().naive_utc()),
                    can_process.eq(ev.can_process),
                ))
                .get_result::<ScanEvent>(&mut get_conn(&self.pool)?)?
        } else {
            get_conn(&self.pool)?.insert_and_return(ev)?
        };

        self.publish(EventType::New, &result);

        Ok(result)
    }

    pub fn get_event(&self, ev_id: &String) -> anyhow::Result<Option<ScanEvent>> {
        Ok(scan_events
            .find(ev_id)
            .first::<ScanEvent>(&mut get_conn(&self.pool)?)
            .ok())
    }

    /// Total matching rows for pagination (independent of LIMIT/OFFSET).
    pub fn count_events(
        &self,
        status: Option<String>,
        search: Option<String>,
    ) -> anyhow::Result<i64> {
        let mut query = scan_events.into_boxed();

        if let Some(status) = status {
            query = query.filter(process_status.eq(status));
        }

        if let Some(search) = search {
            let escaped = escape_like_pattern(&search);
            query = query.filter(file_path.like(format!("%{escaped}%")).escape('\\'));
        }

        query
            .count()
            .get_result::<i64>(&mut get_conn(&self.pool)?)
            .map_err(Into::into)
    }

    pub fn get_events(
        &self,
        mut limit: u8,
        page: u64,
        sort: Option<String>,
        status: Option<String>,
        search: Option<String>,
    ) -> anyhow::Result<Vec<ScanEvent>> {
        let page = page.max(1);
        let mut query = scan_events.into_boxed();

        if let Some(status) = status {
            query = query.filter(process_status.eq(status));
        }

        if limit > 100 {
            limit = 100;
        }

        if let Some(mut sort) = sort {
            let mut direction = "desc";

            if sort.starts_with('-') {
                direction = "asc";
                sort = sort[1..].to_string();
            }

            if direction == "asc" {
                query = match sort.as_str() {
                    "id" => query.order(id.asc()),
                    "file_path" => query.order(file_path.asc()),
                    "process_status" => query.order(process_status.asc()),
                    "event_source" => query.order(event_source.asc()),
                    "created_at" => query.order(created_at.asc()),
                    "updated_at" => query.order(updated_at.asc()),
                    _ => {
                        return Err(anyhow::anyhow!("invalid sort field"));
                    }
                }
            } else {
                query = match sort.as_str() {
                    "id" => query.order(id.desc()),
                    "file_path" => query.order(file_path.desc()),
                    "process_status" => query.order(process_status.desc()),
                    "event_source" => query.order(event_source.desc()),
                    "created_at" => query.order(created_at.desc()),
                    "updated_at" => query.order(updated_at.desc()),
                    _ => {
                        return Err(anyhow::anyhow!("invalid sort field"));
                    }
                }
            }
        } else {
            query = query.order(created_at.desc());
        }

        if let Some(search) = search {
            let escaped = escape_like_pattern(&search);
            query = query.filter(file_path.like(format!("%{escaped}%")).escape('\\'));
        }

        query
            .limit(limit.into())
            .offset(((page - 1) * u64::from(limit)) as i64)
            .load::<ScanEvent>(&mut get_conn(&self.pool)?)
            .map_err(Into::into)
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        let mut runner = PulseRunner::new(self);
        let mut timer = tokio::time::interval(std::time::Duration::from_secs(1));
        let mut consecutive_errors: u32 = 0;

        loop {
            match runner.run().await {
                Ok(()) => {
                    consecutive_errors = 0;
                }
                Err(e) => {
                    consecutive_errors = consecutive_errors.saturating_add(1);
                    // Clamp the shift exponent before the cap: `1u64 << 64` panics in
                    // debug and wraps in release. The 60s cap is already reached at
                    // shift==6 (1<<6 == 64), so anything beyond 6 is dead weight.
                    let shift = consecutive_errors.min(6);
                    let backoff = std::cmp::min(1u64 << shift, 60);
                    error!("event processing error (retry in {backoff}s): {e:?}");
                    tokio::time::sleep(std::time::Duration::from_secs(backoff)).await;
                }
            }

            timer.tick().await;
        }
    }

    pub async fn start_webhooks(&self) -> anyhow::Result<()> {
        let interval = std::time::Duration::from_secs(self.settings.opts.webhook_interval);
        let mut timer = tokio::time::interval(interval);

        loop {
            if let Err(e) = self.webhooks.send().await {
                error!("webhook batch send failed: {e}");
            }

            timer.tick().await;
        }
    }

    pub async fn start_notify(&self) -> anyhow::Result<()> {
        let (global_tx, mut global_rx) = tokio::sync::mpsc::unbounded_channel();

        if !self
            .settings
            .triggers
            .iter()
            .any(|(_, t)| matches!(t, Trigger::Notify(_)))
        {
            return futures::future::pending().await;
        }

        let mut producers = vec![];
        let settings = self.settings.clone();

        for (name, trigger) in settings.triggers.clone() {
            if let Trigger::Notify(service) = trigger {
                let timer = service
                    .timer
                    .clone()
                    .unwrap_or_default()
                    .wait
                    .unwrap_or(self.settings.opts.default_timer_wait)
                    as i64;

                let global_tx = global_tx.clone();

                let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

                let service_clone = service.clone();

                producers.push(tokio::spawn(async move { service_clone.watcher(tx).await }));
                producers.push(tokio::spawn(async move {
                    while let Some((path, reason)) = rx.recv().await {
                        if let Err(e) = global_tx.send((
                            name.clone(),
                            path,
                            reason,
                            chrono::Utc::now().naive_utc() + chrono::Duration::seconds(timer),
                        )) {
                            error!("failed to send notify event: {:?}", e);
                        }
                    }

                    Ok::<(), anyhow::Error>(())
                }));
            }
        }

        let manager = Arc::new(self.clone());

        let consumer = async move {
            while let Some((name, path, reason, when_process)) = global_rx.recv().await {
                let new_scan_event = NewScanEvent {
                    event_source: name.clone(),
                    file_path: path.clone(),
                    can_process: when_process,
                    found_status: FoundStatus::Found.into(),
                    ..Default::default()
                };

                match manager.add_event(&new_scan_event) {
                    Err(e) => error!("failed to add notify event: {:?}", e),
                    Ok(_) => {
                        info!(
                            "added 1 {} file from {} trigger",
                            match reason {
                                notify::EventKind::Create(_) => "created",
                                notify::EventKind::Modify(_) => "modified",
                                notify::EventKind::Remove(_) => "removed",
                                notify::EventKind::Access(_) => "accessed",
                                notify::EventKind::Any | notify::EventKind::Other => "changed",
                            },
                            name,
                        );

                        debug!("file '{}' added from '{}' trigger", path, name);
                    }
                }

                manager
                    .webhooks
                    .add_event(EventType::New, Some(name.clone()), &[path])
                    .await;
            }

            Ok::<(), anyhow::Error>(())
        };

        select! {
            res = futures::future::join_all(producers) => {
                for r in res {
                    r.map_err(|e| anyhow::anyhow!("notify producer task failed: {:?}", e))??;
                }
            },
            _ = consumer => {
                warn!("notify consumer exited unexpectedly");
            }
        }

        Ok(())
    }
}
