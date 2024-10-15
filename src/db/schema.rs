// @generated automatically by Diesel CLI.

diesel::table! {
    scan_events (id) {
        id -> Text,
        event_source -> Text,
        event_timestamp -> Timestamp,
        file_path -> Text,
        file_hash -> Nullable<Text>,
        process_status -> Text,
        found_status -> Text,
        failed_times -> Integer,
        next_retry_at -> Nullable<Timestamp>,
        targets_hit -> Text,
        found_at -> Nullable<Timestamp>,
        processed_at -> Nullable<Timestamp>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        can_process -> Timestamp,
    }
}
