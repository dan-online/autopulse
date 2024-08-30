use crate::utils::settings::Settings;

#[derive(Clone)]
pub struct WebhookManager {
    _settings: Settings,
}

impl WebhookManager {
    pub fn new(settings: Settings) -> Self {
        Self {
            _settings: settings,
        }
    }

    pub fn send(&self, _new_ev: &crate::db::models::ScanEvent) {}
}
