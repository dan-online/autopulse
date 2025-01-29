use std::env;

fn main() {
    let postgres_enabled = env::var("CARGO_FEATURE_POSTGRES").is_ok();
    let sqlite_enabled = env::var("CARGO_FEATURE_SQLITE").is_ok();

    assert!(!(!postgres_enabled && !sqlite_enabled), "You must enable at least one of the `postgres` or `sqlite` features.")
}
