use super::{EventType, WebhookBatch};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Clone)]
#[doc(hidden)]
pub struct DiscordEmbedField {
    pub name: String,
    pub value: String,
}

#[derive(Serialize, Clone)]
#[doc(hidden)]
pub struct DiscordEmbed {
    pub color: i32,
    pub timestamp: String,
    pub fields: Vec<DiscordEmbedField>,
    pub title: String,
}

#[derive(Serialize, Clone)]
#[doc(hidden)]
pub struct DiscordEmbedContent {
    pub username: String,
    pub avatar_url: String,
    pub embeds: Vec<DiscordEmbed>,
}

#[derive(Deserialize, Clone)]
pub struct DiscordWebhook {
    /// Webhook URL
    pub url: String,
    /// Optional avatar URL (default [assets/logo.webp](https://raw.githubusercontent.com/dan-online/autopulse/main/assets/logo.webp))
    pub avatar_url: Option<String>,
    /// Optional username (default: autopulse)
    pub username: Option<String>,
}

impl DiscordWebhook {
    fn get_client(&self) -> reqwest::Client {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("failed to build reqwest client")
    }

    fn generate_json(&self, batch: &WebhookBatch) -> DiscordEmbedContent {
        let mut content = DiscordEmbedContent {
            username: self
                .username
                .clone()
                .unwrap_or_else(|| "autopulse".to_string()),
            avatar_url: self.avatar_url.clone().unwrap_or_else(|| {
                "https://raw.githubusercontent.com/dan-online/autopulse/main/assets/logo.webp"
                    .to_string()
            }),
            embeds: vec![],
        };

        for (event, trigger, files) in batch {
            let color = match event {
                EventType::New => 6_061_450,    // grey
                EventType::Found => 52084,      // green
                EventType::Error => 16_711_680, // red
                EventType::Processed => 39129,  // blue
                EventType::Retrying | EventType::HashMismatch => 16_776_960,
            };

            let title = trigger.clone().map_or_else(
                || {
                    format!(
                        "[{}] - {} file{} {}",
                        event,
                        files.len(),
                        if files.len() > 1 { "s" } else { "" },
                        event.action()
                    )
                },
                |trigger| {
                    format!(
                        "[{}] - [{}] - {} file{} {}",
                        event,
                        trigger,
                        files.len(),
                        if files.len() > 1 { "s" } else { "" },
                        event.action()
                    )
                },
            );

            let fields = vec![
                DiscordEmbedField {
                    name: "Timestamp".to_string(),
                    value: chrono::Local::now().to_rfc3339(),
                },
                DiscordEmbedField {
                    name: "Files".to_string(),
                    value: files.join("\n"),
                },
            ];

            let embed = DiscordEmbed {
                color,
                timestamp: chrono::Utc::now().to_rfc3339(),
                fields,
                title,
            };

            content.embeds.push(embed);
        }

        content
    }

    pub async fn send(&self, batch: &WebhookBatch) -> anyhow::Result<()> {
        let mut message_queue = vec![];

        for chunk in batch.chunks(10) {
            let content = self.generate_json(&chunk.to_vec());
            message_queue.push(content);
        }

        for message in message_queue {
            let res = self
                .get_client()
                .post(&self.url)
                .json(&message)
                .send()
                .await
                .map_err(|e| anyhow::anyhow!(e))?;

            if !res.status().is_success() {
                let body = res.text().await.unwrap_or_else(|_| "no body".to_string());
                return Err(anyhow::anyhow!("failed to send webhook: {}", body));
            }
        }

        Ok(())
    }
}
