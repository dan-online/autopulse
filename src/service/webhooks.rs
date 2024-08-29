use crate::utils::settings::Settings;

#[derive(Clone)]
pub struct WebhookManager {
    settings: Settings,
}

impl WebhookManager {
    pub fn new(settings: Settings) -> Self {
        Self { settings }
    }

    pub fn send(&self, new_ev: &crate::db::models::ScanEvent) {
        println!("Sending webhooks");
    }
}
