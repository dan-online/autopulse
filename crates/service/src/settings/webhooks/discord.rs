use super::{transport, EventType, WebhookBatch};
use autopulse_utils::sify;
use serde::{de::Error as _, Deserialize, Serialize};

/// One of the two broadcast mention literals.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SpecialMention {
    Here,
    Everyone,
}

/// A user or role mention, written in config as a single-key map keyed by
/// `user` or `role` with a string ID value.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TaggedMention {
    Role(String),
    User(String),
}

/// A single entry in a discord webhook's `mentions[].targets`.
///
/// In config, accepts either:
/// - a bare string literal: `"here"` or `"everyone"`
/// - a single-key map keyed by `role` or `user` with a string ID value
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(untagged)]
pub enum MentionTarget {
    Special(SpecialMention),
    Tagged(TaggedMention),
}

/// A single Discord mention entry, attached to one or more event types.
#[derive(Serialize, Clone, Debug)]
pub struct DiscordMention {
    /// Mention targets. Each entry is one of: `"here"`, `"everyone"`, or
    /// a single-key map keyed by `role` or `user` with a string ID value.
    pub targets: Vec<MentionTarget>,
    /// Event types that trigger this mention. An empty list (or omitted
    /// field) means the mention fires on every event in the batch.
    #[serde(default)]
    pub on: Vec<EventType>,
}

impl<'de> Deserialize<'de> for DiscordMention {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Raw {
            targets: Vec<MentionTarget>,
            #[serde(default)]
            on: Vec<EventType>,
        }

        let raw = Raw::deserialize(deserializer)?;
        if raw.targets.is_empty() {
            return Err(D::Error::custom(
                "discord mention: `targets` must be non-empty (entries: \"here\", \"everyone\", or a single-key map keyed by \"role\" or \"user\")",
            ));
        }
        Ok(DiscordMention {
            targets: raw.targets,
            on: raw.on,
        })
    }
}

