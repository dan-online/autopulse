use crate::settings::webhooks::EventType;
use crate::{
    settings::webhooks::WebhookManager,
    settings::{targets::TargetProcess, Settings},
};
use autopulse_database::{
    conn::{get_conn, DbPool},
    diesel::{self, BoolExpressionMethods, ExpressionMethods, QueryDsl, RunQueryDsl},
    models::{FoundStatus, ProcessStatus, ScanEvent},
    schema::scan_events::{
        can_process, dsl::scan_events, found_at, found_status, next_retry_at, process_status,
    },
};
use autopulse_utils::sha256checksum;
use autopulse_utils::sify;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Mutex;
use tracing::{error, info, info_span, warn, Instrument};

pub(super) struct PulseRunner {
    webhooks: Arc<WebhookManager>,
    settings: Arc<Settings>,
    pool: Arc<DbPool>,

    anchors_available: Arc<Mutex<bool>>,
}

impl PulseRunner {
    pub fn new(settings: Arc<Settings>, pool: Arc<DbPool>, webhooks: Arc<WebhookManager>) -> Self {
        Self {
            webhooks,
            settings,
            pool,
            anchors_available: Arc::new(Mutex::new(true)),
        }
    }

    async fn update_found_status(&self) -> anyhow::Result<()> {
        if !self.settings.opts.check_path {
            return Ok(());
        }

        let mut found_files: Vec<(String, String)> = vec![];
        let mut mismatched_files = vec![];

        let mut evs = scan_events
            .filter(found_status.ne::<String>(FoundStatus::Found.into()))
            .filter(process_status.eq::<String>(ProcessStatus::Pending.into()))
            .load::<ScanEvent>(&mut get_conn(&self.pool)?)?;

        for ev in &mut evs {
            let file_path = PathBuf::from(&ev.file_path);

            if file_path.exists() {
                if let Some(hash) = ev.file_hash.clone() {
                    let file_hash = sha256checksum(&file_path)?;

                    if hash != file_hash {
                        if ev.found_status != FoundStatus::HashMismatch.to_string() {
                            mismatched_files.push(ev.file_path.clone());
                        }

                        ev.found_status = FoundStatus::HashMismatch.into();
                        ev.found_at = Some(chrono::Utc::now().naive_utc());
                    } else {
                        ev.found_status = FoundStatus::Found.into();
                        found_files.push((ev.file_path.clone(), ev.event_source.clone()));
                    }
                } else {
                    ev.found_at = Some(chrono::Utc::now().naive_utc());

                    ev.found_status = FoundStatus::Found.into();

                    found_files.push((ev.file_path.clone(), ev.event_source.clone()));
                }
            }

            ev.updated_at = chrono::Utc::now().naive_utc();
            get_conn(&self.pool)?.save_changes(ev)?;
        }

        if !found_files.is_empty() {
            info!("found {} new file{}", found_files.len(), sify(&found_files));

            for (file, target) in found_files {
                self.webhooks
                    .add_event(EventType::Found, Some(target), &[file])
                    .await;
            }
        }

        if !mismatched_files.is_empty() {
            warn!(
                "found {} mismatched file{}",
                mismatched_files.len(),
                sify(&mismatched_files)
            );

            self.webhooks
                .add_event(EventType::HashMismatch, None, &mismatched_files)
                .await;
        }

        Ok(())
    }

    pub async fn update_process_status(&self) -> anyhow::Result<()> {
        let base_query = scan_events
            .filter(process_status.ne::<String>(ProcessStatus::Complete.into()))
            .filter(process_status.ne::<String>(ProcessStatus::Failed.into()))
            .filter(
                next_retry_at
                    .is_null()
                    .or(next_retry_at.lt(chrono::Utc::now().naive_utc())),
            )
            // filter by processable events
            .filter(can_process.lt(chrono::Utc::now().naive_utc()));

        let mut evs = if self.settings.opts.check_path {
            base_query
                .filter(found_status.eq::<String>(FoundStatus::Found.into()))
                .load::<ScanEvent>(&mut get_conn(&self.pool)?)?
        } else {
            base_query.load::<ScanEvent>(&mut get_conn(&self.pool)?)?
        };

        if evs.is_empty() {
            return Ok(());
        }

        let (processed, retrying, failed) = self.process_events(&mut evs).await?;

        if !processed.is_empty() {
            info!(
                "sent {} file{} to targets",
                processed.len(),
                sify(&processed)
            );

            for ev in &processed {
                self.webhooks
                    .add_event(
                        EventType::Processed,
                        Some(ev.event_source.clone()),
                        &[ev.file_path.clone()],
                    )
                    .await;
            }
        }

        if !retrying.is_empty() {
            warn!("retrying {} file{}", retrying.len(), sify(&retrying));

            for ev in &retrying {
                self.webhooks
                    .add_event(
                        EventType::Retrying,
                        Some(ev.event_source.clone()),
                        &[ev.file_path.clone()],
                    )
                    .await;
            }
        }

        if !failed.is_empty() {
            error!(
                "failed to send {} file{} to targets",
                failed.len(),
                sify(&failed)
            );

            for ev in &failed {
                self.webhooks
                    .add_event(
                        EventType::Failed,
                        Some(ev.event_source.clone()),
                        &[ev.file_path.clone()],
                    )
                    .await;
            }
        }

        Ok(())
    }

