//! Direct Anthropic `/v1/messages` client (streaming).
//!
//! Uses an OAuth bearer token instead of `x-api-key`. Includes the headers
//! required for OAuth-authenticated inference (anthropic-version,
//! anthropic-beta, User-Agent).

use std::pin::Pin;

use futures::{Stream, StreamExt};
use reqwest::Client;
use reqwest_eventsource::{Event, EventSource};

use super::config::{
    ANTHROPIC_API_BASE, ANTHROPIC_BETA, ANTHROPIC_MESSAGES_PATH, ANTHROPIC_VERSION, USER_AGENT,
};
use super::error::{ClaudeMaxError, Result};
use super::messages::{MessagesRequest, StreamEvent};

pub type EventStream = Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>;

pub struct AnthropicClient {
    http: Client,
    api_base: String,
}

impl AnthropicClient {
    pub fn new(http: Client) -> Self {
        Self {
            http,
            api_base: ANTHROPIC_API_BASE.into(),
        }
    }

    /// Override the API base URL (useful for tests/mocks).
    pub fn with_api_base(mut self, base: impl Into<String>) -> Self {
        self.api_base = base.into();
        self
    }

    /// Send a streaming `/v1/messages` request.
    ///
    /// Returns a pinned stream of typed events. Caller drives it to completion.
    pub fn stream_messages(
        &self,
        access_token: &str,
        request: MessagesRequest,
    ) -> Result<EventStream> {
        let url = format!("{}{ANTHROPIC_MESSAGES_PATH}", self.api_base);
        let req = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {access_token}"))
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("anthropic-beta", ANTHROPIC_BETA)
            .header("User-Agent", USER_AGENT)
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream")
            .json(&request);

        let event_source =
            EventSource::new(req).map_err(|e| ClaudeMaxError::StreamParse(e.to_string()))?;

        Ok(Box::pin(into_event_stream(event_source)))
    }
}

fn into_event_stream(mut source: EventSource) -> impl Stream<Item = Result<StreamEvent>> {
    async_stream::stream! {
        while let Some(ev) = source.next().await {
            match ev {
                Ok(Event::Open) => {
                    // Opened — wait for messages.
                }
                Ok(Event::Message(msg)) => {
                    if msg.data.is_empty() {
                        continue;
                    }
                    match serde_json::from_str::<StreamEvent>(&msg.data) {
                        Ok(parsed) => {
                            let stop = matches!(
                                parsed,
                                StreamEvent::MessageStop | StreamEvent::Error { .. },
                            );
                            yield Ok(parsed);
                            if stop {
                                source.close();
                                break;
                            }
                        }
                        Err(e) => {
                            yield Err(ClaudeMaxError::StreamParse(format!(
                                "could not parse event '{}': {e}",
                                msg.event,
                            )));
                        }
                    }
                }
                Err(reqwest_eventsource::Error::StreamEnded) => break,
                Err(e) => {
                    yield Err(ClaudeMaxError::StreamParse(e.to_string()));
                    break;
                }
            }
        }
    }
}