#[derive(Serialize, Clone)]
#[doc(hidden)]
pub struct AllowedMentions {
    /// Whitelist of role IDs that may be mentioned. Discord requires
    /// this to be set explicitly even when the content has `<@&id>`.
    pub roles: Vec<String>,
    /// Whitelist of user IDs that may be mentioned. Discord requires
    /// this to be set explicitly even when the content has `<@id>`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub users: Vec<String>,
    /// Mention-type parse list. Discord requires `"everyone"` here to
    /// actually fire `@here` and `@everyone` from message content.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub parse: Vec<String>,
}

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
    /// Mention prefix (`<@user>`, `<@&role>`, `@here`, `@everyone`) rendered
    /// when any embed in this payload matches a configured mention.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_mentions: Option<AllowedMentions>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DiscordWebhook {
    /// Webhook URL
    pub url: String,
    /// Optional avatar URL (default [assets/logo.webp](https://raw.githubusercontent.com/dan-online/autopulse/main/assets/logo.webp))
    pub avatar_url: Option<String>,
    /// Optional username (default: autopulse)
    pub username: Option<String>,
    /// Mentions to attach to messages whose batch contains a matching event type.
    ///
    /// Each entry lists `targets` (any mix of `"here"`, `"everyone"`, or
    /// a single-key map keyed by `role` or `user`) and an optional `on` filter.
    /// An empty or missing `on` means the mention fires on every event.
    ///
    /// Example:
    /// ```yml
    /// mentions:
    ///   - targets:
    ///       - here
    ///       - role: "1234567890"
    ///       - user: "9876543210"
    ///     on: [failed]
    ///   - targets: [everyone]
    /// ```
    #[serde(default)]
    pub mentions: Vec<DiscordMention>,
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
            content: None,
            allowed_mentions: None,
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

        // Walk configured mentions in declared order. For each entry, the
        // `on` filter decides whether it applies to this batch, then each
        // target is bucketed by kind. User and role IDs are deduped
        // insertion-stably so a multi-event batch doesn't repeat the same
        // mention token in `content`.
        let mut user_ids: Vec<String> = Vec::new();
        let mut role_ids: Vec<String> = Vec::new();
        let mut include_here = false;
        let mut include_everyone = false;
        let event_kinds: std::collections::HashSet<&EventType> =
            batch.iter().map(|(e, _, _)| e).collect();
        for mention in &self.mentions {
            let matches =
                mention.on.is_empty() || mention.on.iter().any(|e| event_kinds.contains(e));
            if !matches {
                continue;
            }
            for target in &mention.targets {
                match target {
                    MentionTarget::Special(SpecialMention::Here) => include_here = true,
                    MentionTarget::Special(SpecialMention::Everyone) => include_everyone = true,
                    MentionTarget::Tagged(TaggedMention::Role(id)) => {
                        if !role_ids.iter().any(|r| r == id) {
                            role_ids.push(id.clone());
                        }
                    }
                    MentionTarget::Tagged(TaggedMention::User(id)) => {
                        if !user_ids.iter().any(|u| u == id) {
                            user_ids.push(id.clone());
                        }
                    }
                }
            }
        }

        if !user_ids.is_empty() || !role_ids.is_empty() || include_here || include_everyone {
            // Order: users (most specific) -> roles -> @here -> @everyone.
            let mut parts: Vec<String> = Vec::new();
            parts.extend(user_ids.iter().map(|u| format!("<@{u}>")));
            parts.extend(role_ids.iter().map(|r| format!("<@&{r}>")));
            if include_here {
                parts.push("@here".to_string());
            }
            if include_everyone {
                parts.push("@everyone".to_string());
            }
            content.content = Some(parts.join(" "));
            // Discord uses a single "everyone" parse value to authorize
            // both @here and @everyone content tokens.
            let parse = if include_here || include_everyone {
                vec!["everyone".to_string()]
            } else {
                Vec::new()
            };
            content.allowed_mentions = Some(AllowedMentions {
                roles: role_ids,
                users: user_ids,
                parse,
            });
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

    use super::super::EventType;

    fn role(id: &str) -> MentionTarget {
        MentionTarget::Tagged(TaggedMention::Role(id.to_string()))
    }

    fn user(id: &str) -> MentionTarget {
        MentionTarget::Tagged(TaggedMention::User(id.to_string()))
    }

    const HERE: MentionTarget = MentionTarget::Special(SpecialMention::Here);
    const EVERYONE: MentionTarget = MentionTarget::Special(SpecialMention::Everyone);

    fn mention(targets: Vec<MentionTarget>, on: Vec<EventType>) -> DiscordMention {
        DiscordMention { targets, on }
    }

    fn webhook_with_mentions() -> DiscordWebhook {
        DiscordWebhook {
            url: "https://discord.example/webhook".to_string(),
            avatar_url: None,
            username: None,
            mentions: vec![
                mention(vec![role("111")], vec![EventType::Processed]),
                mention(
                    vec![role("222")],
                    vec![EventType::Failed, EventType::HashMismatch],
                ),
            ],
        }
    }

    #[test]
    fn no_content_when_no_mentions_configured() {
        let w = DiscordWebhook {
            url: "x".to_string(),
            avatar_url: None,
            username: None,
            mentions: vec![],
        };
        let payload = w.generate_json(&[(EventType::Processed, None, vec!["/a".to_string()])]);
        assert!(payload.content.is_none());
        assert!(payload.allowed_mentions.is_none());
    }

    #[test]
    fn content_pings_role_for_matching_event() {
        let w = webhook_with_mentions();
        let payload = w.generate_json(&[(EventType::Processed, None, vec!["/a".to_string()])]);
        assert_eq!(payload.content.as_deref(), Some("<@&111>"));
        let am = payload.allowed_mentions.expect("allowed_mentions set");
        assert_eq!(am.roles, vec!["111".to_string()]);
        assert!(am.users.is_empty());
    }

    #[test]
    fn content_does_not_ping_for_unrelated_event() {
        let w = webhook_with_mentions();
        let payload = w.generate_json(&[(EventType::New, None, vec!["/a".to_string()])]);
        assert!(payload.content.is_none());
        assert!(payload.allowed_mentions.is_none());
    }

    #[test]
    fn content_deduplicates_roles_across_batch() {
        let w = webhook_with_mentions();
        let payload = w.generate_json(&[
            (EventType::Failed, None, vec!["/a".to_string()]),
            (EventType::HashMismatch, None, vec!["/b".to_string()]),
        ]);
        // Role 222 subscribes to both events; only one mention should appear.
        assert_eq!(payload.content.as_deref(), Some("<@&222>"));
        let am = payload.allowed_mentions.expect("allowed_mentions set");
        assert_eq!(am.roles, vec!["222".to_string()]);
    }

    #[test]
    fn here_target_fires_for_matching_event() {
        let w = DiscordWebhook {
            url: "x".to_string(),
            avatar_url: None,
            username: None,
            mentions: vec![mention(vec![HERE], vec![EventType::Processed])],
        };
        let payload = w.generate_json(&[(EventType::Processed, None, vec!["/a".to_string()])]);
        let content = payload.content.expect("content set");
        assert!(content.contains("@here"), "content was {content:?}");
        let am = payload.allowed_mentions.expect("allowed_mentions set");
        assert!(
            am.parse.iter().any(|p| p == "everyone"),
            "parse was {:?}",
            am.parse
        );
    }

    #[test]
    fn everyone_target_fires_for_matching_event() {
        let w = DiscordWebhook {
            url: "x".to_string(),
            avatar_url: None,
            username: None,
            mentions: vec![mention(vec![EVERYONE], vec![EventType::Failed])],
        };
        let payload = w.generate_json(&[(EventType::Failed, None, vec!["/a".to_string()])]);
        let content = payload.content.expect("content set");
        assert!(content.contains("@everyone"), "content was {content:?}");
        let am = payload.allowed_mentions.expect("allowed_mentions set");
        assert!(
            am.parse.iter().any(|p| p == "everyone"),
            "parse was {:?}",
            am.parse
        );
    }

    #[test]
    fn user_target_fires_and_whitelists_user() {
        let w = DiscordWebhook {
            url: "x".to_string(),
            avatar_url: None,
            username: None,
            mentions: vec![mention(vec![user("999")], vec![EventType::Processed])],
        };
        let payload = w.generate_json(&[(EventType::Processed, None, vec!["/a".to_string()])]);
        assert_eq!(payload.content.as_deref(), Some("<@999>"));
        let am = payload.allowed_mentions.expect("allowed_mentions set");
        assert_eq!(am.users, vec!["999".to_string()]);
        assert!(am.roles.is_empty());
    }

    #[test]
    fn user_id_dedup_across_batch() {
        let w = DiscordWebhook {
            url: "x".to_string(),
            avatar_url: None,
            username: None,
            mentions: vec![mention(
                vec![user("42")],
                vec![EventType::Failed, EventType::HashMismatch],
            )],
        };
        let payload = w.generate_json(&[
            (EventType::Failed, None, vec!["/a".to_string()]),
            (EventType::HashMismatch, None, vec!["/b".to_string()]),
        ]);
        assert_eq!(payload.content.as_deref(), Some("<@42>"));
    }

    #[test]
    fn mixed_kinds_in_single_entry_render_in_blast_radius_order() {
        let w = DiscordWebhook {
            url: "x".to_string(),
            avatar_url: None,
            username: None,
            mentions: vec![mention(
                vec![EVERYONE, role("777"), user("42"), HERE],
                vec![EventType::Failed],
            )],
        };
        let payload = w.generate_json(&[(EventType::Failed, None, vec!["/a".to_string()])]);
        // Render order: users -> roles -> @here -> @everyone, regardless of config order.
        assert_eq!(
            payload.content.as_deref(),
            Some("<@42> <@&777> @here @everyone")
        );
        let am = payload.allowed_mentions.expect("allowed_mentions set");
        assert_eq!(am.roles, vec!["777".to_string()]);
        assert_eq!(am.users, vec!["42".to_string()]);
        assert!(am.parse.iter().any(|p| p == "everyone"));
    }

    #[test]
    fn multiple_roles_in_single_entry() {
        let w = DiscordWebhook {
            url: "x".to_string(),
            avatar_url: None,
            username: None,
            mentions: vec![mention(
                vec![role("111"), role("222")],
                vec![EventType::Processed],
            )],
        };
        let payload = w.generate_json(&[(EventType::Processed, None, vec!["/a".to_string()])]);
        let content = payload.content.expect("content set");
        assert!(content.contains("<@&111>"), "content was {content:?}");
        assert!(content.contains("<@&222>"), "content was {content:?}");
        let am = payload.allowed_mentions.expect("allowed_mentions set");
        assert_eq!(am.roles, vec!["111".to_string(), "222".to_string()]);
    }

    #[test]
    fn role_id_dedup_within_single_entry() {
        let w = DiscordWebhook {
            url: "x".to_string(),
            avatar_url: None,
            username: None,
            mentions: vec![mention(
                vec![role("111"), role("111")],
                vec![EventType::Processed],
            )],
        };
        let payload = w.generate_json(&[(EventType::Processed, None, vec!["/a".to_string()])]);
        assert_eq!(payload.content.as_deref(), Some("<@&111>"));
    }

    #[test]
    fn deserialize_mixed_targets() {
        let json = r#"{
            "targets": ["here", { "role": "111" }, { "user": "222" }, "everyone"],
            "on": ["failed"]
        }"#;
        let m: DiscordMention = serde_json::from_str(json).unwrap();
        assert_eq!(
            m.targets,
            vec![
                MentionTarget::Special(SpecialMention::Here),
                MentionTarget::Tagged(TaggedMention::Role("111".to_string())),
                MentionTarget::Tagged(TaggedMention::User("222".to_string())),
                MentionTarget::Special(SpecialMention::Everyone),
            ]
        );
        assert_eq!(m.on, vec![EventType::Failed]);
    }

    #[test]
    fn deserialize_rejects_empty_targets() {
        let json = r#"{ "targets": [], "on": ["processed"] }"#;
        let err = serde_json::from_str::<DiscordMention>(json).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("non-empty"),
            "expected validation error mentioning 'non-empty', got: {msg}"
        );
    }

    #[test]
    fn deserialize_rejects_missing_targets() {
        let json = r#"{ "on": ["processed"] }"#;
        let err = serde_json::from_str::<DiscordMention>(json).unwrap_err();
        // serde itself reports the missing field; we don't customize this message.
        let msg = err.to_string();
        assert!(
            msg.contains("targets"),
            "expected error mentioning `targets`, got: {msg}"
        );
    }

    #[test]
    fn deserialize_rejects_unknown_special() {
        // "channel" is not a recognized special; should fail to match either
        // variant of the untagged enum.
        let json = r#"{ "targets": ["channel"] }"#;
        let err = serde_json::from_str::<DiscordMention>(json).unwrap_err();
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn deserialize_rejects_tagged_with_unknown_kind() {
        let json = r#"{ "targets": [{ "channel": "1" }] }"#;
        let err = serde_json::from_str::<DiscordMention>(json).unwrap_err();
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn event_type_deserializes_from_snake_case() {
        let v: EventType = serde_json::from_str("\"hash_mismatch\"").unwrap();
        assert_eq!(v, EventType::HashMismatch);
        let v: EventType = serde_json::from_str("\"processed\"").unwrap();
        assert_eq!(v, EventType::Processed);
    }

    #[test]
    fn role_with_no_on_pings_for_every_event() {
        let w = DiscordWebhook {
            url: "x".to_string(),
            avatar_url: None,
            username: None,
            mentions: vec![mention(vec![role("111")], vec![])],
        };
        let payload = w.generate_json(&[(EventType::New, None, vec!["/a".to_string()])]);
        assert_eq!(payload.content.as_deref(), Some("<@&111>"));
        let am = payload.allowed_mentions.expect("allowed_mentions set");
        assert_eq!(am.roles, vec!["111".to_string()]);
    }

    #[test]
    fn here_with_no_on_pings_for_every_event() {
        let w = DiscordWebhook {
            url: "x".to_string(),
            avatar_url: None,
            username: None,
            mentions: vec![mention(vec![HERE], vec![])],
        };
        let payload = w.generate_json(&[(EventType::New, None, vec!["/a".to_string()])]);
        let content = payload.content.expect("content set");
        assert!(content.contains("@here"), "content was {content:?}");
        let am = payload.allowed_mentions.expect("allowed_mentions set");
        assert!(
            am.parse.iter().any(|p| p == "everyone"),
            "parse was {:?}",
            am.parse
        );
    }

    #[test]
    fn mixed_default_and_specific_on_both_fire() {
        let w = DiscordWebhook {
            url: "x".to_string(),
            avatar_url: None,
            username: None,
            mentions: vec![
                mention(vec![role("111")], vec![]),
                mention(vec![role("222")], vec![EventType::Failed]),
            ],
        };
        let payload = w.generate_json(&[(EventType::Failed, None, vec!["/a".to_string()])]);
        let content = payload.content.expect("content set");
        assert!(content.contains("<@&111>"), "content was {content:?}");
        assert!(content.contains("<@&222>"), "content was {content:?}");
        let am = payload.allowed_mentions.expect("allowed_mentions set");
        assert!(am.roles.contains(&"111".to_string()));
        assert!(am.roles.contains(&"222".to_string()));
    }

    #[test]
    fn event_type_snake_case_matches_key_method() {
        for variant in [
            EventType::New,
            EventType::HashMismatch,
            EventType::Found,
            EventType::Retrying,
            EventType::Processed,
            EventType::Failed,
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            assert_eq!(
                json.trim_matches('"'),
                variant.key(),
                "serde wire format must equal key() for {variant:?}"
            );
        }
    }
}
