//! Direct-to-Anthropic harness using a Claude.ai Pro/Max OAuth token.
//!
//! Unlike `claude_code` and `gemini`, this harness does NOT spawn an external
//! CLI subprocess. It speaks directly to `api.anthropic.com/v1/messages` via
//! the [`claude_max`] crate, using OAuth tokens stored in the platform
//! keychain (see [`crate::ai::claude_max_glue`]).
//!
//! ## Phase 2b status: scaffold only
//!
//! This file wires the harness into [`super::harness_kind`] dispatch and
//! satisfies the [`ThirdPartyHarness`] / [`HarnessRunner`] traits enough for
//! the workspace to build. The runner's `start` is intentionally
//! `unimplemented!()`: actually pumping the Anthropic SSE stream into Warp's
//! [`AgentDriver`] event protocol requires bridging work that is deferred to
//! Phase 2c.
//!
//! ## Why a stub anyway?
//!
//! Landing the dispatch hook now means: (a) [`Harness::ClaudeMaxDirect`] is
//! a real route, not a placeholder; (b) the build verifies all match arms
//! over `Harness` are exhaustive; (c) Phase 2c can fill in `start` without
//! touching the trait surface or dispatch.

use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use warp_cli::agent::Harness;
use warpui::{ModelHandle, ModelSpawner};

use crate::ai::ambient_agents::AmbientAgentTaskId;
use crate::server::server_api::ServerApi;
use crate::terminal::CLIAgent;

use super::super::terminal::{CommandHandle, TerminalDriver};
use super::super::{AgentDriver, AgentDriverError};
use super::{HarnessRunner, ResumePayload, SavePoint, ThirdPartyHarness};

pub(crate) struct ClaudeMaxDirectHarness;

#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
impl ThirdPartyHarness for ClaudeMaxDirectHarness {
    fn harness(&self) -> Harness {
        Harness::ClaudeMaxDirect
    }

    /// We share Claude's display assets — same vendor, same logo. The lack of
    /// a dedicated `CLIAgent::ClaudeMaxDirect` variant is intentional: that
    /// enum models *external* CLI agents, which we are not.
    fn cli_agent(&self) -> CLIAgent {
        CLIAgent::Claude
    }

    /// No CLI to validate. Token presence is checked at runtime by the
    /// runner; that produces a more actionable error than a setup-phase
    /// "missing on PATH" message would.
    fn validate(&self) -> Result<(), AgentDriverError> {
        Ok(())
    }

    /// No on-disk config to stage — the OAuth token lives in keychain.
    fn prepare_environment_config(
        &self,
        _working_dir: &Path,
        _system_prompt: Option<&str>,
        _secrets: &std::collections::HashMap<String, warp_managed_secrets::ManagedSecretValue>,
    ) -> Result<(), AgentDriverError> {
        Ok(())
    }

    fn build_runner(
        &self,
        _prompt: &str,
        _system_prompt: Option<&str>,
        _resumption_prompt: Option<&str>,
        _working_dir: &Path,
        _task_id: Option<AmbientAgentTaskId>,
        _server_api: Arc<ServerApi>,
        _terminal_driver: ModelHandle<TerminalDriver>,
        _resume: Option<ResumePayload>,
    ) -> Result<Box<dyn HarnessRunner>, AgentDriverError> {
        Ok(Box::new(ClaudeMaxDirectRunner))
    }
}

pub(crate) struct ClaudeMaxDirectRunner;

#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
impl HarnessRunner for ClaudeMaxDirectRunner {
    /// Phase 2c will: (1) load tokens via `claude_max_glue::load_tokens`,
    /// refreshing if expired; (2) construct an `AnthropicClient`; (3) call
    /// `stream_messages` with a `SystemPrompt::with_identity_prefix(...)` so
    /// the OAuth-Max policy fingerprint is satisfied; (4) translate
    /// `StreamEvent::ContentBlockDelta` events into terminal block writes
    /// driven through `terminal_driver`, returning a `CommandHandle` that
    /// resolves on `MessageStop`.
    async fn start(
        &self,
        _foreground: &ModelSpawner<AgentDriver>,
    ) -> Result<CommandHandle, AgentDriverError> {
        unimplemented!("claude_max_direct runner: SSE stream → terminal block bridge is Phase 2c")
    }

    async fn save_conversation(
        &self,
        _save_point: SavePoint,
        _foreground: &ModelSpawner<AgentDriver>,
    ) -> Result<()> {
        // Phase 2c: persist via crates/persistence (or skip — direct mode
        // may be one-shot only initially).
        Ok(())
    }

    async fn exit(&self, _foreground: &ModelSpawner<AgentDriver>) -> Result<()> {
        // Nothing to gracefully shut down yet — no streams or tasks owned.
        Ok(())
    }
}
