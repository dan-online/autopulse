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

impl Into<String> for FoundStatus {
    fn into(self) -> String {
        match self {
            FoundStatus::Found => "found",
            FoundStatus::HashMismatch => "hash_mismatch",
            FoundStatus::NotFound => "not_found",
        }
        .to_string()
    }
}

impl Into<String> for ProcessStatus {
    fn into(self) -> String {
        match self {
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
    pub id: i32,

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
    pub event_source: String,

    pub file_path: String,
    pub file_hash: Option<String>,

    pub found_status: Option<String>,
}

impl Default for NewScanEvent {
    fn default() -> Self {
        Self {
            event_source: "unknown".to_string(),
            file_path: "unknown".to_string(),
            file_hash: None,
            found_status: None,
        }
    }
}