    async fn process_events(
        &self,
        evs: &mut [ScanEvent],
    ) -> anyhow::Result<(Vec<ScanEvent>, Vec<ScanEvent>, Vec<ScanEvent>)> {
        let mut failed_ids = vec![];

        let trigger_settings = &self.settings.triggers;

        for (name, target) in &self.settings.targets {
            let evs = evs
                .iter_mut()
                .filter(|x| !x.get_targets_hit().contains(name))
                .filter(|x| {
                    trigger_settings
                        .get(&x.event_source)
                        .is_none_or(|trigger| !trigger.excludes().contains(name))
                })
                .collect::<Vec<&mut ScanEvent>>();

            let res = target
                .process(
                    // TODO: Somehow clean this up
                    evs.iter()
                        .map(|x| &**x)
                        .collect::<Vec<&ScanEvent>>()
                        .as_slice(),
                )
                .instrument(info_span!("process ", target = name))
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
                    failed_ids.extend(evs.iter().map(|x| x.id.clone()));

                    error!("failed to process target '{}': {:?}", name, e);
                }
            }
        }

        let mut succeeded = vec![];
        let mut retrying = vec![];
        let mut failed = vec![];

        for ev in evs.iter_mut() {
            ev.updated_at = chrono::Utc::now().naive_utc();

            let mut conn = get_conn(&self.pool)?;

            if failed_ids.contains(&ev.id) {
                ev.failed_times += 1;

                if ev.failed_times >= self.settings.opts.max_retries {
                    ev.process_status = ProcessStatus::Failed.into();
                    ev.next_retry_at = None;
                    failed.push(conn.save_changes(ev)?);
                } else {
                    let next_retry = chrono::Utc::now().naive_utc()
                        + chrono::Duration::seconds(2_i64.pow(ev.failed_times as u32 + 1));

                    ev.process_status = ProcessStatus::Retry.into();
                    ev.next_retry_at = Some(next_retry);

                    retrying.push(conn.save_changes(ev)?);
                }
            } else {
                ev.process_status = ProcessStatus::Complete.into();
                ev.processed_at = Some(chrono::Utc::now().naive_utc());
                succeeded.push(conn.save_changes(ev)?);
            }
        }

        Ok((succeeded, retrying, failed))
    }

    fn cleanup(&self) -> anyhow::Result<()> {
        let time_before_cleanup = chrono::Utc::now().naive_utc()
            - chrono::Duration::days(self.settings.opts.cleanup_days as i64);

        let delete_not_found = diesel::delete(
            scan_events
                .filter(found_status.eq::<String>(FoundStatus::NotFound.into()))
                .filter(found_at.lt(time_before_cleanup)),
        );

        if let Err(e) = delete_not_found.execute(&mut get_conn(&self.pool)?) {
            error!("failed to delete not found events: {:?}", e);
        }

        let delete_failed = diesel::delete(
            scan_events
                .filter(process_status.eq::<String>(ProcessStatus::Failed.into()))
                .filter(found_at.lt(time_before_cleanup)),
        );

        if let Err(e) = delete_failed.execute(&mut get_conn(&self.pool)?) {
            error!("failed to delete failed events: {:?}", e);
        }

        Ok(())
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let set_anchors_available = self.settings.anchors.iter().all(|anchor| anchor.exists());

        let mut anchors_available = self.anchors_available.lock().await;
        if set_anchors_available != *anchors_available {
            if set_anchors_available {
                info!("anchors are available again, continuing");
            } else {
                warn!("anchors are not available, pausing");
            }
            *anchors_available = set_anchors_available;
        }

        if !*anchors_available {
            return Ok(());
        }

        drop(anchors_available);

        self.update_found_status().await?;
        self.update_process_status().await?;
        self.cleanup()?;

        Ok(())
    }
}
