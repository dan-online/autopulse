use serde::Deserialize;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Deserialize, Clone)]
pub struct Timer {
    #[serde(skip)]
    #[doc(hidden)]
    last_tick: Arc<Mutex<chrono::DateTime<chrono::Utc>>>,

    /// Time to wait before processing events (default: [opts.default_timer_wait](super::settings::Opts::default_timer_wait))
    wait: Option<u64>,
}

impl Default for Timer {
    fn default() -> Self {
        Self::new()
    }
}

#[doc(hidden)]
impl Timer {
    pub fn new() -> Self {
        Self {
            last_tick: Arc::new(Mutex::new(chrono::Utc::now())),
            wait: None,
        }
    }

    pub fn tick(&self) {
        let mut last_tick = self.last_tick.lock().unwrap();
        *last_tick = chrono::Utc::now();
    }

    pub fn can_tick(&self, default: u64) -> bool {
        let buffer_time = Duration::from_secs(self.wait.unwrap_or(default));

        *self.last_tick.lock().unwrap() + buffer_time < chrono::Utc::now()
    }
}
