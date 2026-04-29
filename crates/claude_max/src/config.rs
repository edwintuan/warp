//! OAuth + API endpoint constants for Claude Max direct mode.
//!
//! Values verified against `lukilabs/craft-agents-oss`
//! (`packages/shared/src/auth/claude-oauth-config.ts`), which is known to
//! work end-to-end for Claude.ai Pro/Max accounts. Earlier guesses based on
//! `ex-machina-co/opencode-anthropic-auth` (different redirect URI, more
//! scopes) caused the authorize page to reject the request because the
//! redirect URI was not registered for this client_id.
//!
//! Note: Anthropic's "Authentication and credential use" policy restricts
//! OAuth tokens from Free/Pro/Max accounts to Claude Code and Claude.ai.
//! Use of these tokens elsewhere is at the operator's risk.

pub const OAUTH_CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";

/// Authorize endpoint for **subscription** (Claude.ai Pro/Max) accounts.
pub const OAUTH_AUTHORIZE_URL: &str = "https://claude.ai/oauth/authorize";

pub const OAUTH_TOKEN_URL: &str = "https://platform.claude.com/v1/oauth/token";

/// Must match what's registered for `OAUTH_CLIENT_ID`. Despite the
/// `console.anthropic.com` host being legacy-looking, this is what works in
/// practice — `platform.claude.com/oauth/code/callback` is NOT registered
/// for this client_id and will cause the authorize page to silently drop
/// the request (browser sees "Missing client_id" or similar).
pub const OAUTH_REDIRECT_URI: &str = "https://console.anthropic.com/oauth/code/callback";

/// Exactly the three scopes craft-agents-oss requests. Adding more (e.g.
/// `user:sessions:claude_code`, `user:mcp_servers`, `user:file_upload`)
/// can also cause silent rejection.
pub const OAUTH_SCOPES: &[&str] = &["org:create_api_key", "user:profile", "user:inference"];

pub const ANTHROPIC_API_BASE: &str = "https://api.anthropic.com";

pub const ANTHROPIC_MESSAGES_PATH: &str = "/v1/messages";

pub const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Beta header value required for OAuth-authenticated inference.
pub const ANTHROPIC_BETA: &str = "oauth-2025-04-20";

/// User-Agent for the token endpoint and inference endpoint. craft-agents-oss
/// successfully authenticates with its own product name, so we use a neutral
/// identifier rather than impersonating claude-code.
pub const USER_AGENT: &str = "warp-claude-max/0.1.0";

/// In-memory OAuth state TTL.
pub const OAUTH_STATE_TTL_SECS: u64 = 600;
