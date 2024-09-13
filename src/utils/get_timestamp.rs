use chrono::Timelike;

pub fn get_timestamp() -> String {
    chrono::Local::now().with_nanosecond(0).unwrap().to_string()
}
