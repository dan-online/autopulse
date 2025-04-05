use std::env;

fn main() {
    let postgres_enabled = env::var("CARGO_FEATURE_POSTGRES").is_ok();
    let sqlite_enabled = env::var("CARGO_FEATURE_SQLITE").is_ok();
    let mysql_enabled = env::var("CARGO_FEATURE_MYSQL").is_ok();

    assert!(
        !(!postgres_enabled && !sqlite_enabled && !mysql_enabled),
        "You must enable at least one of the `postgres`, `sqlite`, or `mysql` features."
    )
}
