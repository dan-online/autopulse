use super::{transport, EventType, WebhookBatch};
use autopulse_utils::sify;
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

#[derive(Serialize, Deserialize, Clone)]
pub struct DiscordWebhook {
    /// Webhook URL
    pub url: String,
    /// Optional avatar URL (default [assets/logo.webp](https://raw.githubusercontent.com/dan-online/autopulse/main/assets/logo.webp))
    pub avatar_url: Option<String>,
    /// Optional username (default: autopulse)
    pub username: Option<String>,
}

impl DiscordWebhook {
    fn truncate_message(message: String, length: usize) -> String {
        if length < 3 || message.len() <= length {
            return message;
        }

        let cut = message.floor_char_boundary(length - 3);
        format!("{}...", &message[..cut])
    }

    fn generate_json(
        &self,
        batch: &[(EventType, Option<String>, Vec<String>)],
    ) -> DiscordEmbedContent {
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
            let timestamp = chrono::Utc::now().to_rfc3339();

            let color = match event {
                EventType::New => 6_061_450,     // grey
                EventType::Found => 52084,       // green
                EventType::Failed => 16_711_680, // red
                EventType::Processed => 39129,   // blue
                EventType::Retrying | EventType::HashMismatch => 16_776_960,
            };

            let title = trigger.clone().map_or_else(
                || {
                    format!(
                        "[{}] - {} file{} {}",
                        event,
                        files.len(),
                        sify(files),
                        event.action()
                    )
                },
                |trigger| {
                    format!(
                        "[{}] - [{}] - {} file{} {}",
                        event,
                        trigger,
                        files.len(),
                        sify(files),
                        event.action()
                    )
                },
            );

            let fields = vec![
                DiscordEmbedField {
                    name: "Timestamp".to_string(),
                    value: timestamp.clone(),
                },
                DiscordEmbedField {
                    name: "Files".to_string(),
                    // value: files.join("\n"),
                    value: Self::truncate_message(files.join("\n"), 1024),
                },
            ];

            let embed = DiscordEmbed {
                color,
                timestamp,
                fields,
                title,
            };

            content.embeds.push(embed);
        }

        content
    }

    pub async fn send(
        &self,
        batch: &WebhookBatch,
        retries: u8,
        timeout_secs: u64,
    ) -> anyhow::Result<()> {
        let mut message_queue = vec![];

        for chunk in batch.chunks(10) {
            let content = self.generate_json(chunk);
            message_queue.push(content);
        }

        transport::shared_sender(std::time::Duration::from_secs(timeout_secs))?
            .send_json(&self.url, &message_queue, retries)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_ascii_under_limit() {
        let input = "short".to_string();
        assert_eq!(DiscordWebhook::truncate_message(input, 10), "short");
    }

    #[test]
    fn truncate_ascii_over_limit() {
        let input = "this is a long message".to_string();
        let result = DiscordWebhook::truncate_message(input, 10);
        assert_eq!(result, "this is...");
        assert_eq!(result.len(), 10);
    }

    #[test]
    fn truncate_multibyte_at_boundary() {
        // '😀' is 4 bytes; cutting mid-emoji must not panic
        let input = "abcde😀fgh".to_string();
        // length=8 → cut at 5, but byte 5 is inside the 4-byte emoji (bytes 5..9)
        // floor_char_boundary(5) should back up to byte 5 which is the start of 😀
        let result = DiscordWebhook::truncate_message(input, 8);
        assert!(result.ends_with("..."));
        assert!(result.is_char_boundary(result.len()));
    }

    #[test]
    fn truncate_empty_string() {
        let result = DiscordWebhook::truncate_message(String::new(), 10);
        assert_eq!(result, "");
    }

    #[test]
    fn truncate_exactly_at_limit() {
        let input = "exactly 10".to_string();
        assert_eq!(input.len(), 10);
        assert_eq!(DiscordWebhook::truncate_message(input, 10), "exactly 10");
    }
}
