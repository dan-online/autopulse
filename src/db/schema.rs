// @generated automatically by Diesel CLI.

pub mod sql_types {
    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "foundstatus"))]
    pub struct Foundstatus;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "processstatus"))]
    pub struct Processstatus;
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::Processstatus;
    use super::sql_types::Foundstatus;

    scan_events (id) {
        id -> Int4,
        event_source -> Text,
        event_timestamp -> Timestamptz,
        file_path -> Text,
        file_hash -> Nullable<Text>,
        process_status -> Processstatus,
        found_status -> Foundstatus,
        failed_times -> Int4,
        next_retry_at -> Nullable<Timestamptz>,
        targets_hit -> Array<Text>,
        found_at -> Nullable<Timestamptz>,
        processed_at -> Nullable<Timestamptz>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}
