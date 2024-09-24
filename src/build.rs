use std::env;

fn main() {
    let postgres_enabled = env::var("CARGO_FEATURE_POSTGRES").is_ok();
    let sqlite_enabled = env::var("CARGO_FEATURE_SQLITE").is_ok();

    if !postgres_enabled && !sqlite_enabled {
        panic!("You must enable at least one of the `postgres` or `sqlite` features.");
    }
}
