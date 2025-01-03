use super::webhooks::WebhookManager;
use super::{runner::PulseRunner, webhooks::EventType};
use crate::db::schema::scan_events::{
    can_process, created_at, event_source, file_path, id, updated_at,
};
use crate::routes::stats::Stats;
use crate::{
    db::{
        conn::{get_conn, DbPool},
        models::{FoundStatus, NewScanEvent, ProcessStatus, ScanEvent},
        schema::scan_events::{dsl::scan_events, found_status, process_status},
    },
    settings::{trigger::Trigger, Settings},
};
use anyhow::Context;
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl, TextExpressionMethods};
use futures_util::future::try_join_all;
use std::sync::Arc;
use tracing::{debug, error};

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

    pub fn get_stats(&self) -> anyhow::Result<Stats> {
        let total = scan_events
            .count()
            .get_result::<i64>(&mut get_conn(&self.pool)?)?;

        let processed = scan_events
            .filter(process_status.eq::<String>(ProcessStatus::Complete.into()))
            .count()
            .get_result::<i64>(&mut get_conn(&self.pool)?)?;

        let retrying = scan_events
            .filter(process_status.eq::<String>(ProcessStatus::Retry.into()))
            .count()
            .get_result::<i64>(&mut get_conn(&self.pool)?)?;

        let failed = scan_events
            .filter(process_status.eq::<String>(ProcessStatus::Failed.into()))
            .count()
            .get_result::<i64>(&mut get_conn(&self.pool)?)?;

        let pending = scan_events
            .filter(process_status.eq::<String>(ProcessStatus::Pending.into()))
            .count()
            .get_result::<i64>(&mut get_conn(&self.pool)?)?;

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
            .into_boxed();

        if ev.found_status == FoundStatus::Found.to_string() {
            check = check.filter(found_status.eq(&ev.found_status));
        }

        if let Ok(existing) = check.first::<ScanEvent>(&mut get_conn(&self.pool)?) {
            let updated = diesel::update(&existing)
                .set((
                    event_source.eq(&ev.event_source),
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

            if sort.starts_with("-") {
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
            query = query.filter(file_path.like(format!("%{}%", search)));
        }

        query
            .limit(limit.into())
            .offset(((page - 1) * (limit as u64)) as i64)
            .load::<ScanEvent>(&mut get_conn(&self.pool)?)
            .map_err(Into::into)
    }

    pub fn start(&self) -> tokio::task::JoinHandle<()> {
        let pool = self.pool.clone();
        let webhooks = self.webhooks.clone();
        let settings = self.settings.clone();

        // spawn a loop to log every 0.5s the pool.state()

        // let pool_clone = pool.clone();
        // tokio::spawn(async move {
        //     let mut timer = tokio::time::interval(std::time::Duration::from_millis(500));

        //     loop {
        //         info!("pool state: {:?}", pool_clone.state());
        //         timer.tick().await;
        //     }
        // });

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

    pub fn start_notify(&self) -> anyhow::Result<tokio::task::JoinHandle<anyhow::Result<()>>> {
        let (global_tx, mut global_rx) = tokio::sync::mpsc::unbounded_channel();

        let settings = self.settings.clone();
        let mut spawners: Vec<tokio::task::JoinHandle<anyhow::Result<()>>> = vec![];

        for (name, trigger) in settings.triggers.clone() {
            if let Trigger::Notify(service) = trigger {
                let cloned_name = name.clone();
                let timer = service
                    .timer
                    .wait
                    .unwrap_or(settings.opts.default_timer_wait) as i64;

                let global_tx = global_tx.clone();

                let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

                let handle = tokio::spawn(async move {
                    service
                        .watcher(tx)
                        .await
                        .context(format!("unable to start notify service '{}'", cloned_name))
                });

                tokio::spawn(async move {
                    while let Some((path, reason)) = rx.recv().await {
                        if let Err(e) = global_tx.send((
                            name.clone(),
                            path,
                            reason,
                            chrono::Utc::now().naive_utc() + chrono::Duration::seconds(timer),
                        )) {
                            error!("unable to send notify event: {:?}", e);
                        }
                    }
                });

                spawners.push(handle);
            }
        }

        let manager = Arc::new(self.clone());

        let global_consumer: tokio::task::JoinHandle<anyhow::Result<()>> =
            tokio::spawn(async move {
                while let Some((name, path, reason, when_process)) = global_rx.recv().await {
                    let new_scan_event = NewScanEvent {
                        event_source: name.clone(),
                        file_path: path.clone(),
                        can_process: when_process,
                        ..Default::default()
                    };

                    if let Err(e) = manager.add_event(&new_scan_event) {
                        error!("unable to add notify event: {:?}", e);
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

                Ok(())
            });

        Ok(tokio::spawn(async move {
            tokio::select! {
                res = global_consumer => {
                    res??;
                }
                res = try_join_all(spawners) => {
                    for r in res? {
                        r?;
                    }
                }
            }

            Ok(())
        }))
    }
}
