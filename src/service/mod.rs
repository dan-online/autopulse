pub mod webhooks;

use std::path::PathBuf;

use crate::{
    db::{
        models::{FoundStatus, NewScanEvent, ScanEvent},
        schema::scan_events::{dsl::scan_events, found_status, process_status},
    },
    service::webhooks::WebhookManager,
    utils::settings::Settings,
    DbPool,
};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl, SaveChangesDsl, SelectableHelper};
use tracing::error;

#[derive(Clone)]
pub struct PulseService {
    pub settings: Settings,
    pub pool: DbPool,
    pub webhooks: WebhookManager,
}

struct PulseRunner {
    settings: Settings,
    pool: DbPool,
}

impl PulseRunner {
    pub fn new(settings: Settings, pool: DbPool) -> Self {
        Self { settings, pool }
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
                }
            }

            ev.updated_at = chrono::Utc::now().naive_utc();
            ev.save_changes::<ScanEvent>(&mut conn)?;
        }

        Ok(())
    }

    pub async fn update_process_status(&self) -> anyhow::Result<()> {
        let mut conn = self.get_conn();
        let mut evs = if self.settings.check_path {
            scan_events
                .filter(process_status.ne(crate::db::models::ProcessStatus::Complete))
                .filter(found_status.eq(FoundStatus::Found))
                .load::<ScanEvent>(&mut conn)?
        } else {
            scan_events
                .filter(process_status.ne(crate::db::models::ProcessStatus::Complete))
                .load::<ScanEvent>(&mut conn)?
        };

        for ev in evs.iter_mut() {
            let res = self.process_event(ev).await;

            if let Err(e) = res {
                error!("Error processing event: {:?}", e);
                ev.process_status = crate::db::models::ProcessStatus::Failed;
            } else {
                ev.process_status = crate::db::models::ProcessStatus::Complete;
            }

            ev.updated_at = chrono::Utc::now().naive_utc();
            ev.save_changes::<ScanEvent>(&mut conn)?;
        }

        Ok(())
    }

    async fn process_event(&self, ev: &mut ScanEvent) -> anyhow::Result<()> {
        for target in self.settings.targets.values() {
            // if target.process(&ev.file_path).await? {
            //     return Ok(());
            // }
            target.process(ev).await?;
        }

        Ok(())
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        self.update_found_status().await?;
        self.update_process_status().await?;

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

        let scan_event = diesel::insert_into(crate::db::schema::scan_events::table)
            .values(&ev)
            .returning(ScanEvent::as_returning())
            .get_result::<ScanEvent>(&mut conn)
            .expect("Failed to insert new scan event");

        self.webhooks.send(&scan_event);

        scan_event
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

        tokio::spawn(async move {
            let runner = PulseRunner::new(settings.clone(), pool.clone());
            let mut timer = tokio::time::interval(std::time::Duration::from_secs(5));

            loop {
                timer.tick().await;

                if let Err(e) = runner.run().await {
                    error!("Error running pulse: {:?}", e);
                }
            }
        });
    }
}
