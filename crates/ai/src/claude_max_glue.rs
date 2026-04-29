//! Glue between the standalone `claude_max` crate (OAuth + Anthropic client)
//! and Warp's app-context-bound services (secure storage).
//!
//! Phase 2a: keychain-backed token storage. The `claude_max` crate ships its
//! own [`FileTokenStore`](claude_max::FileTokenStore) for development; in the
//! integrated Warp app we store OAuth tokens in the platform keychain via
//! `warpui_extras::secure_storage`, which already backs `AiApiKeys` and the
//! rest of Warp's secret material.

use claude_max::OAuthTokens;
use warpui::AppContext;
use warpui_extras::secure_storage::{self, AppContextExt};

/// The secure-storage key under which the Claude Max OAuth token blob lives.
/// Sibling of `AiApiKeys` (defined in `crate::api_keys`); namespace is per
/// `service_name` registered at app startup, so collisions across users on
/// the same machine are not a concern.
const STORAGE_KEY: &str = "ClaudeMaxOAuth";

/// Read OAuth tokens from secure storage. Returns `Ok(None)` if the user has
/// never authenticated (or has logged out).
pub fn load_tokens(ctx: &AppContext) -> anyhow::Result<Option<OAuthTokens>> {
    match ctx.secure_storage().read_value(STORAGE_KEY) {
        Ok(json) => Ok(Some(serde_json::from_str(&json)?)),
        Err(secure_storage::Error::NotFound) => Ok(None),
        Err(e) => Err(anyhow::anyhow!(
            "claude_max secure storage read failed: {e}"
        )),
    }
}

/// Persist OAuth tokens to secure storage. Overwrites any prior value.
pub fn save_tokens(ctx: &AppContext, tokens: &OAuthTokens) -> anyhow::Result<()> {
    let json = serde_json::to_string(tokens)?;
    ctx.secure_storage()
        .write_value(STORAGE_KEY, &json)
        .map_err(|e| anyhow::anyhow!("claude_max secure storage write failed: {e}"))?;
    Ok(())
}

/// Remove the stored token blob. Idempotent — does not error if the key is
/// already absent.
pub fn clear_tokens(ctx: &AppContext) -> anyhow::Result<()> {
    match ctx.secure_storage().remove_value(STORAGE_KEY) {
        Ok(()) | Err(secure_storage::Error::NotFound) => Ok(()),
        Err(e) => Err(anyhow::anyhow!(
            "claude_max secure storage remove failed: {e}"
        )),
    }
}

/// True iff a non-expired token is currently stored. Cheap (one storage read,
/// no network).
pub fn is_authenticated(ctx: &AppContext) -> bool {
    match load_tokens(ctx) {
        Ok(Some(tokens)) => !tokens.is_expired(chrono::Utc::now()),
        Ok(None) => false,
        Err(e) => {
            log::warn!("claude_max is_authenticated check failed: {e:#}");
            false
        }
    }
}
