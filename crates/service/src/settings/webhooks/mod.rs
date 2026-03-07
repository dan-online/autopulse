/// Discord - Discord Webhook
///
/// Sends a message to a Discord Webhook on events
///
/// # Example
///
/// ```yml
/// webhooks:
///   my_discord:
///     type: discord
///     url: "https://discord.com/api/webhooks/..."
/// ```
///
/// or
///
/// ```yml
/// webhooks:
///   my_discord:
///     type: discord
///     avatar_url: "https://example.com/avatar.png"
///     username: "autopulse"
/// ```
///
/// See [`DiscordWebhook`] for all options
pub mod discord;

/// Hookshot - Matrix Hookshot inbound webhook
///
/// Sends a Matrix Hookshot-compatible JSON body with `text`, generated `html`,
/// and an optional `username`
///
/// # Example
///
/// ```yml
/// webhooks:
///   my_hookshot:
///     type: hookshot
///     url: "https://matrix.example.com/_matrix/hookshot/webhook/..."
/// ```
///
/// See [`HookshotWebhook`] for all options
pub mod hookshot;

/// JSON - generic structured webhook payload
///
/// Sends a stable JSON object with an `events` array for generic webhook
/// consumers
///
/// # Example
///
/// ```yml
/// webhooks:
///   my_json:
///     type: json
///     url: "https://example.com/webhooks/autopulse"
/// ```
///
/// See [`JsonWebhook`] for all options
pub mod json;

#[doc(hidden)]
pub mod manager;

#[doc(hidden)]
pub mod transport;

#[doc(hidden)]
pub use manager::*;

use discord::DiscordWebhook;
use hookshot::HookshotWebhook;
use json::JsonWebhook;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Webhook {
    Discord(DiscordWebhook),
    Hookshot(HookshotWebhook),
    Json(JsonWebhook),
}

impl Webhook {
    pub async fn send(
        &self,
        batch: &WebhookBatch,
        retries: u8,
        timeout_secs: u64,
    ) -> anyhow::Result<()> {
        if batch.is_empty() {
            return Ok(());
        }

        match self {
            Self::Discord(d) => d.send(batch, retries, timeout_secs).await,
            Self::Hookshot(h) => h.send(batch, retries, timeout_secs).await,
            Self::Json(j) => j.send(batch, retries, timeout_secs).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{hookshot::HookshotWebhook, json::JsonWebhook, Webhook};

    #[test]
    fn deserializes_hookshot_webhook_config() {
        let webhook = serde_json::from_value::<Webhook>(serde_json::json!({
            "type": "hookshot",
            "url": "https://example.com/webhooks/hookshot"
        }));

        assert!(webhook.is_ok(), "expected hookshot webhook to deserialize");
    }

    #[test]
    fn deserializes_json_webhook_config() {
        let webhook = serde_json::from_value::<Webhook>(serde_json::json!({
            "type": "json",
            "url": "https://example.com/webhooks/json"
        }));

        assert!(webhook.is_ok(), "expected json webhook to deserialize");
    }

    #[tokio::test]
    async fn skips_sending_empty_batches() {
        let hookshot = Webhook::Hookshot(HookshotWebhook {
            url: "http://127.0.0.1:9/hookshot".to_string(),
            username: None,
        });
        let json = Webhook::Json(JsonWebhook {
            url: "http://127.0.0.1:9/json".to_string(),
        });

        hookshot.send(&Vec::new(), 3, 10).await.unwrap();
        json.send(&Vec::new(), 3, 10).await.unwrap();
    }
}
