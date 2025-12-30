use super::runner::PulseRunner;
use crate::settings::triggers::Trigger;
use crate::settings::webhooks::{EventType, WebhookManager};
use crate::settings::Settings;
use anyhow::Context;
use autopulse_database::diesel::sql_types::BigInt;
use autopulse_database::diesel::QueryableByName;
use autopulse_database::schema::scan_events::{
    can_process, created_at, event_source, file_path, id, updated_at,
};
use autopulse_database::{
    conn::{get_conn, DbPool},
    diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl, TextExpressionMethods},
    models::{FoundStatus, NewScanEvent, ProcessStatus, ScanEvent},
    schema::scan_events::{dsl::scan_events, found_status, process_status},
};
use serde::Serialize;
use std::sync::Arc;
use tokio::select;
use tokio::task::JoinHandle;
use tracing::{debug, error};

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

#[derive(Clone)]
pub struct PulseManager {
    pub settings: Arc<Settings>,
    pub pool: Arc<DbPool>,
    pub webhooks: Arc<WebhookManager>,
}

impl PulseManager {
    pub fn new(settings: Settings, pool: DbPool) -> Self {
        let settings = Arc::new(settings);
        let pool = Arc::new(pool);
        let webhooks = Arc::new(WebhookManager::new(settings.clone()));

        Self {
            settings,
            pool,
            webhooks,
        }
    }

    //  pub fn get_stats(&self) -> anyhow::Result<Stats> {
    //     let stats = sql_query(
    //         "SELECT
    //             COUNT(*) as total,
    //             COALESCE(SUM(CASE WHEN process_status = 'complete' THEN 1 ELSE 0 END), 0) as processed,
    //             COALESCE(SUM(CASE WHEN process_status = 'retry' THEN 1 ELSE 0 END), 0) as retrying,
    //             COALESCE(SUM(CASE WHEN process_status = 'failed' THEN 1 ELSE 0 END), 0) as failed,
    //             COALESCE(SUM(CASE WHEN process_status = 'pending' THEN 1 ELSE 0 END), 0) as pending
    //         FROM scan_events",
    //     )
    //     .get_result::<Stats>(&mut get_conn(&self.pool)?)?;

    //     Ok(stats)
    // }

    pub fn get_stats(&self) -> anyhow::Result<Stats> {
        let conn = &mut get_conn(&self.pool)?;

        let total = scan_events.count().get_result::<i64>(conn)?;

        let processed = scan_events
            .filter(process_status.eq::<String>(ProcessStatus::Complete.into()))
            .count()
            .get_result::<i64>(conn)?;

        let retrying = scan_events
            .filter(process_status.eq::<String>(ProcessStatus::Retry.into()))
            .count()
            .get_result::<i64>(conn)?;

        let failed = scan_events
            .filter(process_status.eq::<String>(ProcessStatus::Failed.into()))
            .count()
            .get_result::<i64>(conn)?;

        let pending = scan_events
            .filter(process_status.eq::<String>(ProcessStatus::Pending.into()))
            .count()
            .get_result::<i64>(conn)?;

        Ok(Stats {
            total,
            processed,
            retrying,
            failed,
            pending,
        })
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

        if let Ok(existing) = check.first::<ScanEvent>(&mut get_conn(&self.pool)?) {
            let updated = diesel::update(&existing)
                .set((
                    updated_at.eq(chrono::Utc::now().naive_utc()),
                    can_process.eq(ev.can_process),
                ))
                .get_result::<ScanEvent>(&mut get_conn(&self.pool)?)?;

            return Ok(updated);
        }

        get_conn(&self.pool)?.insert_and_return(ev)
    }

    pub fn get_event(&self, ev_id: &String) -> anyhow::Result<Option<ScanEvent>> {
        Ok(scan_events
            .find(ev_id)
            .first::<ScanEvent>(&mut get_conn(&self.pool)?)
            .ok())
    }

    pub fn get_events(
        &self,
        mut limit: u8,
        page: u64,
        sort: Option<String>,
        status: Option<String>,
        search: Option<String>,
    ) -> anyhow::Result<Vec<ScanEvent>> {
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
            query = query.filter(file_path.like(format!("%{search}%")));
        }

        query
            .limit(limit.into())
            .offset(((page - 1) * u64::from(limit)) as i64)
            .load::<ScanEvent>(&mut get_conn(&self.pool)?)
            .map_err(Into::into)
    }

    pub async fn spawn(&self) -> anyhow::Result<()> {
        let handles = vec![
            self.spawn_start(),
            self.spawn_webhooks(),
            self.spawn_notify(),
        ];

        futures::future::select_all(handles).await.0?
    }

    fn spawn_start(&self) -> JoinHandle<anyhow::Result<()>> {
        let manager = Arc::new(self.clone());

        tokio::spawn(async move {
            manager
                .start()
                .await
                .context("failed to handle scan events")
        })
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        let runner = PulseRunner::new(
            self.settings.clone(),
            self.pool.clone(),
            self.webhooks.clone(),
        );
        let mut timer = tokio::time::interval(std::time::Duration::from_secs(1));

        loop {
            runner.run().await?;

            timer.tick().await;
        }
    }

    fn spawn_webhooks(&self) -> JoinHandle<anyhow::Result<()>> {
        let manager = Arc::new(self.clone());

        tokio::spawn(async move {
            manager
                .start_webhooks()
                .await
                .context("failed to send webhooks")
        })
    }

    pub async fn start_webhooks(&self) -> anyhow::Result<()> {
        let mut timer = tokio::time::interval(std::time::Duration::from_secs(10));

        loop {
            self.webhooks.send().await?;

            timer.tick().await;
        }
    }

    fn spawn_notify(&self) -> JoinHandle<anyhow::Result<()>> {
        let manager = Arc::new(self.clone());

        tokio::spawn(async move {
            manager
                .start_notify()
                .await
                .context("failed to handle notify triggers")
        })
    }

    pub async fn start_notify(&self) -> anyhow::Result<()> {
        let (global_tx, mut global_rx) = tokio::sync::mpsc::unbounded_channel();

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
                    ..Default::default()
                };

                if let Err(e) = manager.add_event(&new_scan_event) {
                    error!("failed to add notify event: {:?}", e);
                } else {
                    debug!(
                        "added 1 file from {} trigger due to: {}",
                        name,
                        match reason {
                            notify::EventKind::Create(_) => "create",
                            notify::EventKind::Modify(_) => "modify",
                            notify::EventKind::Remove(_) => "remove",
                            notify::EventKind::Access(_) => "access",
                            notify::EventKind::Any => "any",
                            notify::EventKind::Other => "other",
                        }
                    );
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
            _ = consumer => {}
        }

        Err(anyhow::anyhow!("notify manager exited unexpectedly"))
    }
}
