use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Timer {
    #[serde(skip)]
    last_tick: Arc<Mutex<chrono::DateTime<chrono::Utc>>>,

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
        }
    }

    pub fn tick(&self) {
        let mut last_tick = self.last_tick.lock().unwrap();
        *last_tick = chrono::Utc::now();
    }

    pub fn can_tick(&self) -> bool {
        let buffer_time = Duration::from_secs(self.wait.unwrap_or(10));

        *self.last_tick.lock().unwrap() + buffer_time < chrono::Utc::now()
    }
}
