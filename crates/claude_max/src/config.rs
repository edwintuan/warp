//! OAuth + API endpoint constants for Claude Max direct mode.
//!
//! Values mirror the public claude-code OAuth client. Cross-checked against
//! `ex-machina-co/opencode-anthropic-auth` (more current than
//! `lukilabs/craft-agents-oss`, which references the deprecated
//! `console.anthropic.com` callback host).
//!
//! Note: Anthropic's "Authentication and credential use" policy restricts
//! OAuth tokens from Free/Pro/Max accounts to Claude Code and Claude.ai.
//! Use of these tokens elsewhere is at the operator's risk.

pub const OAUTH_CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";

/// Authorize endpoint for **subscription** (Claude.ai Pro/Max) accounts.
/// Console-billed accounts use `https://platform.claude.com/oauth/authorize`
/// instead, but they're not the target of this crate.
pub const OAUTH_AUTHORIZE_URL: &str = "https://claude.ai/oauth/authorize";

pub const OAUTH_TOKEN_URL: &str = "https://platform.claude.com/v1/oauth/token";

/// Must be on `platform.claude.com` to match the URL registered for the
/// claude-code client_id. The legacy `console.anthropic.com` host no longer
/// works (the domain has been redirected and the registration was migrated).
pub const OAUTH_REDIRECT_URI: &str = "https://platform.claude.com/oauth/code/callback";

/// Full scope set the claude-code client is registered for. Requesting a
/// subset can result in the authorize page rejecting the request, so we
/// request all six even though only `user:inference` is strictly needed
/// for `/v1/messages`.
pub const OAUTH_SCOPES: &[&str] = &[
    "org:create_api_key",
    "user:profile",
    "user:inference",
    "user:sessions:claude_code",
    "user:mcp_servers",
    "user:file_upload",
];

pub const ANTHROPIC_API_BASE: &str = "https://api.anthropic.com";

pub const ANTHROPIC_MESSAGES_PATH: &str = "/v1/messages";

pub const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Beta header value sent by claude-code to access OAuth-authenticated inference.
/// Comma-separated list of the betas claude-code currently enables.
pub const ANTHROPIC_BETA: &str = "oauth-2025-04-20,interleaved-thinking-2025-05-14";

/// User-Agent identifying as claude-code; required for OAuth tokens to be accepted
/// by Anthropic's inference endpoint.
pub const USER_AGENT: &str = "claude-cli/2.1.87 (external, cli)";

/// In-memory OAuth state TTL.
pub const OAUTH_STATE_TTL_SECS: u64 = 600;
