use serde::Deserialize;

use crate::service::webhooks::{discord::DiscordWebhook, WebhookBatch};

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
