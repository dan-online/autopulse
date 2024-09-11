use super::webhooks::WebhookManager;
use super::{runner::PulseRunner, webhooks::EventType};
use crate::{
    db::{
        models::{FoundStatus, NewScanEvent, ProcessStatus, ScanEvent},
        schema::scan_events::{dsl::scan_events, found_status, process_status},
    },
    utils::{
        conn::{get_conn, DbPool},
        settings::{Settings, Trigger},
    },
};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error};

#[derive(Clone, Serialize)]
pub struct Stats {
    total: i64,
    found: i64,
    processed: i64,
    retrying: i64,
    failed: i64,
}

#[derive(Clone)]
pub struct PulseManager {
    pub settings: Settings,
    pub pool: DbPool,
    pub webhooks: WebhookManager,
}

impl PulseManager {
    pub fn new(settings: Settings, pool: DbPool) -> Self {
        Self {
            settings: settings.clone(),
            pool,
            webhooks: WebhookManager::new(settings),
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

        conn.insert_and_return(ev)
    }

    pub fn get_event(&self, id: &String) -> Option<ScanEvent> {
        let mut conn = get_conn(&self.pool);

        scan_events.find(id).first::<ScanEvent>(&mut conn).ok()
    }

    pub fn start(&self) -> tokio::task::JoinHandle<()> {
        let settings = self.settings.clone();
        let pool = self.pool.clone();
        let webhooks = self.webhooks.clone();

        tokio::spawn(async move {
            let runner = PulseRunner::new(Arc::new(RwLock::new(settings)), pool, webhooks);
            let mut timer = tokio::time::interval(std::time::Duration::from_secs(1));

            loop {
                if let Err(e) = runner.run().await {
                    error!("unable to run pulse: {:?}", e);
                }

                timer.tick().await;
            }
        })
    }

    pub async fn start_notify(&self) {
        let (global_tx, mut global_rx) = tokio::sync::mpsc::unbounded_channel();

        let settings = self.settings.clone();
        let settings = Arc::new(settings);

        for (name, trigger) in settings.triggers.clone() {
            if let Trigger::Notify(service) = trigger {
                let cloned_name = name.clone();
                let global_tx = global_tx.clone();

                let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

                tokio::spawn(async move {
                    if let Err(e) = service.watcher(tx).await {
                        error!("unable to start notify service '{}': {:?}", cloned_name, e);
                    }
                });

                let settings_clone = settings.clone();

                tokio::spawn(async move {
                    while let Some(file_path) = rx.recv().await {
                        if let Err(e) = global_tx.send((name.clone(), file_path)) {
                            error!("unable to send notify event: {:?}", e);
                        } else {
                            settings_clone.triggers.get(&name).unwrap().tick();
                        }
                    }
                });
            }
        }

        while let Some((name, file_path)) = global_rx.recv().await {
            let new_scan_event = NewScanEvent {
                event_source: name.clone(),
                file_path: file_path.clone(),
                ..Default::default()
            };

            if let Err(e) = self.add_event(&new_scan_event) {
                error!("unable to add notify event: {:?}", e);
            } else {
                debug!("added 1 file from {} trigger", name);
            }

            self.webhooks
                .send(EventType::New, Some(name), &[file_path])
                .await;
        }
    }
}
