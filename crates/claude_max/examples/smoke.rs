//! End-to-end smoke test for `claude_max`.
//!
//! Run:
//! ```
//! # First time: do the OAuth flow.
//! cargo run -p claude_max --example smoke -- login
//!
//! # Then send a one-shot prompt (uses cached tokens).
//! cargo run -p claude_max --example smoke -- chat "Say hi in one word."
//!
//! # Wipe the saved token.
//! cargo run -p claude_max --example smoke -- logout
//! ```
//!
//! Tokens are stored at `$XDG_CONFIG_HOME/warp-claude-max/tokens.json` with
//! mode 0600 (Unix). They are NOT encrypted — this is for development only.

use std::io::{self, BufRead, Write};

use claude_max::{
    exchange_code,
    messages::{ContentBlock, Delta, Message, MessagesRequest, Role, StreamEvent, SystemPrompt},
    refresh, start, AnthropicClient, AuthFlow, ClaudeMaxError, FileTokenStore, OAuthTokens, Result,
    TokenStore,
};
use futures::StreamExt;
use reqwest::Client;

const DEFAULT_MODEL: &str = "claude-sonnet-4-5-20250929";

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Workspace's reqwest uses `rustls-tls-native-roots-no-provider`; we have
    // to install a crypto provider ourselves before any TLS handshake.
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();

    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("help");
    let store = FileTokenStore::new(FileTokenStore::default_path()?);
    let http = Client::builder().build()?;

    match cmd {
        "login" => login(&http, &store).await?,
        "chat" => {
            let prompt = args
                .get(2)
                .cloned()
                .unwrap_or_else(|| "Reply with the single word: pong.".to_string());
            chat(&http, &store, &prompt).await?
        }
        "logout" => {
            store.clear()?;
            println!("Cleared stored tokens.");
        }
        _ => print_help(),
    }
    Ok(())
}

fn print_help() {
    eprintln!("subcommands: login | chat <prompt> | logout");
}

async fn login(http: &Client, store: &dyn TokenStore) -> Result<()> {
    let flow: AuthFlow = start()?;
    println!("Open this URL in your browser, sign in to Claude, and approve:");
    println!();
    println!("  {}", flow.authorize_url);
    println!();
    print!("Paste the code shown on the callback page (with or without #state suffix): ");
    io::stdout().flush().ok();

    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;

    diagnose_pasted_input(&line);

    let tokens = exchange_code(http, &flow, &line).await?;
    store.save(&tokens)?;
    println!(
        "Stored tokens (access_token len={}).",
        tokens.access_token.len()
    );
    Ok(())
}

/// Print a sanitized summary of what was pasted. Exposes structural problems
/// (empty input, accidental URL paste, control characters) without revealing
/// the secret value itself.
fn diagnose_pasted_input(raw: &str) {
    let trimmed = raw.trim();
    let len = trimmed.chars().count();
    let has_hash = trimmed.contains('#');
    let has_space = trimmed.contains(char::is_whitespace);
    let has_slash = trimmed.contains('/');
    let has_eq = trimmed.contains('=');
    let starts_with = trimmed.chars().take(4).collect::<String>();
    let ends_with = trimmed
        .chars()
        .rev()
        .take(4)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    let control_chars = trimmed.chars().filter(|c| c.is_control()).count();

    eprintln!(
        "[diag] pasted input: len={len} has_hash={has_hash} has_whitespace={has_space} \
         has_slash={has_slash} has_eq={has_eq} control_chars={control_chars} \
         starts_with={starts_with:?} ends_with={ends_with:?}"
    );
    if len == 0 {
        eprintln!("[diag] WARNING: empty input — nothing was pasted.");
    }
    if has_slash || has_eq {
        eprintln!(
            "[diag] WARNING: input looks like a URL or query string. \
             Paste only the short code shown on the callback page \
             (something like 'abc123def#xyz789'), not the full callback URL."
        );
    }
    if has_space {
        eprintln!("[diag] WARNING: whitespace inside input — likely contains stray characters.");
    }
}

async fn chat(http: &Client, store: &dyn TokenStore, prompt: &str) -> Result<()> {
    let tokens = ensure_fresh_tokens(http, store).await?;
    let client = AnthropicClient::new(http.clone());

    let req = MessagesRequest {
        model: DEFAULT_MODEL.into(),
        max_tokens: 256,
        messages: vec![Message {
            role: Role::User,
            content: vec![ContentBlock::text(prompt)],
        }],
        system: Some(SystemPrompt::with_identity_prefix(default_system_prompt())),
        temperature: None,
        stream: true,
    };

    let mut stream = client.stream_messages(&tokens.access_token, req)?;
    while let Some(event) = stream.next().await {
        match event? {
            StreamEvent::ContentBlockDelta {
                delta: Delta::Text { text },
                ..
            } => {
                print!("{text}");
                io::stdout().flush().ok();
            }
            StreamEvent::MessageStop => {
                println!();
                break;
            }
            StreamEvent::Error { error } => {
                eprintln!("\n[anthropic error] {}: {}", error.kind, error.message);
                break;
            }
            _ => {}
        }
    }
    Ok(())
}

async fn ensure_fresh_tokens(http: &Client, store: &dyn TokenStore) -> Result<OAuthTokens> {
    let tokens = store
        .load()?
        .ok_or_else(|| ClaudeMaxError::MissingRefreshToken)?;
    if !tokens.is_expired(chrono::Utc::now()) {
        return Ok(tokens);
    }
    let rt = tokens
        .refresh_token
        .as_deref()
        .ok_or(ClaudeMaxError::MissingRefreshToken)?;
    let new = refresh(http, rt).await?;
    store.save(&new)?;
    Ok(new)
}

fn default_system_prompt() -> &'static str {
    "You are a helpful assistant connected directly to a developer's terminal. \
     Keep replies concise unless asked otherwise."
}
