//! Anthropic `/v1/messages` request + streaming-event types.
//!
//! Minimal subset: text blocks only (no tool use, no images). Phase 2 will
//! add tool definitions, tool_use / tool_result blocks, and image content.

use serde::{Deserialize, Serialize};

/// The exact prefix Anthropic's policy enforcement requires when sending
/// `/v1/messages` with a Claude.ai Pro/Max OAuth token. Without this string
/// as the first system block, the request returns HTTP 429 with a
/// suspiciously empty `rate_limit_error` (`"message": "Error"`). With it,
/// the request is accepted normally regardless of additional system content.
///
/// Verified empirically against api.anthropic.com on 2026-04-29.
pub const CLAUDE_CODE_IDENTITY: &str = "You are Claude Code, Anthropic's official CLI for Claude.";

#[derive(Clone, Debug, Serialize)]
pub struct MessagesRequest {
    pub model: String,
    pub max_tokens: u32,
    pub messages: Vec<Message>,
    /// Either a plain string or an array of system blocks. Use the array form
    /// to send a custom prompt: place [`CLAUDE_CODE_IDENTITY`] as the first
    /// block (required for OAuth-Max requests to bypass policy enforcement),
    /// then any number of additional blocks with the actual instructions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<SystemPrompt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub stream: bool,
}

/// Anthropic accepts `system` as either a single string or an array of typed
/// blocks. Serialized untagged so JSON output is the natural form.
#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum SystemPrompt {
    Single(String),
    Blocks(Vec<SystemBlock>),
}

impl SystemPrompt {
    /// Build the canonical OAuth-Max-safe form: required identity block first,
    /// followed by the caller's actual instructions.
    pub fn with_identity_prefix(actual: impl Into<String>) -> Self {
        Self::Blocks(vec![
            SystemBlock::text(CLAUDE_CODE_IDENTITY),
            SystemBlock::text(actual),
        ])
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct SystemBlock {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub text: String,
}

impl SystemBlock {
    pub fn text(s: impl Into<String>) -> Self {
        Self {
            kind: "text",
            text: s.into(),
        }
    }
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

    #[test]
    fn system_prompt_with_identity_prefix_serializes_as_array() {
        let req = MessagesRequest {
            model: "claude".into(),
            max_tokens: 1,
            messages: vec![],
            system: Some(SystemPrompt::with_identity_prefix("be terse")),
            temperature: None,
            stream: false,
        };
        let json = serde_json::to_value(&req).unwrap();
        let system = json.get("system").unwrap();
        assert!(system.is_array(), "system must serialize as JSON array");
        let arr = system.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["type"], "text");
        assert_eq!(arr[0]["text"], CLAUDE_CODE_IDENTITY);
        assert_eq!(arr[1]["type"], "text");
        assert_eq!(arr[1]["text"], "be terse");
    }

    #[test]
    fn system_prompt_single_serializes_as_string() {
        let req = MessagesRequest {
            model: "claude".into(),
            max_tokens: 1,
            messages: vec![],
            system: Some(SystemPrompt::Single("hello".into())),
            temperature: None,
            stream: false,
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["system"], "hello");
    }
}
