use serde::{Deserialize, Serialize};

use super::EventType;

#[derive(Serialize, Clone, Debug)]
pub struct DiscordEmbedField {
    pub name: String,
    pub value: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct DiscordEmbed {
    pub color: i32,
    pub timestamp: String,
    pub url: String,
    pub fields: Vec<DiscordEmbedField>,
    pub title: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct DiscordEmbedContent {
    pub username: String,
    pub avatar_url: String,
    pub embeds: Vec<DiscordEmbed>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct DiscordWebhook {
    pub url: String,
    pub avatar_url: Option<String>,
    pub username: Option<String>,
}

impl DiscordWebhook {
    pub fn generate_json(
        username: String,
        avatar_url: String,
        event: EventType,
        trigger: Option<String>,
        files: Vec<String>,
    ) -> DiscordEmbedContent {
        let color = match event {
            EventType::New => 6061450,     // grey
            EventType::Found => 52084,     // green
            EventType::Error => 16711680,  // red
            EventType::Processed => 39129, // blue
        };

        let title = if let Some(trigger) = trigger {
            format!(
                "[{}] - [{}] - {} file{} {}",
                event,
                trigger,
                files.len(),
                if files.len() > 1 { "s" } else { "" },
                event.action()
            )
        } else {
            format!(
                "[{}] - {} file{} {}",
                event,
                files.len(),
                if files.len() > 1 { "s" } else { "" },
                event.action()
            )
        };

        let fields = vec![
            DiscordEmbedField {
                name: "Timestamp".to_string(),
                value: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            },
            DiscordEmbedField {
                name: "Files".to_string(),
                value: files.join("\n"),
            },
        ];

        let embed = DiscordEmbed {
            color,
            timestamp: chrono::Utc::now().to_rfc3339(),
            url: "https://discord.com".to_string(),
            fields,
            title,
        };

        DiscordEmbedContent {
            username,
            avatar_url,
            embeds: vec![embed],
        }
    }
}
