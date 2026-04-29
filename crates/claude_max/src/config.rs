//! OAuth + API endpoint constants for Claude Max direct mode.
//!
//! Values mirror the public claude-code OAuth client. See:
//! - https://code.claude.com/docs/en/authentication
//! - reference: lukilabs/craft-agents-oss `claude-oauth-config.ts`
//!
//! Note: Anthropic's "Authentication and credential use" policy restricts
//! OAuth tokens from Free/Pro/Max accounts to Claude Code and Claude.ai.
//! Use of these tokens elsewhere is at the operator's risk.

pub const OAUTH_CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";

pub const OAUTH_AUTHORIZE_URL: &str = "https://claude.ai/oauth/authorize";

pub const OAUTH_TOKEN_URL: &str = "https://platform.claude.com/v1/oauth/token";

pub const OAUTH_REDIRECT_URI: &str = "https://console.anthropic.com/oauth/code/callback";

pub const OAUTH_SCOPES: &[&str] = &["org:create_api_key", "user:profile", "user:inference"];

pub const ANTHROPIC_API_BASE: &str = "https://api.anthropic.com";

pub const ANTHROPIC_MESSAGES_PATH: &str = "/v1/messages";

pub const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Beta header value sent by claude-code to access OAuth-authenticated inference.
pub const ANTHROPIC_BETA: &str = "oauth-2025-04-20";

/// User-Agent identifying as claude-code; required for OAuth tokens to be accepted
/// by Anthropic's inference endpoint.
pub const USER_AGENT: &str = "claude-cli/1.0.0 (external, cli)";

/// In-memory OAuth state TTL.
pub const OAUTH_STATE_TTL_SECS: u64 = 600;
