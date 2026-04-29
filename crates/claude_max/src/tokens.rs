//! Token model + simple disk persistence.
//!
//! For first-cut testing this stores tokens as JSON at a known path. The
//! `TokenStore` trait lets a future Warp integration swap in keychain-backed
//! storage (`warpui_extras::secure_storage`) without changing call sites.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use super::error::{ClaudeMaxError, Result};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OAuthTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    /// Absolute expiry; `None` means "unknown, treat as fresh until 401".
    pub expires_at: Option<DateTime<Utc>>,
    pub token_type: String,
    pub scope: Option<String>,
}

impl OAuthTokens {
    /// Construct from a token-endpoint response.
    pub fn from_response(resp: TokenResponse, now: DateTime<Utc>) -> Self {
        let expires_at = resp
            .expires_in
            .map(|secs| now + Duration::seconds(secs as i64));
        Self {
            access_token: resp.access_token,
            refresh_token: resp.refresh_token,
            expires_at,
            token_type: resp.token_type.unwrap_or_else(|| "Bearer".into()),
            scope: resp.scope,
        }
    }

    /// True if the token is past its expiry, with a small safety margin.
    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        match self.expires_at {
            Some(exp) => now + Duration::seconds(60) >= exp,
            None => false,
        }
    }
}

/// Raw shape of the token endpoint response.
#[derive(Clone, Debug, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub expires_in: Option<u64>,
    #[serde(default)]
    pub token_type: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
}

/// Pluggable storage for OAuth tokens.
pub trait TokenStore: Send + Sync {
    fn load(&self) -> Result<Option<OAuthTokens>>;
    fn save(&self, tokens: &OAuthTokens) -> Result<()>;
    fn clear(&self) -> Result<()>;
}

/// Disk-backed JSON store. **Not encrypted** — for development and CLI smoke
/// tests. Production should swap to a keychain-backed implementation.
pub struct FileTokenStore {
    path: PathBuf,
}

impl FileTokenStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Default location: `$XDG_CONFIG_HOME/warp-claude-max/tokens.json`
    /// (or `~/.config/warp-claude-max/tokens.json`).
    pub fn default_path() -> Result<PathBuf> {
        let base = dirs::config_dir().ok_or_else(|| {
            ClaudeMaxError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "config dir not found",
            ))
        })?;
        Ok(base.join("warp-claude-max").join("tokens.json"))
    }

    fn ensure_parent(path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(())
    }
}

impl TokenStore for FileTokenStore {
    fn load(&self) -> Result<Option<OAuthTokens>> {
        match std::fs::read(&self.path) {
            Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn save(&self, tokens: &OAuthTokens) -> Result<()> {
        Self::ensure_parent(&self.path)?;
        let bytes = serde_json::to_vec_pretty(tokens)?;
        std::fs::write(&self.path, bytes)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            std::fs::set_permissions(&self.path, perms)?;
        }
        Ok(())
    }

    fn clear(&self) -> Result<()> {
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn from_response_computes_expiry() {
        let now = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let resp = TokenResponse {
            access_token: "a".into(),
            refresh_token: Some("r".into()),
            expires_in: Some(3600),
            token_type: Some("Bearer".into()),
            scope: Some("user:inference".into()),
        };
        let tokens = OAuthTokens::from_response(resp, now);
        assert_eq!(tokens.expires_at, Some(now + Duration::seconds(3600)));
        assert_eq!(tokens.refresh_token.as_deref(), Some("r"));
        assert_eq!(tokens.token_type, "Bearer");
    }

    #[test]
    fn is_expired_uses_safety_margin() {
        let now = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let mut t = OAuthTokens {
            access_token: "a".into(),
            refresh_token: None,
            expires_at: Some(now + Duration::seconds(30)),
            token_type: "Bearer".into(),
            scope: None,
        };
        assert!(t.is_expired(now), "30s left < 60s margin");
        t.expires_at = Some(now + Duration::seconds(120));
        assert!(!t.is_expired(now));
    }

    #[test]
    fn file_store_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileTokenStore::new(dir.path().join("t.json"));
        assert!(store.load().unwrap().is_none());
        let tokens = OAuthTokens {
            access_token: "abc".into(),
            refresh_token: Some("rrr".into()),
            expires_at: None,
            token_type: "Bearer".into(),
            scope: None,
        };
        store.save(&tokens).unwrap();
        assert_eq!(store.load().unwrap(), Some(tokens));
        store.clear().unwrap();
        assert!(store.load().unwrap().is_none());
    }
}
