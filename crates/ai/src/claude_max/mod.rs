//! Direct connection to Anthropic via Claude Max OAuth.
//!
//! This module bypasses Warp's cloud server (`warp-server`) and talks to
//! `api.anthropic.com` directly using an OAuth token obtained from a
//! `claude.ai` Free/Pro/Max subscription.
//!
//! ⚠️ Anthropic's "Authentication and credential use" policy restricts these
//! tokens to Claude Code and Claude.ai. Use elsewhere is not permitted by
//! that policy and may result in token revocation.
//!
//! ### Layout
//! - [`config`]: client_id, URLs, scopes, beta header — all the magic strings.
//! - [`pkce`]: PKCE verifier/challenge + state generation.
//! - [`oauth`]: authorize-URL builder, code exchange, refresh.
//! - [`tokens`]: `OAuthTokens` data + `TokenStore` trait + file-backed impl.
//! - [`messages`]: `/v1/messages` request + streaming event types.
//! - [`client`]: streaming HTTP client.
//!
//! ### Phase 1 (this module): chat-only, no tools.
//! ### Phase 2 (todo): tool_use blocks, MCP bridging, AgentConversationsModel
//!                     wiring, settings UI.

pub mod client;
pub mod config;
pub mod error;
pub mod messages;
pub mod oauth;
pub mod pkce;
pub mod tokens;

pub use client::AnthropicClient;
pub use error::{ClaudeMaxError, Result};
pub use oauth::{exchange_code, refresh, start, AuthFlow};
pub use tokens::{FileTokenStore, OAuthTokens, TokenStore};
