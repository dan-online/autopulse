use std::path::PathBuf;

use crate::{
    db::{
        models::{FoundStatus, NewScanEvent, ScanEvent},
        schema::{
            self,
            scan_events::{
                dsl::scan_events, found_at, found_status, next_retry_at, process_status,
            },
        },
    },
    service::webhooks::WebhookManager,
    utils::settings::Settings,
    DbPool,
};
use diesel::{
    BoolExpressionMethods, ExpressionMethods, QueryDsl, RunQueryDsl, SaveChangesDsl,
    SelectableHelper,
};
use tracing::{error, info};
use webhooks::EventType;

pub mod targets;
pub mod triggers;
pub mod webhooks;

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
    pub fn new(settings: Settings, pool: DbPool, webhooks: WebhookManager) -> Self {
        Self {
            settings,
            pool,
            webhooks,
        }
    }

    fn get_conn(
        &self,
    ) -> diesel::r2d2::PooledConnection<diesel::r2d2::ConnectionManager<diesel::PgConnection>> {
        self.pool
            .get()
            .expect("Failed to get database connection from pool")
    }

    async fn update_found_status(&self) -> anyhow::Result<()> {
        if !self.settings.check_path {
            return Ok(());
        }

        let mut count = vec![];

        let mut conn = self.get_conn();
        let mut evs = scan_events
            .filter(found_status.ne(FoundStatus::Found))
            .load::<ScanEvent>(&mut conn)?;

        for ev in evs.iter_mut() {
            let file_path = PathBuf::from(&ev.file_path);

            if file_path.exists() {
                let file_hash = crate::utils::checksum::sha256checksum(&file_path);

                ev.found_status = FoundStatus::Found;

                if let Some(hash) = ev.file_hash.clone() {
                    if hash != file_hash {
                        ev.found_status = FoundStatus::HashMismatch;
                        ev.found_at = Some(chrono::Utc::now().naive_utc());
                    }
                } else {
                    ev.found_at = Some(chrono::Utc::now().naive_utc());
                    count.push(ev.file_path.clone());
                }
            }

            ev.updated_at = chrono::Utc::now().naive_utc();
            ev.save_changes::<ScanEvent>(&mut conn)?;
        }

        if !count.is_empty() {
            info!(
                "found {} new file{}",
                count.len(),
                if count.len() > 1 { "s" } else { "" }
            );

            self.webhooks.send(EventType::Found, None, count).await;
        }

        Ok(())
    }

    pub async fn update_process_status(&self) -> anyhow::Result<()> {
        let mut processed = vec![];
        let mut failed = vec![];

        let mut conn = self.get_conn();
        let mut evs = {
            let base_query = scan_events
                .filter(process_status.ne(crate::db::models::ProcessStatus::Complete))
                .filter(process_status.ne(crate::db::models::ProcessStatus::Failed))
                .filter(
                    next_retry_at
                        .is_null()
                        .or(next_retry_at.lt(chrono::Utc::now().naive_utc())),
                );

            if self.settings.check_path {
                base_query
                    .filter(found_status.eq(FoundStatus::Found))
                    .load::<ScanEvent>(&mut conn)?
            } else {
                base_query.load::<ScanEvent>(&mut conn)?
            }
        };

        for ev in evs.iter_mut() {
            let res = self.process_event(ev).await;

            if let Err(e) = res {
                error!("unable to process event: {:?}", e);
                ev.failed_times += 1;

                if ev.failed_times > self.settings.max_retries {
                    ev.process_status = crate::db::models::ProcessStatus::Failed;
                    ev.next_retry_at = None;
                    failed.push(ev.file_path.clone());
                } else {
                    let next_retry = chrono::Utc::now().naive_utc()
                        + chrono::Duration::seconds(2_i64.pow(ev.failed_times as u32 + 1));

                    ev.process_status = crate::db::models::ProcessStatus::Retry;
                    ev.next_retry_at = Some(next_retry);
                }
            } else {
                ev.process_status = crate::db::models::ProcessStatus::Complete;
                processed.push(ev.file_path.clone());
            }

            ev.updated_at = chrono::Utc::now().naive_utc();
            ev.save_changes::<ScanEvent>(&mut conn)?;
        }

        if !processed.is_empty() {
            info!(
                "sent {} file{} to targets",
                processed.len(),
                if processed.len() > 1 { "s" } else { "" }
            );

            self.webhooks
                .send(EventType::Processed, None, processed)
                .await;
        }

        if !failed.is_empty() {
            error!(
                "failed to send {} file{} to targets",
                failed.len(),
                if failed.len() > 1 { "s" } else { "" }
            );

            self.webhooks.send(EventType::Error, None, failed).await;
        }

        Ok(())
    }

    async fn process_event(&self, ev: &ScanEvent) -> anyhow::Result<()> {
        let futures = self
            .settings
            .targets
            .values()
            .map(|target| target.process(ev))
            .collect::<Vec<_>>();

        futures::future::try_join_all(futures).await?;

        Ok(())
    }

    async fn cleanup(&self) -> anyhow::Result<()> {
        let mut conn = self.get_conn();

        // TODO: make this a setting
        let time_before_cleanup = chrono::Utc::now().naive_utc() - chrono::Duration::days(10);

        let _ = diesel::delete(
            scan_events
                .filter(found_status.eq(crate::db::models::FoundStatus::NotFound))
                .filter(found_at.lt(time_before_cleanup)),
        );

        let _ = diesel::delete(
            scan_events
                .filter(process_status.eq(crate::db::models::ProcessStatus::Failed))
                .filter(found_at.lt(time_before_cleanup)),
        )
        .execute(&mut conn)?;

        Ok(())
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        self.update_found_status().await?;
        self.update_process_status().await?;
        self.cleanup().await?;

        Ok(())
    }
}

impl PulseService {
    pub fn new(settings: Settings, pool: DbPool) -> Self {
        Self {
            settings: settings.clone(),
            pool,
            webhooks: WebhookManager::new(settings.clone()),
        }
    }

    pub fn get_conn(
        &self,
    ) -> diesel::r2d2::PooledConnection<diesel::r2d2::ConnectionManager<diesel::PgConnection>> {
        self.pool
            .get()
            .expect("Failed to get database connection from pool")
    }

    pub fn add_event(&self, ev: NewScanEvent) -> ScanEvent {
        let mut conn = self.get_conn();

        diesel::insert_into(schema::scan_events::table)
            .values(&ev)
            .returning(ScanEvent::as_returning())
            .get_result::<ScanEvent>(&mut conn)
            .expect("Failed to insert new scan event")
    }

    pub async fn get_event(&self, id: &i32) -> Option<ScanEvent> {
        let mut conn = self.get_conn();

        let res = scan_events.find(id).first::<ScanEvent>(&mut conn);

        match res {
            Ok(ev) => Some(ev),
            Err(_) => None,
        }
    }

    pub fn start(&self) {
        let settings = self.settings.clone();
        let pool = self.pool.clone();
        let webhooks = self.webhooks.clone();

        tokio::spawn(async move {
            let runner = PulseRunner::new(settings, pool, webhooks);
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
