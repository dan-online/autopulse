use chrono::Timelike;

pub fn get_timestamp() -> String {
    chrono::Local::now()
        .with_nanosecond(0)
        .unwrap_or_default()
        .to_string()
}
