use std::fmt::Display;

use discord::DiscordWebhook;
use reqwest::Client;
use tracing::error;

use crate::utils::settings::{Settings, Webhook};

pub mod discord;

#[derive(Clone)]
pub enum EventType {
    New,
    Found,
    Error,
    HashMismatch,
    Retrying,
    Processed,
}

impl Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let event = match self {
            Self::New => "NEW",
            Self::Retrying => "RETRY",
            Self::Found => "FOUND",
            Self::Error => "ERROR",
            Self::Processed => "PROCESSED",
            Self::HashMismatch => "HASH MISMATCH",
        };

        write!(f, "{event}")
    }
}

impl EventType {
    fn action(&self) -> String {
        match self {
            Self::New => "added",
            Self::Found => "found",
            Self::Retrying => "retrying",
            Self::Error => "failed",
            Self::Processed => "processed",
            Self::HashMismatch => "mismatched",
        }
        .to_string()
    }
}

#[derive(Clone)]
pub struct WebhookManager {
    settings: Settings,
}

impl WebhookManager {
    pub const fn new(settings: Settings) -> Self {
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
            settings
                .username
                .clone()
                .unwrap_or_else(|| "autopulse".to_string()),
            settings.avatar_url.clone().unwrap_or_default(),
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

    pub async fn send(&self, event: EventType, trigger: Option<String>, files: &[String]) {
        let client = Client::new();

        for (name, webhook) in &self.settings.webhooks {
            let result = match webhook {
                Webhook::Discord(discord) => {
                    self.discord_webhook(
                        &client,
                        discord,
                        event.clone(),
                        trigger.clone(),
                        files.to_owned(),
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
