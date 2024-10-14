use super::webhooks::WebhookManager;
use super::{runner::PulseRunner, webhooks::EventType};
use crate::db::schema::scan_events::{can_process, event_source, file_path, updated_at};
use crate::routes::stats::Stats;
use crate::{
    db::{
        conn::{get_conn, DbPool},
        models::{FoundStatus, NewScanEvent, ProcessStatus, ScanEvent},
        schema::scan_events::{dsl::scan_events, found_status, process_status},
    },
    utils::settings::{Settings, Trigger},
};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use std::sync::Arc;
use tracing::{debug, error};

#[derive(Clone)]
pub struct PulseManager {
    pub settings: Arc<Settings>,
    pub pool: DbPool,
    pub webhooks: Arc<WebhookManager>,
}

impl PulseManager {
    pub fn new(settings: Settings, pool: DbPool) -> Self {
        let settings = Arc::new(settings);

        Self {
            settings: settings.clone(),
            pool,
            webhooks: Arc::new(WebhookManager::new(settings)),
        }
    }

    pub fn get_stats(&self) -> anyhow::Result<Stats> {
        let mut conn = get_conn(&self.pool);

        let total = scan_events.count().get_result::<i64>(&mut conn)?;

        let found = scan_events
            .filter(found_status.eq::<String>(FoundStatus::Found.into()))
            .count()
            .get_result::<i64>(&mut conn)?;

        let processed = scan_events
            .filter(process_status.eq::<String>(ProcessStatus::Complete.into()))
            .count()
            .get_result::<i64>(&mut conn)?;

        let retrying = scan_events
            .filter(process_status.eq::<String>(ProcessStatus::Retry.into()))
            .count()
            .get_result::<i64>(&mut conn)?;

        let failed = scan_events
            .filter(process_status.eq::<String>(ProcessStatus::Failed.into()))
            .count()
            .get_result::<i64>(&mut conn)?;

        Ok(Stats {
            total,
            found,
            processed,
            retrying,
            failed,
        })
    }

    pub fn add_event(&self, ev: &NewScanEvent) -> anyhow::Result<ScanEvent> {
        let mut conn = get_conn(&self.pool);

        // if there is an existing event with the same file path and waiting
        if let Ok(existing) = scan_events
            .filter(file_path.eq(&ev.file_path))
            .filter(process_status.eq::<String>(ProcessStatus::Pending.into()))
            .first::<ScanEvent>(&mut conn)
        {
            let updated = diesel::update(&existing)
                .set((
                    event_source.eq(&ev.event_source),
                    updated_at.eq(chrono::Utc::now().naive_utc()),
                    can_process.eq(ev.can_process),
                ))
                .get_result::<ScanEvent>(&mut conn)?;

            return Ok(updated);
        }

        conn.insert_and_return(ev)
    }

    pub fn get_event(&self, id: &String) -> Option<ScanEvent> {
        let mut conn = get_conn(&self.pool);

        scan_events.find(id).first::<ScanEvent>(&mut conn).ok()
    }

    pub fn start(&self) -> tokio::task::JoinHandle<()> {
        let pool = self.pool.clone();
        let webhooks = self.webhooks.clone();
        let settings = self.settings.clone();

        tokio::spawn(async move {
            let runner = PulseRunner::new(settings, pool, webhooks);
            let mut timer = tokio::time::interval(std::time::Duration::from_secs(1));

            loop {
                if let Err(e) = runner.run().await {
                    error!("unable to run pulse: {:?}", e);
                }

                timer.tick().await;
            }
        })
    }

    pub fn start_webhooks(&self) -> tokio::task::JoinHandle<()> {
        let webhooks = self.webhooks.clone();
        let mut timer = tokio::time::interval(std::time::Duration::from_secs(10));

        tokio::spawn(async move {
            loop {
                if let Err(e) = webhooks.send().await {
                    error!("unable to send webhooks: {:?}", e);
                }

                timer.tick().await;
            }
        })
    }

    pub fn start_notify(&self) -> tokio::task::JoinHandle<()> {
        let (global_tx, mut global_rx) = tokio::sync::mpsc::unbounded_channel();

        let settings = self.settings.clone();

        for (name, trigger) in settings.triggers.clone() {
            if let Trigger::Notify(service) = trigger {
                let cloned_name = name.clone();
                let timer = service
                    .timer
                    .wait
                    .unwrap_or(settings.opts.default_timer_wait) as i64;
                let global_tx = global_tx.clone();

                let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

                tokio::spawn(async move {
                    if let Err(e) = service.watcher(tx).await {
                        error!("unable to start notify service '{}': {:?}", cloned_name, e);
                    }
                });

                tokio::spawn(async move {
                    while let Some(path) = rx.recv().await {
                        if let Err(e) = global_tx.send((
                            name.clone(),
                            path,
                            chrono::Utc::now().naive_utc() + chrono::Duration::seconds(timer),
                        )) {
                            error!("unable to send notify event: {:?}", e);
                        }
                    }
                });
            }
        }

        let manager = Arc::new(self.clone());

        tokio::spawn(async move {
            while let Some((name, path, when_process)) = global_rx.recv().await {
                let new_scan_event = NewScanEvent {
                    event_source: name.clone(),
                    file_path: path.clone(),
                    can_process: when_process,
                    ..Default::default()
                };

                if let Err(e) = manager.add_event(&new_scan_event) {
                    error!("unable to add notify event: {:?}", e);
                } else {
                    debug!("added 1 file from {} trigger", name);
                }

                manager
                    .webhooks
                    .add_event(EventType::New, Some(name.clone()), &[path])
                    .await;
            }
        })
    }
}
