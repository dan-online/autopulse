/// Discord - Discord Webhook
///
/// Sends a message to a Discord Webhook on events
///
/// # Example
///
/// ```yml
/// webhooks:
///   my_discord:
///     type: discord
///     url: "https://discord.com/api/webhooks/..."
/// ```
///
/// or
///
/// ```yml
/// webhooks:
///   my_discord:
///     type: discord
///     avatar_url: "https://example.com/avatar.png"
///     username: "autopulse"
/// ```
///
/// See [`DiscordWebhook`](discord::DiscordWebhook) for all options
pub mod discord;

#[doc(hidden)]
pub mod manager;

#[doc(hidden)]
pub use manager::*;
