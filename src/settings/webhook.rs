use serde::Deserialize;

use crate::service::webhooks::{discord::DiscordWebhook, WebhookBatch};

/// [Webhooks](crate::service::webhooks) for the service
#[derive(Deserialize, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Webhook {
    Discord(DiscordWebhook),
}

impl Webhook {
    pub async fn send(&self, batch: &WebhookBatch) -> anyhow::Result<()> {
        match self {
            Self::Discord(d) => d.send(batch).await,
        }
    }
}
