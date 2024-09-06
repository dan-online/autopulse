use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::Serialize;

#[derive(Serialize)]
pub enum ProcessStatus {
    Pending,
    Complete,
    Retry,
    Failed,
}

#[derive(Serialize)]
pub enum FoundStatus {
    Found,
    NotFound,
    HashMismatch,
}

impl From<FoundStatus> for String {
    fn from(val: FoundStatus) -> Self {
        match val {
            FoundStatus::Found => "found",
            FoundStatus::HashMismatch => "hash_mismatch",
            FoundStatus::NotFound => "not_found",
        }
        .to_string()
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

#[derive(Queryable, Selectable, Serialize, Clone, Debug, AsChangeset, Identifiable)]
#[diesel(table_name = crate::db::schema::scan_events)]
pub struct ScanEvent {
    pub id: String,

    pub event_source: String,
    pub event_timestamp: NaiveDateTime,

    pub file_path: String,
    pub file_hash: Option<String>,
    pub process_status: String,
    pub found_status: String,

    pub failed_times: i32,
    pub next_retry_at: Option<chrono::NaiveDateTime>,

    pub targets_hit: String,

    pub found_at: Option<chrono::NaiveDateTime>,
    pub processed_at: Option<chrono::NaiveDateTime>,

    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = crate::db::schema::scan_events)]
pub struct NewScanEvent {
    pub id: String,
    pub event_source: String,

    pub file_path: String,
    pub file_hash: Option<String>,

    pub found_status: Option<String>,
}

fn generate_uuid() -> String {
    let uuid = uuid::Uuid::new_v4();
    uuid.to_string()
}

impl Default for NewScanEvent {
    fn default() -> Self {
        Self {
            id: generate_uuid(),
            event_source: "unknown".to_string(),
            file_path: "unknown".to_string(),
            file_hash: None,
            found_status: None,
        }
    }
}
