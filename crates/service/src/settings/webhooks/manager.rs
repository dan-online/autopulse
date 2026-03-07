use crate::settings::Settings;
use futures::future::join_all;
use std::{collections::HashMap, fmt::Display, sync::Arc};
use tokio::sync::RwLock;
use tracing::error;

pub type WebhookBatch = Vec<(EventType, Option<String>, Vec<String>)>;
type WebhookQueue = HashMap<(EventType, Option<String>), Vec<String>>;

/// Event type
#[derive(Clone, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub enum EventType {
    /// New event
    New = 0,
    /// Hash mismatch
    HashMismatch = 1,
    /// Found file
    Found = 2,
    /// Retrying event
    Retrying = 3,
    /// Processed event
    Processed = 4,
    /// Failed event
    Failed = 5,
}

impl Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let event = match self {
            Self::New => "NEW",
            Self::Retrying => "RETRY",
            Self::Found => "FOUND",
            Self::Failed => "FAILED",
            Self::Processed => "PROCESSED",
            Self::HashMismatch => "HASH MISMATCH",
        };

        write!(f, "{event}")
    }
}

impl EventType {
    pub const fn key(&self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Found => "found",
            Self::Retrying => "retrying",
            Self::Failed => "failed",
            Self::Processed => "processed",
            Self::HashMismatch => "hash_mismatch",
        }
    }

    pub const fn action(&self) -> &'static str {
        match self {
            Self::New => "added",
            Self::Found => "found",
            Self::Retrying => "retrying",
            Self::Failed => "failed",
            Self::Processed => "processed",
            Self::HashMismatch => "mismatched",
        }
    }
}

#[derive(Clone)]
pub struct WebhookManager {
    settings: Arc<Settings>,
    queue: Arc<RwLock<WebhookQueue>>,
}

impl WebhookManager {
    pub fn new(settings: Arc<Settings>) -> Self {
        Self {
            settings,
            queue: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn add_event(&self, event: EventType, trigger: Option<String>, files: &[String]) {
        let mut queue = self.queue.write().await;
        let key = (event, trigger);

        queue.entry(key).or_default().extend(files.iter().cloned());
    }

    pub async fn send(&self) -> anyhow::Result<()> {
        let mut queue = self.queue.write().await;
        let webhooks = &self.settings.webhooks;
        let retries = self.settings.opts.webhook_retries;
        let timeout_secs = self.settings.opts.webhook_timeout;

        let mut batch = queue
            .drain()
            .map(|((event_type, trigger), files)| (event_type, trigger, files))
            .collect::<WebhookBatch>();

        drop(queue);

        batch.sort_by(|(a, _, _), (b, _, _)| a.cmp(b));

        let futures: Vec<_> = webhooks
            .iter()
            .map(|(name, webhook)| {
                let batch = &batch;
                async move {
                    if let Err(e) = webhook.send(batch, retries, timeout_secs).await {
                        error!("failed to send webhook '{}': {}", name, e);
                    }
                }
            })
            .collect();

        join_all(futures).await;

        Ok(())
    }
}
