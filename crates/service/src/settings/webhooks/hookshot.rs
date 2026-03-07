use super::{transport, EventType, WebhookBatch};
use autopulse_utils::sify;
use html_escape::encode_text;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Clone)]
struct HookshotPayload {
    text: String,
    html: String,
    username: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct HookshotWebhook {
    /// Webhook URL
    pub url: String,
    /// Optional username (default: autopulse)
    pub username: Option<String>,
}

impl HookshotWebhook {
    fn summary_line(event: &EventType, trigger: Option<&str>, files: &[String]) -> String {
        trigger.map_or_else(
            || {
                format!(
                    "[{event}] - {} file{} {}",
                    files.len(),
                    sify(files),
                    event.action()
                )
            },
            |trigger| {
                format!(
                    "[{event}] - [{trigger}] - {} file{} {}",
                    files.len(),
                    sify(files),
                    event.action()
                )
            },
        )
    }

    fn generate_payload(&self, batch: &WebhookBatch) -> HookshotPayload {
        let username = self
            .username
            .clone()
            .unwrap_or_else(|| "autopulse".to_string());
        let sections = batch
            .iter()
            .map(|(event, trigger, files)| {
                let summary = Self::summary_line(event, trigger.as_deref(), files);
                let files = files.join("\n");

                format!("{summary}\n{files}")
            })
            .collect::<Vec<_>>();
        let text = sections.join("\n\n");
        let html = batch
            .iter()
            .map(|(event, trigger, files)| {
                let raw_summary = Self::summary_line(event, trigger.as_deref(), files);
                let summary = encode_text(&raw_summary);
                let files = files
                    .iter()
                    .map(|file| format!("<li><code>{}</code></li>", encode_text(file)))
                    .collect::<Vec<_>>()
                    .join("");

                format!("<li><strong>{summary}</strong><ul>{files}</ul></li>")
            })
            .collect::<Vec<_>>()
            .join("");
        let html = format!("<ul>{html}</ul>");

        HookshotPayload {
            text,
            html,
            username,
        }
    }

    pub async fn send(
        &self,
        batch: &WebhookBatch,
        retries: u8,
        timeout_secs: u64,
    ) -> anyhow::Result<()> {
        let payload = self.generate_payload(batch);

        transport::shared_sender(std::time::Duration::from_secs(timeout_secs))?
            .send_json(&self.url, &[payload], retries)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_batch() -> WebhookBatch {
        vec![
            (
                EventType::Processed,
                Some("sonarr".to_string()),
                vec!["/media/tv/show-01.mkv".to_string()],
            ),
            (
                EventType::Failed,
                None,
                vec!["/media/movies/movie-01.mkv".to_string()],
            ),
        ]
    }

    #[test]
    fn generate_payload_defaults_username_and_summarizes_each_batch_item() {
        let webhook = HookshotWebhook {
            url: "https://example.com/webhooks/hookshot".to_string(),
            username: None,
        };

        let payload = webhook.generate_payload(&sample_batch());

        assert_eq!(payload.username, "autopulse");
        assert!(payload
            .text
            .contains("[PROCESSED] - [sonarr] - 1 file processed"));
        assert!(payload.text.contains("/media/tv/show-01.mkv"));
        assert!(payload.text.contains("[FAILED] - 1 file failed"));
        assert!(payload
            .html
            .contains("<code>/media/movies/movie-01.mkv</code>"));
    }

    #[test]
    fn html_entities_in_filenames_are_escaped() {
        let batch: WebhookBatch = vec![(
            EventType::Processed,
            Some("sonarr".to_string()),
            vec![
                r#"/media/tv/<script>alert("xss")</script>.mkv"#.to_string(),
                "/media/tv/Tom & Jerry's Show.mkv".to_string(),
            ],
        )];

        let webhook = HookshotWebhook {
            url: "https://example.com/hook".to_string(),
            username: None,
        };

        let payload = webhook.generate_payload(&batch);

        // Angle brackets escaped
        assert!(payload.html.contains("&lt;script&gt;"));
        assert!(payload.html.contains("&lt;/script&gt;"));
        // Ampersand escaped
        assert!(payload.html.contains("Tom &amp; Jerry"));
        // Quotes are safe in text nodes and left unescaped by encode_text
        assert!(payload.html.contains(r#"alert("xss")"#));
        assert!(payload.html.contains("Jerry's"));
        // Raw angle brackets must NOT appear in HTML output
        assert!(!payload.html.contains("<script>"));
    }
}
