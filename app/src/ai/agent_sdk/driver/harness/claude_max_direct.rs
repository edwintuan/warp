//! Direct-to-Anthropic harness using a Claude.ai Pro/Max OAuth token.
//!
//! Unlike `claude_code` and `gemini`, this harness does not invoke a third-
//! party CLI shipped by another vendor. Instead it shells out to
//! `claude-max`, the small companion binary built from
//! `crates/claude_max/src/bin/claude_max.rs`. That binary owns the OAuth
//! flow, token storage, the policy-bypass system-prompt prefix, and the
//! Anthropic SSE streaming. The harness's job is just to plumb a Warp
//! terminal block into it.
//!
//! Lifecycle:
//!   1. `validate()` ensures `claude-max` is on PATH (install with
//!      `cargo install --path crates/claude_max`).
//!   2. `build_runner()` writes the prompt (and optional system prompt)
//!      to temp files, builds a shell command of the form
//!      `claude-max chat @prompt -s @sys`, and stages a runner that owns
//!      the temp files for cleanup.
//!   3. `start()` calls `TerminalDriver::execute_command(...)`. Output
//!      streams into a Warp block exactly like Claude/Gemini CLI.
//!
//! ⚠️ Anthropic's policy restricts Pro/Max OAuth tokens to Claude Code and
//! Claude.ai. Use elsewhere is at the operator's risk; the runner makes no
//! effort to hide what it's doing.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use parking_lot::Mutex;
use tempfile::NamedTempFile;
use warp_cli::agent::Harness;
use warp_managed_secrets::ManagedSecretValue;
use warpui::{ModelHandle, ModelSpawner};

use crate::ai::ambient_agents::AmbientAgentTaskId;
use crate::server::server_api::ServerApi;
use crate::terminal::CLIAgent;

use super::super::terminal::{CommandHandle, TerminalDriver};
use super::super::{AgentDriver, AgentDriverError};
use super::{
    validate_cli_installed, write_temp_file, HarnessRunner, ResumePayload, SavePoint,
    ThirdPartyHarness,
};

/// Name of the companion binary we shell out to. Built from
/// `crates/claude_max`; users install it with
/// `cargo install --path crates/claude_max`.
const CLAUDE_MAX_BIN: &str = "claude-max";

const INSTALL_DOCS: &str =
    "Install with: `cargo install --path crates/claude_max` from a Warp checkout.";

pub(crate) struct ClaudeMaxDirectHarness;

#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
impl ThirdPartyHarness for ClaudeMaxDirectHarness {
    fn harness(&self) -> Harness {
        Harness::ClaudeMaxDirect
    }

    /// Reuses Claude's display assets — same vendor, same logo. The
    /// `CLIAgent` enum models *external* CLI tools and `Claude` is the
    /// closest fit; we don't add a dedicated variant because that enum
    /// drives a lot of unrelated UI plumbing we don't need.
    fn cli_agent(&self) -> CLIAgent {
        CLIAgent::Claude
    }

    fn install_docs_url(&self) -> Option<&'static str> {
        // Not a URL — `validate_cli_installed` only ever embeds it in a
        // user-facing error string, so a one-line install command is more
        // useful than a docs link.
        Some(INSTALL_DOCS)
    }

    fn validate(&self) -> Result<(), AgentDriverError> {
        validate_cli_installed(CLAUDE_MAX_BIN, self.install_docs_url())
    }

    /// Nothing to stage on disk — `claude-max` reads tokens from its own
    /// keychain/file path, and the prompt/system-prompt are passed via
    /// args on the build_runner side.
    fn prepare_environment_config(
        &self,
        _working_dir: &Path,
        _system_prompt: Option<&str>,
        _secrets: &HashMap<String, ManagedSecretValue>,
    ) -> Result<(), AgentDriverError> {
        Ok(())
    }

    fn build_runner(
        &self,
        prompt: &str,
        system_prompt: Option<&str>,
        _resumption_prompt: Option<&str>,
        _working_dir: &Path,
        _task_id: Option<AmbientAgentTaskId>,
        _server_api: Arc<ServerApi>,
        terminal_driver: ModelHandle<TerminalDriver>,
        _resume: Option<ResumePayload>,
    ) -> Result<Box<dyn HarnessRunner>, AgentDriverError> {
        Ok(Box::new(ClaudeMaxDirectRunner::new(
            prompt,
            system_prompt,
            terminal_driver,
        )?))
    }
}

pub(crate) struct ClaudeMaxDirectRunner {
    command: String,
    /// Held so the temp files are cleaned up only when the runner is
    /// dropped — not when `build_runner` returns.
    _prompt_file: NamedTempFile,
    _system_prompt_file: Option<NamedTempFile>,
    terminal_driver: ModelHandle<TerminalDriver>,
    block_id: Mutex<Option<crate::terminal::model::block::BlockId>>,
}

impl ClaudeMaxDirectRunner {
    fn new(
        prompt: &str,
        system_prompt: Option<&str>,
        terminal_driver: ModelHandle<TerminalDriver>,
    ) -> Result<Self, AgentDriverError> {
        let prompt_file = write_temp_file("claude_max_prompt_", prompt)?;
        let prompt_path = prompt_file.path().display().to_string();

        let (system_prompt_file, system_arg) = match system_prompt {
            Some(sp) if !sp.trim().is_empty() => {
                let f = write_temp_file("claude_max_system_", sp)?;
                let p = f.path().display().to_string();
                (Some(f), format!(" -s '{p}'"))
            }
            _ => (None, String::new()),
        };

        // The `@` prefix tells `claude-max chat` to read the prompt from a
        // file. Quoting paths handles spaces; Warp's terminal runs commands
        // through a shell so quoting is important.
        let command = format!("{CLAUDE_MAX_BIN} chat '@{prompt_path}'{system_arg}");

        Ok(Self {
            command,
            _prompt_file: prompt_file,
            _system_prompt_file: system_prompt_file,
            terminal_driver,
            block_id: Mutex::new(None),
        })
    }
}

#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
impl HarnessRunner for ClaudeMaxDirectRunner {
    async fn start(
        &self,
        foreground: &ModelSpawner<AgentDriver>,
    ) -> Result<CommandHandle, AgentDriverError> {
        let command = self.command.clone();
        let terminal_driver = self.terminal_driver.clone();
        let command_handle = foreground
            .spawn(move |_, ctx| {
                terminal_driver.update(ctx, |driver, ctx| driver.execute_command(&command, ctx))
            })
            .await??
            .await?;

        *self.block_id.lock() = Some(command_handle.block_id().clone());
        Ok(command_handle)
    }

    async fn save_conversation(
        &self,
        _save_point: SavePoint,
        _foreground: &ModelSpawner<AgentDriver>,
    ) -> Result<()> {
        // Direct mode is one-shot: each chat is its own block, the
        // transcript lives in the terminal's regular block history. No
        // server-side conversation record to upload.
        Ok(())
    }

    async fn exit(&self, _foreground: &ModelSpawner<AgentDriver>) -> Result<()> {
        // The bin exits naturally on `MessageStop`; nothing to send.
        Ok(())
    }
}
