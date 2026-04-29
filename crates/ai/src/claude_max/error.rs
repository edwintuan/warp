use thiserror::Error;

#[derive(Error, Debug)]
pub enum ClaudeMaxError {
    #[error("OAuth state mismatch or expired")]
    InvalidState,

    #[error("OAuth code exchange failed: {0}")]
    CodeExchangeFailed(String),

    #[error("Token refresh failed: {0}")]
    RefreshFailed(String),

    #[error("No refresh token available; user must re-authenticate")]
    MissingRefreshToken,

    #[error("Anthropic API request failed (status {status}): {body}")]
    ApiError { status: u16, body: String },

    #[error("Stream parse error: {0}")]
    StreamParse(String),

    #[error("HTTP transport error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("URL parse error: {0}")]
    Url(#[from] url::ParseError),
}

pub type Result<T> = std::result::Result<T, ClaudeMaxError>;
