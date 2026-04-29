//! `claude-max` CLI: OAuth login + direct streaming chat against
//! `api.anthropic.com` using a Claude.ai Pro/Max subscription.
//!
//! ```text
//! claude-max login              # OAuth flow; tokens stored on disk
//! claude-max logout             # forget stored tokens
//! claude-max status             # print auth state
//! claude-max chat "hi"          # one-shot prompt, streamed to stdout
//! claude-max chat -             # prompt from stdin
//! claude-max chat @prompt.txt   # prompt from file
//! claude-max chat hi -s sys.txt # plus a custom system prompt
//! ```
//!
//! Tokens live at `$XDG_CONFIG_HOME/warp-claude-max/tokens.json` (mode 0600
//! on Unix). When invoked from inside Warp's `ClaudeMaxDirect` harness,
//! `chat` is the entry point.
//!
//! ⚠️ Anthropic's policy restricts these tokens to Claude Code and
//! Claude.ai. Using them via this binary is at the operator's risk.

use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use chrono::Utc;
use clap::{Args, Parser, Subcommand};
use claude_max::messages::{
    ContentBlock, Delta, Message, MessagesRequest, Role, StreamEvent, SystemPrompt,
};
use claude_max::{
    exchange_code, refresh, start, AnthropicClient, AuthFlow, ClaudeMaxError, FileTokenStore,
    OAuthTokens, Result, TokenStore,
};
use futures::StreamExt;
use reqwest::Client;

const DEFAULT_MODEL: &str = "claude-sonnet-4-5-20250929";
const DEFAULT_MAX_TOKENS: u32 = 4096;

#[derive(Parser)]
#[command(
    name = "claude-max",
    about = "Talk to Claude directly using your Pro/Max subscription's OAuth token.",
    long_about = None,
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run the OAuth flow and store tokens.
    Login,
    /// Remove the stored tokens.
    Logout,
    /// Print current auth status.
    Status,
    /// Send a one-shot prompt and stream the reply to stdout.
    Chat(ChatArgs),
}

#[derive(Args)]
struct ChatArgs {
    /// Prompt text. Use `-` for stdin or `@path` to read a file.
    prompt: String,
    /// Optional file with extra system instructions (wrapped with the
    /// claude-code identity prefix automatically — required by Anthropic's
    /// OAuth-Max policy enforcement).
    #[arg(short = 's', long = "system-prompt-file")]
    system_prompt_file: Option<PathBuf>,
    /// Anthropic model id.
    #[arg(short = 'm', long, default_value = DEFAULT_MODEL)]
    model: String,
    /// Maximum output tokens.
    #[arg(long, default_value_t = DEFAULT_MAX_TOKENS)]
    max_tokens: u32,
}

fn main() -> ExitCode {
    // The workspace's reqwest is built with `*-no-provider`; install a
    // crypto provider before any TLS handshake.
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();

    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("failed to start runtime: {e}");
            return ExitCode::FAILURE;
        }
    };

    let result: std::result::Result<(), Box<dyn std::error::Error>> = runtime.block_on(run());
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let store = FileTokenStore::new(FileTokenStore::default_path()?);
    let http = Client::builder().build()?;

    match cli.cmd {
        Cmd::Login => login(&http, &store).await?,
        Cmd::Logout => {
            store.clear()?;
            println!("Tokens cleared.");
        }
        Cmd::Status => status(&store),
        Cmd::Chat(args) => chat(&http, &store, args).await?,
    }
    Ok(())
}

fn status(store: &dyn TokenStore) {
    match store.load() {
        Ok(Some(tokens)) => {
            let expired = tokens.is_expired(Utc::now());
            let kind = if tokens.refresh_token.is_some() {
                "access + refresh"
            } else {
                "access only"
            };
            let when = tokens
                .expires_at
                .map(|e| format!("expires_at={e}"))
                .unwrap_or_else(|| "no expiry".to_string());
            println!(
                "logged in: {kind}, {when}, currently {}",
                if expired {
                    "expired (refresh on next chat)"
                } else {
                    "valid"
                },
            );
        }
        Ok(None) => println!("not logged in. run `claude-max login`."),
        Err(e) => eprintln!("could not read tokens: {e}"),
    }
}

