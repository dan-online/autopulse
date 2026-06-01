use crate::manager::PulseManager;
use crate::settings::Settings;
use autopulse_database::conn::{get_conn, get_pool};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn unique_db_url(scope: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("sqlite:///tmp/autopulse-service-{scope}-{nanos}-{seq}.db")
}

pub fn fresh_manager(scope: &str) -> PulseManager {
    let url = unique_db_url(scope);
    let mut settings = Settings::default();
    settings.app.database_url = url.clone();
    let pool = get_pool(&url).expect("test database pool should initialize");
    get_conn(&pool)
        .expect("test database connection should initialize")
        .migrate()
        .expect("test database migrations should apply");
    PulseManager::new(settings, pool)
}
