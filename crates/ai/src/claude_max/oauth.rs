//! OAuth authorization-code-with-PKCE flow against Anthropic.

use chrono::Utc;
use reqwest::Client;
use serde::Serialize;
use url::Url;

use super::config::{
    OAUTH_AUTHORIZE_URL, OAUTH_CLIENT_ID, OAUTH_REDIRECT_URI, OAUTH_SCOPES, OAUTH_TOKEN_URL,
    USER_AGENT,
};
use super::error::{ClaudeMaxError, Result};
use super::pkce::{generate_state, PkcePair};
use super::tokens::{OAuthTokens, TokenResponse};

/// Per-flow state held between `start` and `exchange_code`.
#[derive(Clone, Debug)]
pub struct AuthFlow {
    pub authorize_url: Url,
    pub state: String,
    pub pkce: PkcePair,
}

/// Build the `authorize` URL the user should open in a browser.
pub fn start() -> Result<AuthFlow> {
    let pkce = PkcePair::generate();
    let state = generate_state();
    let mut url = Url::parse(OAUTH_AUTHORIZE_URL)?;
    url.query_pairs_mut()
        .append_pair("code", "true")
        .append_pair("client_id", OAUTH_CLIENT_ID)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", OAUTH_REDIRECT_URI)
        .append_pair("scope", &OAUTH_SCOPES.join(" "))
        .append_pair("state", &state)
        .append_pair("code_challenge", &pkce.challenge)
        .append_pair("code_challenge_method", "S256");
    Ok(AuthFlow {
        authorize_url: url,
        state,
        pkce,
    })
}

#[derive(Serialize)]
struct ExchangeBody<'a> {
    grant_type: &'a str,
    code: &'a str,
    state: &'a str,
    client_id: &'a str,
    redirect_uri: &'a str,
    code_verifier: &'a str,
}

#[derive(Serialize)]
struct RefreshBody<'a> {
    grant_type: &'a str,
    refresh_token: &'a str,
    client_id: &'a str,
}

/// Exchange the authorization code returned by the redirect for tokens.
///
/// `code` may include the `#state=...` fragment that Anthropic appends — the
/// `#` and everything after it is stripped before submission.
pub async fn exchange_code(
    http: &Client,
    flow: &AuthFlow,
    code_input: &str,
) -> Result<OAuthTokens> {
    let (code, returned_state) = split_code_and_state(code_input);
    if let Some(returned) = returned_state {
        if returned != flow.state {
            return Err(ClaudeMaxError::InvalidState);
        }
    }
    let body = ExchangeBody {
        grant_type: "authorization_code",
        code,
        state: &flow.state,
        client_id: OAUTH_CLIENT_ID,
        redirect_uri: OAUTH_REDIRECT_URI,
        code_verifier: &flow.pkce.verifier,
    };
    let resp = http
        .post(OAUTH_TOKEN_URL)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/json")
        .json(&body)
        .send()
        .await?;
    parse_token_response(resp).await
}

pub async fn refresh(http: &Client, refresh_token: &str) -> Result<OAuthTokens> {
    let body = RefreshBody {
        grant_type: "refresh_token",
        refresh_token,
        client_id: OAUTH_CLIENT_ID,
    };
    let resp = http
        .post(OAUTH_TOKEN_URL)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/json")
        .json(&body)
        .send()
        .await?;
    parse_token_response(resp).await.map_err(|e| match e {
        ClaudeMaxError::CodeExchangeFailed(m) => ClaudeMaxError::RefreshFailed(m),
        other => other,
    })
}

async fn parse_token_response(resp: reqwest::Response) -> Result<OAuthTokens> {
    let status = resp.status();
    let body = resp.text().await?;
    if !status.is_success() {
        return Err(ClaudeMaxError::CodeExchangeFailed(format!(
            "{status}: {body}"
        )));
    }
    let parsed: TokenResponse = serde_json::from_str(&body)?;
    Ok(OAuthTokens::from_response(parsed, Utc::now()))
}

/// Anthropic's callback page often shows the code as `<code>#<state>`. Some
/// users paste only the code, others paste the whole thing. Tolerate both.
fn split_code_and_state(input: &str) -> (&str, Option<&str>) {
    let trimmed = input.trim();
    match trimmed.split_once('#') {
        Some((code, state)) => (code, Some(state)),
        None => (trimmed, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorize_url_has_required_query_params() {
        let flow = start().unwrap();
        let pairs: std::collections::HashMap<_, _> = flow
            .authorize_url
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        assert_eq!(
            pairs.get("client_id").map(String::as_str),
            Some(OAUTH_CLIENT_ID)
        );
        assert_eq!(
            pairs.get("redirect_uri").map(String::as_str),
            Some(OAUTH_REDIRECT_URI)
        );
        assert_eq!(pairs.get("response_type").map(String::as_str), Some("code"));
        assert_eq!(
            pairs.get("code_challenge_method").map(String::as_str),
            Some("S256")
        );
        assert!(pairs.contains_key("code_challenge"));
        assert!(pairs.contains_key("state"));
        assert!(pairs.get("scope").unwrap().contains("user:inference"));
    }

    #[test]
    fn split_code_handles_both_forms() {
        assert_eq!(split_code_and_state("abc"), ("abc", None));
        assert_eq!(split_code_and_state("abc#xyz"), ("abc", Some("xyz")));
        assert_eq!(split_code_and_state("  abc#xyz  "), ("abc", Some("xyz")));
    }
}
