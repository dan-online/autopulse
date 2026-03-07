use super::{transport, WebhookBatch};
use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Clone)]
struct JsonWebhookEvent {
    event: String,
    action: String,
    trigger: Option<String>,
    files: Vec<String>,
    file_count: usize,
    timestamp: String,
}

#[derive(Serialize, Clone)]
struct JsonWebhookPayload {
    events: Vec<JsonWebhookEvent>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct JsonWebhook {
    /// Webhook URL
    pub url: String,
}

impl JsonWebhook {
    fn generate_payload(&self, batch: &WebhookBatch) -> JsonWebhookPayload {
        let timestamp = Utc::now().to_rfc3339();
        let events = batch
            .iter()
            .map(|(event, trigger, files)| JsonWebhookEvent {
                event: event.key().to_string(),
                action: event.action().to_string(),
                trigger: trigger.clone(),
                files: files.clone(),
                file_count: files.len(),
                timestamp: timestamp.clone(),
            })
            .collect();

        JsonWebhookPayload { events }
    }

    pub async fn send(
        &self,
        batch: &WebhookBatch,
        retries: u8,
        timeout_secs: u64,
    ) -> anyhow::Result<()> {
        let payload = self.generate_payload(batch);

        transport::shared_sender(std::time::Duration::from_secs(timeout_secs))?
            .send_json(&self.url, &[payload], retries)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::webhooks::EventType;

    #[test]
    fn generate_payload_exposes_stable_event_fields() {
        let webhook = JsonWebhook {
            url: "https://example.com/webhooks/json".to_string(),
        };
        let batch = vec![
            (
                EventType::New,
                Some("sonarr".to_string()),
                vec!["/media/tv/show-01.mkv".to_string()],
            ),
            (
                EventType::HashMismatch,
                None,
                vec!["/media/tv/show-02.mkv".to_string()],
            ),
        ];

        let payload = serde_json::to_value(webhook.generate_payload(&batch)).unwrap();
        let events = payload
            .get("events")
            .and_then(serde_json::Value::as_array)
            .unwrap();

        assert_eq!(events.len(), 2);
        assert_eq!(events[0]["event"], "new");
        assert_eq!(events[0]["action"], "added");
        assert_eq!(events[0]["trigger"], "sonarr");
        assert_eq!(
            events[0]["files"],
            serde_json::json!(["/media/tv/show-01.mkv"])
        );
        assert_eq!(events[0]["file_count"], 1);
        assert!(
            chrono::DateTime::parse_from_rfc3339(events[0]["timestamp"].as_str().unwrap()).is_ok()
        );
        assert_eq!(events[1]["event"], "hash_mismatch");
        assert!(events[1]["trigger"].is_null());
    }
}
