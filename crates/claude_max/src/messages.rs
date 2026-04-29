//! Anthropic `/v1/messages` request + streaming-event types.
//!
//! Minimal subset: text blocks only (no tool use, no images). Phase 2 will
//! add tool definitions, tool_use / tool_result blocks, and image content.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize)]
pub struct MessagesRequest {
    pub model: String,
    pub max_tokens: u32,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub stream: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct Message {
    pub role: Role,
    pub content: Vec<ContentBlock>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text { text: String },
}

impl ContentBlock {
    pub fn text(s: impl Into<String>) -> Self {
        Self::Text { text: s.into() }
    }
}

/// Streaming events from `/v1/messages` (event-name → JSON payload).
///
/// We only model what we need for plain text streaming. Anthropic emits more
/// granular events (`ping`, `message_start`, `message_delta`, etc.) which we
/// ignore.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type")]
pub enum StreamEvent {
    #[serde(rename = "content_block_start")]
    ContentBlockStart {
        index: u32,
        content_block: ContentBlock,
    },
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta { index: u32, delta: Delta },
    #[serde(rename = "content_block_stop")]
    ContentBlockStop { index: u32 },
    #[serde(rename = "message_stop")]
    MessageStop,
    #[serde(rename = "error")]
    Error { error: ApiErrorBody },
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type")]
pub enum Delta {
    #[serde(rename = "text_delta")]
    Text { text: String },
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ApiErrorBody {
    #[serde(rename = "type")]
    pub kind: String,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_content_block_delta() {
        let raw =
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"hi"}}"#;
        let evt: StreamEvent = serde_json::from_str(raw).unwrap();
        match evt {
            StreamEvent::ContentBlockDelta { index, delta } => {
                assert_eq!(index, 0);
                match delta {
                    Delta::Text { text } => assert_eq!(text, "hi"),
                    _ => panic!("expected text delta"),
                }
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn deserialize_message_stop() {
        let raw = r#"{"type":"message_stop"}"#;
        let evt: StreamEvent = serde_json::from_str(raw).unwrap();
        assert!(matches!(evt, StreamEvent::MessageStop));
    }

    #[test]
    fn unknown_event_type_falls_through() {
        let raw = r#"{"type":"ping"}"#;
        let evt: StreamEvent = serde_json::from_str(raw).unwrap();
        assert!(matches!(evt, StreamEvent::Unknown));
    }

    #[test]
    fn serialize_request_omits_optional_fields_when_none() {
        let req = MessagesRequest {
            model: "claude-3-5-sonnet-20241022".into(),
            max_tokens: 16,
            messages: vec![Message {
                role: Role::User,
                content: vec![ContentBlock::text("hi")],
            }],
            system: None,
            temperature: None,
            stream: true,
        };
        let json = serde_json::to_value(&req).unwrap();
        assert!(json.get("system").is_none());
        assert!(json.get("temperature").is_none());
        assert_eq!(json["stream"], true);
    }
}
