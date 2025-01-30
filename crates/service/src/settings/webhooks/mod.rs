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

#[doc(hidden)]
pub mod manager;

#[doc(hidden)]
pub use manager::*;

use discord::DiscordWebhook;
use serde::Deserialize;

#[derive(Deserialize, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Webhook {
    Discord(DiscordWebhook),
}

impl Webhook {
    pub async fn send(&self, batch: &WebhookBatch) -> anyhow::Result<()> {
        match self {
            Self::Discord(d) => d.send(batch, 3).await,
        }
    }
}
