//! Background loop that polls the Yantrik Companion for proactive urges
//! and delivers them via the appropriate YantrikClaw channel.
//!
//! The companion's background cognition (70+ instincts) generates urges —
//! things the AI wants to say proactively. This loop polls `GET /urges`,
//! delivers each urge via the configured channel's `send()`, and suppresses
//! it via `POST /urges/{id}/suppress`.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, info, warn};

use crate::channels::traits::{Channel, SendMessage};

/// Configuration for the urge delivery loop.
pub struct UrgeLoopConfig {
    /// Companion HTTP API base URL.
    pub companion_url: String,
    /// Poll interval in seconds.
    pub poll_interval_secs: u64,
    /// Default channel to deliver urges through (e.g. "telegram").
    pub default_channel: Option<String>,
    /// Telegram chat ID for proactive message delivery.
    pub proactive_chat_id: Option<i64>,
}

/// A proactive urge from the companion's instinct pipeline.
#[derive(Debug, Deserialize)]
struct CompanionUrge {
    urge_id: String,
    #[allow(dead_code)]
    instinct_name: String,
    reason: String,
    urgency: f64,
    suggested_message: String,
    #[allow(dead_code)]
    created_at: Option<f64>,
    #[allow(dead_code)]
    boost_count: Option<u32>,
}

/// Spawn the background urge polling loop.
///
/// This runs indefinitely, polling the companion for pending urges and
/// delivering them via registered channels.
pub fn spawn_urge_loop(
    channels_by_name: Arc<HashMap<String, Arc<dyn Channel>>>,
    config: UrgeLoopConfig,
) {
    let interval = Duration::from_secs(config.poll_interval_secs);
    let client = Client::new();

    tokio::spawn(async move {
        let mut timer = tokio::time::interval(interval);
        // Skip the initial immediate tick — give channels time to connect.
        timer.tick().await;

        info!(
            "urge loop started: polling {} every {}s, default channel: {:?}",
            config.companion_url, config.poll_interval_secs, config.default_channel,
        );

        loop {
            timer.tick().await;

            if let Err(e) = poll_and_deliver(&client, &config, &channels_by_name).await {
                debug!("urge poll cycle failed: {e}");
            }
        }
    });
}

/// Single poll-and-deliver cycle.
async fn poll_and_deliver(
    client: &Client,
    config: &UrgeLoopConfig,
    channels: &HashMap<String, Arc<dyn Channel>>,
) -> Result<(), String> {
    let url = format!("{}/urges", config.companion_url);

    let res = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("failed to poll urges: {e}"))?;

    if !res.status().is_success() {
        return Err(format!("urge poll returned {}", res.status()));
    }

    let urges: Vec<CompanionUrge> = res
        .json()
        .await
        .map_err(|e| format!("failed to parse urges: {e}"))?;

    if urges.is_empty() {
        return Ok(());
    }

    debug!("received {} pending urges", urges.len());

    for urge in &urges {
        // Determine target channel.
        let target_channel = config.default_channel.as_deref().unwrap_or("telegram");

        let channel = match channels.get(target_channel) {
            Some(ch) => Arc::clone(ch),
            None => {
                warn!(
                    "urge {}: no channel '{}' active, skipping",
                    urge.urge_id, target_channel,
                );
                continue;
            }
        };

        // Build the recipient. For Telegram, use proactive_chat_id.
        let recipient = if target_channel == "telegram" {
            if let Some(chat_id) = config.proactive_chat_id {
                chat_id.to_string()
            } else {
                warn!(
                    "urge {}: proactive_chat_id not configured, cannot deliver to telegram",
                    urge.urge_id,
                );
                continue;
            }
        } else {
            // For other channels, use a generic recipient.
            // Channels like Discord/Slack need a channel/room ID configured separately.
            "proactive".to_string()
        };

        // Fall back to reason when suggested_message is empty.
        let text = if urge.suggested_message.is_empty() {
            &urge.reason
        } else {
            &urge.suggested_message
        };

        let message = SendMessage::new(text, &recipient);

        // Deliver via channel.
        match channel.send(&message).await {
            Ok(()) => {
                info!(
                    "delivered urge {} via {} (urgency {:.2})",
                    urge.urge_id, target_channel, urge.urgency,
                );

                // Suppress the urge so it isn't re-delivered.
                let suppress_url =
                    format!("{}/urges/{}/suppress", config.companion_url, urge.urge_id);
                if let Err(e) = client.post(&suppress_url).send().await {
                    warn!("failed to suppress urge {}: {e}", urge.urge_id);
                }
            }
            Err(e) => {
                warn!(
                    "failed to deliver urge {} via {}: {e}",
                    urge.urge_id, target_channel,
                );
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_companion_urge() {
        let json = r#"{
            "urge_id": "u-123",
            "instinct_name": "check_in",
            "reason": "haven't talked in a while",
            "urgency": 0.75,
            "suggested_message": "Hey, how's your day going?",
            "created_at": 1710900000.0,
            "boost_count": 2
        }"#;

        let urge: CompanionUrge = serde_json::from_str(json).unwrap();
        assert_eq!(urge.urge_id, "u-123");
        assert_eq!(urge.urgency, 0.75);
        assert_eq!(urge.suggested_message, "Hey, how's your day going?");
    }

    #[test]
    fn deserializes_urge_with_missing_optional_fields() {
        let json = r#"{
            "urge_id": "u-456",
            "instinct_name": "share_discovery",
            "reason": "found something interesting",
            "urgency": 0.5,
            "suggested_message": "Check this out!"
        }"#;

        let urge: CompanionUrge = serde_json::from_str(json).unwrap();
        assert_eq!(urge.urge_id, "u-456");
        assert!(urge.created_at.is_none());
        assert!(urge.boost_count.is_none());
    }
}
