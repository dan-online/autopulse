use std::path::PathBuf;

use crate::{
    db::{
        models::{FoundStatus, NewScanEvent, ProcessStatus, ScanEvent},
        schema::scan_events::{
            dsl::scan_events, found_at, found_status, next_retry_at, process_status,
        },
    },
    service::webhooks::WebhookManager,
    utils::{
        conn::{get_conn, DbPool},
        settings::Settings,
    },
};
use diesel::{BoolExpressionMethods, ExpressionMethods, QueryDsl, RunQueryDsl};
use serde::Serialize;
use tracing::{error, info};
use webhooks::EventType;

pub mod targets;
pub mod triggers;
pub mod webhooks;

#[derive(Clone, Serialize)]
pub struct Stats {
    total: i64,
    found: i64,
    processed: i64,
    retrying: i64,
    failed: i64,
}

#[derive(Clone)]
pub struct PulseService {
    pub settings: Settings,
    pub pool: DbPool,
    pub webhooks: WebhookManager,
}

struct PulseRunner {
    webhooks: WebhookManager,
    settings: Settings,
    pool: DbPool,
}

impl PulseRunner {
    pub const fn new(settings: Settings, pool: DbPool, webhooks: WebhookManager) -> Self {
        Self {
            webhooks,
            settings,
            pool,
        }
    }

    async fn update_found_status(&self) -> anyhow::Result<()> {
        if !self.settings.opts.check_path {
            return Ok(());
        }

        let mut count = vec![];

        let mut conn = get_conn(&self.pool);
        let mut evs = scan_events
            .filter(found_status.ne::<String>(FoundStatus::Found.into()))
            .load::<ScanEvent>(&mut conn)?;

        for ev in &mut evs {
            let file_path = PathBuf::from(&ev.file_path);

            if file_path.exists() {
                let file_hash = crate::utils::checksum::sha256checksum(&file_path);

                ev.found_status = FoundStatus::Found.into();

                if let Some(hash) = ev.file_hash.clone() {
                    if hash != file_hash {
                        ev.found_status = FoundStatus::HashMismatch.into();
                        ev.found_at = Some(chrono::Utc::now().naive_utc());
                    }
                } else {
                    ev.found_at = Some(chrono::Utc::now().naive_utc());
                    count.push(ev.file_path.clone());
                }
            }

            ev.updated_at = chrono::Utc::now().naive_utc();
            conn.save_changes(ev);
        }

        if !count.is_empty() {
            info!(
                "found {} new file{}",
                count.len(),
                if count.len() > 1 { "s" } else { "" }
            );

            self.webhooks.send(EventType::Found, None, &count).await;
        }

        Ok(())
    }

