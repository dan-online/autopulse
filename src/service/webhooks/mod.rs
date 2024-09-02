use std::fmt::Display;

use discord::DiscordWebhook;
use reqwest::Client;
use tracing::error;

use crate::utils::settings::{Settings, Webhook};

pub mod discord;

#[derive(Clone)]
pub struct WebhookManager {
    settings: Settings,
}

#[derive(Clone)]
pub enum EventType {
    New,
    Found,
    Error,
    Processed,
}

impl Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let event = match self {
            EventType::New => "New",
            EventType::Found => "Found",
            EventType::Error => "Error",
            EventType::Processed => "Processed",
        };

        write!(f, "{}", event)
    }
}

impl EventType {
    fn action(&self) -> String {
        match self {
            EventType::New => "added",
            EventType::Found => "found",
            EventType::Error => "failed",
            EventType::Processed => "processed",
        }
        .to_string()
    }
}

impl WebhookManager {
    pub fn new(settings: Settings) -> Self {
        Self { settings }
    }

    pub async fn discord_webhook(
        &self,
        client: &Client,
        settings: &DiscordWebhook,
        event: EventType,
        trigger: Option<String>,
        files: Vec<String>,
    ) -> anyhow::Result<()> {
        let embed = DiscordWebhook::generate_json(
            settings.username.clone().unwrap_or("autopulse".to_string()),
            settings.avatar_url.clone().unwrap_or("".to_string()),
            event,
            trigger,
            files,
        );

        let mut url = url::Url::parse(&settings.url).map_err(|e| anyhow::anyhow!(e))?;

        if !url.path().ends_with("/json") {
            url.set_path(&format!("{}/json", url.path()));
        }

        client
            .post(&settings.url)
            .json(&embed)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!(e))
            .map(|_| ())
    }

    pub async fn send(&self, event: EventType, trigger: Option<String>, files: Vec<String>) {
        let client = Client::new();

        for (name, webhook) in self.settings.webhooks.iter() {
            let result = match webhook {
                Webhook::Discord(discord) => {
                    self.discord_webhook(
                        &client,
                        discord,
                        event.clone(),
                        trigger.clone(),
                        files.clone(),
                    )
                    .await
                }
            };

            if result.is_err() {
                error!("unable to send webhook {}: {:?}", name, result);
            }
        }
    }
}
