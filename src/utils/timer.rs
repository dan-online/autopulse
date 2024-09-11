use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use serde::Deserialize;

use super::settings::Settings;

#[derive(Debug, Deserialize, Clone)]
pub struct Timer {
    #[serde(skip)]
    last_tick: Arc<Mutex<chrono::DateTime<chrono::Utc>>>,

    #[serde(skip)]
    default: u64,

    wait: Option<u64>,
}

impl Default for Timer {
    fn default() -> Self {
        Self::new()
    }
}

impl Timer {
    pub fn new() -> Self {
        Self {
            last_tick: Arc::new(Mutex::new(chrono::Utc::now())),
            wait: None,
            default: Settings::get_settings().unwrap().opts.default_timer_wait,
        }
    }

    pub fn tick(&self) {
        let mut last_tick = self.last_tick.lock().unwrap();
        *last_tick = chrono::Utc::now();
    }

    pub fn can_tick(&self) -> bool {
        let buffer_time = Duration::from_secs(self.wait.unwrap_or(self.default));

        *self.last_tick.lock().unwrap() + buffer_time < chrono::Utc::now()
    }
}
