use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::Serialize;

#[derive(diesel_derive_enum::DbEnum, Debug, Clone, Copy, Serialize)]
#[ExistingTypePath = "crate::db::schema::sql_types::Processstatus"]
pub enum ProcessStatus {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "complete")]
    Complete,
    #[serde(rename = "retry")]
    Retry,
    #[serde(rename = "failed")]
    Failed,
}

#[derive(diesel_derive_enum::DbEnum, Debug, Clone, Copy, Serialize)]
#[ExistingTypePath = "crate::db::schema::sql_types::Foundstatus"]
pub enum FoundStatus {
    #[serde(rename = "found")]
    Found,
    #[serde(rename = "not_found")]
    NotFound,
    #[serde(rename = "hash_mismatch")]
    HashMismatch,
}

#[derive(Queryable, Selectable, Serialize, Clone, Debug, AsChangeset, Identifiable)]
#[diesel(table_name = crate::db::schema::scan_events)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ScanEvent {
    pub id: i32,

    pub event_source: String,
    pub event_timestamp: NaiveDateTime,

    pub file_path: String,
    pub file_hash: Option<String>,
    pub process_status: ProcessStatus,
    pub found_status: FoundStatus,

    pub failed_times: i32,
    pub next_retry_at: Option<chrono::NaiveDateTime>,

    pub targets_hit: Vec<String>,

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
}

impl Default for NewScanEvent {
    fn default() -> Self {
        Self {
            event_source: "unknown".to_string(),
            file_path: "unknown".to_string(),
            file_hash: None,
        }
    }
}
