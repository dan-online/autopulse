use super::webhooks::EventType;
use crate::{
    db::{
        models::{FoundStatus, ProcessStatus, ScanEvent},
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
use std::path::PathBuf;
use tracing::{error, info, warn};

pub(super) struct PulseRunner {
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

        let mut found_files = vec![];
        let mut mismatched_files = vec![];

        let mut conn = get_conn(&self.pool);
        let mut evs = scan_events
            .filter(found_status.ne::<String>(FoundStatus::Found.into()))
            .load::<ScanEvent>(&mut conn)?;

        for ev in &mut evs {
            let file_path = PathBuf::from(&ev.file_path);

            if file_path.exists() {
                let file_hash = crate::utils::checksum::sha256checksum(&file_path);

                if let Some(hash) = ev.file_hash.clone() {
                    if hash != file_hash {
                        if ev.found_status != FoundStatus::HashMismatch.to_string() {
                            mismatched_files.push(ev.file_path.clone());
                        }

                        ev.found_status = FoundStatus::HashMismatch.into();
                        ev.found_at = Some(chrono::Utc::now().naive_utc());
                    } else {
                        ev.found_status = FoundStatus::Found.into();
                    }
                } else {
                    ev.found_at = Some(chrono::Utc::now().naive_utc());
                    found_files.push(ev.file_path.clone());

                    ev.found_status = FoundStatus::Found.into();
                }
            }

            ev.updated_at = chrono::Utc::now().naive_utc();
            conn.save_changes(ev)?;
        }

        if !found_files.is_empty() {
            info!(
                "found {} new file{}",
                found_files.len(),
                if found_files.len() > 1 { "s" } else { "" }
            );

            self.webhooks
                .send(EventType::Found, None, &found_files)
                .await;
        }

        if !mismatched_files.is_empty() {
            warn!(
                "found {} mismatched file{}",
                mismatched_files.len(),
                if mismatched_files.len() > 1 { "s" } else { "" }
            );

            self.webhooks
                .send(EventType::HashMismatch, None, &mismatched_files)
                .await;
        }

        Ok(())
    }

    pub async fn update_process_status(&mut self) -> anyhow::Result<()> {
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

        if evs.is_empty() {
            return Ok(());
        }

        let (processed, retrying, failed) = self.process_events(&mut evs).await?;

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

        if !retrying.is_empty() {
            warn!(
                "retrying {} file{}",
                retrying.len(),
                if retrying.len() > 1 { "s" } else { "" }
            );

            self.webhooks
                .send(EventType::Retrying, None, &retrying)
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

    async fn process_events(
        &mut self,
        evs: &mut [ScanEvent],
    ) -> anyhow::Result<(Vec<String>, Vec<String>, Vec<String>)> {
        let mut failed_ids = vec![];

        for (name, target) in &mut self.settings.targets {
            let evs = evs
                .iter_mut()
                .filter(|x| !x.get_targets_hit().contains(name))
                .collect::<Vec<&mut ScanEvent>>();

            let res = target
                .process(
                    // TODO: Somehow clean this up
                    evs.iter()
                        .map(|x| &**x)
                        .collect::<Vec<&ScanEvent>>()
                        .as_slice(),
                )
                .await;

            match res {
                Ok(s) => {
                    for ev in evs {
                        if s.contains(&ev.id) {
                            ev.add_target_hit(name);
                        } else {
                            failed_ids.push(ev.id.clone());
                        }
                    }
                }
                Err(e) => {
                    error!("failed to process target '{}': {:?}", name, e);
                }
            }
        }

        let mut succeeded = vec![];
        let mut retrying = vec![];
        let mut failed = vec![];

        let mut conn = get_conn(&self.pool);

        for ev in evs.iter_mut() {
            ev.updated_at = chrono::Utc::now().naive_utc();

            if failed_ids.contains(&ev.id) {
                ev.failed_times += 1;

                if ev.failed_times >= self.settings.opts.max_retries {
                    ev.process_status = ProcessStatus::Failed.into();
                    ev.next_retry_at = None;
                    failed.push(conn.save_changes(ev)?.file_path.clone());
                } else {
                    let next_retry = chrono::Utc::now().naive_utc()
                        + chrono::Duration::seconds(2_i64.pow(ev.failed_times as u32 + 1));

                    ev.process_status = ProcessStatus::Retry.into();
                    ev.next_retry_at = Some(next_retry);

                    retrying.push(conn.save_changes(ev)?.file_path.clone());
                }
            } else {
                ev.process_status = ProcessStatus::Complete.into();
                ev.processed_at = Some(chrono::Utc::now().naive_utc());
                succeeded.push(conn.save_changes(ev)?.file_path.clone());
            }
        }

        Ok((succeeded, retrying, failed))
    }

    fn cleanup(&self) -> anyhow::Result<()> {
        let mut conn = get_conn(&self.pool);

        let time_before_cleanup = chrono::Utc::now().naive_utc() - chrono::Duration::days(10);

        let delete_not_found = diesel::delete(
            scan_events
                .filter(found_status.eq::<String>(FoundStatus::NotFound.into()))
                .filter(found_at.lt(time_before_cleanup)),
        );

        if let Err(e) = delete_not_found.execute(&mut conn) {
            error!("unable to delete not found events: {:?}", e);
        }

        let delete_failed = diesel::delete(
            scan_events
                .filter(process_status.eq::<String>(ProcessStatus::Failed.into()))
                .filter(found_at.lt(time_before_cleanup)),
        )
        .execute(&mut conn);

        if let Err(e) = delete_failed {
            error!("unable to delete failed events: {:?}", e);
        }

        Ok(())
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        self.update_found_status().await?;
        self.update_process_status().await?;
        self.cleanup()?;

        Ok(())
    }
}