    pub async fn update_process_status(&mut self) -> anyhow::Result<()> {
        let mut processed = vec![];
        let mut failed = vec![];

        let mut conn = get_conn(&self.pool);
        let base_query = scan_events
            .filter(process_status.ne::<String>(ProcessStatus::Complete.into()))
            .filter(process_status.ne::<String>(ProcessStatus::Failed.into()))
            .filter(
                next_retry_at
                    .is_null()
                    .or(next_retry_at.lt(chrono::Utc::now().naive_utc())),
            );

        let mut evs = if self.settings.opts.check_path {
            base_query
                .filter(found_status.eq::<String>(FoundStatus::Found.into()))
                .load::<ScanEvent>(&mut conn)?
        } else {
            base_query.load::<ScanEvent>(&mut conn)?
        };

        for ev in evs.iter_mut() {
            let res = self.process_event(ev).await;

            if let Ok((succeeded, _)) = &res {
                let mut hit = ev
                    .targets_hit
                    .split(",")
                    .map(|x| x.to_string())
                    .collect::<Vec<String>>();

                hit.append(&mut succeeded.clone());

                ev.targets_hit = hit.join(",");
            }

            if res.is_err() || !res.as_ref().unwrap().1.is_empty() {
                ev.failed_times += 1;

                if ev.failed_times >= self.settings.opts.max_retries {
                    ev.process_status = ProcessStatus::Failed.into();
                    ev.next_retry_at = None;
                    failed.push(ev.file_path.clone());
                } else {
                    let next_retry = chrono::Utc::now().naive_utc()
                        + chrono::Duration::seconds(2_i64.pow(ev.failed_times as u32 + 1));

                    ev.process_status = ProcessStatus::Retry.into();
                    ev.next_retry_at = Some(next_retry);
                }
            } else {
                ev.process_status = ProcessStatus::Complete.into();
                ev.processed_at = Some(chrono::Utc::now().naive_utc());
                processed.push(ev.file_path.clone());
            }

            ev.updated_at = chrono::Utc::now().naive_utc();
            conn.save_changes(ev);
        }

        if !processed.is_empty() {
            info!(
                "sent {} file{} to targets",
                processed.len(),
                if processed.len() > 1 { "s" } else { "" }
            );

            self.webhooks
                .send(EventType::Processed, None, &processed)
                .await;
        }

        if !failed.is_empty() {
            error!(
                "failed to send {} file{} to targets",
                failed.len(),
                if failed.len() > 1 { "s" } else { "" }
            );

            self.webhooks.send(EventType::Error, None, &failed).await;
        }

        Ok(())
    }

    async fn process_event(
        &mut self,
        ev: &ScanEvent,
    ) -> anyhow::Result<(Vec<String>, Vec<String>)> {
        let mut succeeded = vec![];
        let mut failed = vec![];

        for (name, target) in &mut self.settings.targets {
            if !ev.targets_hit.is_empty() && ev.targets_hit.contains(name) {
                continue;
            }

            let res = target.process(ev).await;

            match res {
                Ok(()) => succeeded.push(name.clone()),
                Err(e) => {
                    failed.push(name.clone());
                    error!("failed to process target '{}': {:?}", name, e);
                }
            }
        }

        Ok((succeeded, failed))
    }

    fn cleanup(&self) -> anyhow::Result<()> {
        let mut conn = get_conn(&self.pool);

        let time_before_cleanup = chrono::Utc::now().naive_utc() - chrono::Duration::days(10);

        let _ = diesel::delete(
            scan_events
                .filter(found_status.eq::<String>(FoundStatus::NotFound.into()))
                .filter(found_at.lt(time_before_cleanup)),
        );

        let _ = diesel::delete(
            scan_events
                .filter(process_status.eq::<String>(ProcessStatus::Failed.into()))
                .filter(found_at.lt(time_before_cleanup)),
        )
        .execute(&mut conn)?;

        Ok(())
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        self.update_found_status().await?;
        self.update_process_status().await?;
        self.cleanup()?;

        Ok(())
    }
}

impl PulseService {
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

        // diesel::insert_into(crate::db::schema::scan_events::table)
        //     .values(ev)
        //     .execute(&mut conn)?;

        // scan_events
        //     .find(&ev.id)
        //     .get_result(&mut conn)
        //     .map_err(Into::into)

        conn.insert_and_return(ev)
    }

    pub fn get_event(&self, id: &String) -> Option<ScanEvent> {
        let mut conn = get_conn(&self.pool);

        scan_events.find(id).first::<ScanEvent>(&mut conn).ok()
    }

    pub fn start(&self) {
        let settings = self.settings.clone();
        let pool = self.pool.clone();
        let webhooks = self.webhooks.clone();

        tokio::spawn(async move {
            let mut runner = PulseRunner::new(settings, pool, webhooks);
            let mut timer = tokio::time::interval(std::time::Duration::from_secs(1));

            loop {
                if let Err(e) = runner.run().await {
                    error!("unable to run pulse: {:?}", e);
                }

                timer.tick().await;
            }
        });
    }
}