async fn login(http: &Client, store: &dyn TokenStore) -> Result<()> {
    let flow: AuthFlow = start()?;
    eprintln!("Open this URL in your browser, sign in to Claude, and approve:");
    eprintln!();
    eprintln!("  {}", flow.authorize_url);
    eprintln!();
    eprint!("Paste the code from the callback page (e.g. abc123#xyz789): ");
    io::stderr().flush().ok();

    let mut line = String::new();
    io::stdin()
        .read_line(&mut line)
        .map_err(ClaudeMaxError::Io)?;

    let tokens = exchange_code(http, &flow, &line).await?;
    store.save(&tokens)?;
    eprintln!(
        "Logged in. Stored tokens (access_token len={}{}).",
        tokens.access_token.len(),
        if tokens.refresh_token.is_some() {
            ", with refresh"
        } else {
            ""
        },
    );
    Ok(())
}

async fn chat(http: &Client, store: &dyn TokenStore, args: ChatArgs) -> Result<()> {
    let prompt = read_prompt(&args.prompt).map_err(ClaudeMaxError::Io)?;
    let system_extra = match args.system_prompt_file {
        Some(p) => Some(std::fs::read_to_string(p).map_err(ClaudeMaxError::Io)?),
        None => None,
    };

    let tokens = ensure_fresh_tokens(http, store).await?;
    let client = AnthropicClient::new(http.clone());

    let req = MessagesRequest {
        model: args.model,
        max_tokens: args.max_tokens,
        messages: vec![Message {
            role: Role::User,
            content: vec![ContentBlock::text(prompt)],
        }],
        system: Some(build_system_prompt(system_extra.as_deref())),
        temperature: None,
        stream: true,
    };

    let mut stream = client.stream_messages(&tokens.access_token, req)?;
    let mut had_output = false;
    while let Some(event) = stream.next().await {
        match event? {
            StreamEvent::ContentBlockDelta {
                delta: Delta::Text { text },
                ..
            } => {
                print!("{text}");
                io::stdout().flush().ok();
                had_output = true;
            }
            StreamEvent::MessageStop => break,
            StreamEvent::Error { error } => {
                eprintln!(
                    "\nanthropic returned an error: {} — {}",
                    error.kind, error.message,
                );
                return Err(ClaudeMaxError::ApiError {
                    status: 0,
                    body: format!("{}: {}", error.kind, error.message),
                });
            }
            _ => {}
        }
    }
    if had_output {
        // Tail newline so subsequent shell prompts don't paste onto the
        // last token.
        println!();
    }
    Ok(())
}

/// Build the OAuth-Max-safe system field. Always begins with the
/// claude-code identity block; any extra content the caller supplied
/// follows as a second block.
fn build_system_prompt(extra: Option<&str>) -> SystemPrompt {
    match extra.map(str::trim).filter(|s| !s.is_empty()) {
        Some(extra) => SystemPrompt::with_identity_prefix(extra),
        None => SystemPrompt::with_identity_prefix(
            "You are a helpful assistant connected directly to a developer's terminal. \
             Keep replies concise unless asked otherwise.",
        ),
    }
}

/// Resolve the prompt argument:
///   - `-`        → stdin
///   - `@path`    → file
///   - anything else → literal string
fn read_prompt(arg: &str) -> std::io::Result<String> {
    if arg == "-" {
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf)?;
        Ok(buf)
    } else if let Some(path) = arg.strip_prefix('@') {
        std::fs::read_to_string(path)
    } else {
        Ok(arg.to_string())
    }
}

async fn ensure_fresh_tokens(http: &Client, store: &dyn TokenStore) -> Result<OAuthTokens> {
    let tokens = store.load()?.ok_or(ClaudeMaxError::MissingRefreshToken)?;
    if !tokens.is_expired(Utc::now()) {
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
