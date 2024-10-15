use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::Serialize;
use std::fmt::Display;

use crate::utils::generate_uuid::generate_uuid;

/// The status of a scan event being proccessed by [Targets](crate::service::targets).
#[derive(Serialize)]
pub enum ProcessStatus {
    Pending,
    Complete,
    Retry,
    Failed,
}

/// Whether a file was found or not.
///
/// Note: only used if [opts.check_path](crate::utils::settings::Opts::check_path) is set.
#[derive(Serialize)]
pub enum FoundStatus {
    Found,
    NotFound,
    HashMismatch,
}

impl Display for FoundStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let status = match self {
            Self::Found => "found",
            Self::NotFound => "not_found",
            Self::HashMismatch => "hash_mismatch",
        };

        write!(f, "{status}")
    }
}

impl From<FoundStatus> for String {
    fn from(val: FoundStatus) -> Self {
        val.to_string()
    }
}

impl From<ProcessStatus> for String {
    fn from(val: ProcessStatus) -> Self {
        match val {
            ProcessStatus::Pending => "pending",
            ProcessStatus::Complete => "complete",
            ProcessStatus::Retry => "retry",
            ProcessStatus::Failed => "failed",
        }
        .to_string()
    }
}

/// Represents a scan event.
///
/// A scan event is created when a file is added by [Triggers](crate::service::triggers).
#[derive(Queryable, Selectable, Serialize, Clone, Debug, AsChangeset, Identifiable)]
#[diesel(table_name = crate::db::schema::scan_events)]
pub struct ScanEvent {
    /// The [uuid](crate::utils::generate_uuid::generate_uuid) of the scan event.
    pub id: String,

    /// The name of the Trigger that created the scan event.
    pub event_source: String,
    /// The time the scan event was created.
    pub event_timestamp: NaiveDateTime,

    /// The rewritten path of the file.
    pub file_path: String,
    /// Optional hash of the file.
    pub file_hash: Option<String>,
    /// The status of the scan event being processed.
    pub process_status: String,
    /// The status of the file being found.
    pub found_status: String,

    /// The number of times the scan event has failed. Used for retries and is limited to [opts.max_retries](crate::utils::settings::Opts::max_retries).
    pub failed_times: i32,
    /// The time the scan event will be retried.
    pub next_retry_at: Option<chrono::NaiveDateTime>,

    /// The targets that have been hit by the scan event delimited by a comma.
    pub targets_hit: String,

    /// The time the file was found.
    pub found_at: Option<chrono::NaiveDateTime>,
    /// The time the scan event was processed.
    pub processed_at: Option<chrono::NaiveDateTime>,

    /// The time the scan event was created.
    pub created_at: NaiveDateTime,
    /// The time the scan event was updated.
    pub updated_at: NaiveDateTime,

    /// The time the scan event can be processed.
    pub can_process: NaiveDateTime,
}

impl ScanEvent {
    pub fn get_targets_hit(&self) -> Vec<String> {
        self.targets_hit.split(',').map(|s| s.to_string()).collect()
    }

    pub fn add_target_hit(&mut self, target: &str) {
        let mut targets = self.get_targets_hit();
        targets.push(target.to_string());
        targets.sort();
        targets.dedup();
        self.targets_hit = targets.join(",");
    }
}

#[derive(Insertable)]
#[diesel(table_name = crate::db::schema::scan_events)]
#[doc(hidden)]
pub struct NewScanEvent {
    pub id: String,
    pub event_source: String,

    pub file_path: String,
    pub file_hash: Option<String>,

    pub found_status: String,
    pub can_process: NaiveDateTime,
}

impl Default for NewScanEvent {
    fn default() -> Self {
        Self {
            id: generate_uuid(),
            event_source: "unknown".to_string(),
            file_path: "unknown".to_string(),
            file_hash: None,
            found_status: FoundStatus::NotFound.into(),
            can_process: chrono::Utc::now().naive_utc(),
        }
    }
}
