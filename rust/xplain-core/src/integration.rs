//! Integration trait and shared command types. Implementations live in `xplain-integrations`.
//!
//! Spec: F-INTEG-01..06, F-MCPUI-01/02. Owner: core lead (types frozen; texts and argv are the
//! integrations lead's job).
//! Must not: perform IO. An integration is a pure description: which commands to run, how to read their
//! output, which texts to show. Core drives the flow (confirm, busy, notes, re-check) and emits
//! `Effect::RunCommand`. Agent names appear only in `xplain-integrations`.

use std::fmt::Debug;
use std::sync::Arc;

use crate::errors::IoReason;
use crate::mcp::McpEndpoint;

/// A process to run: `program args...` in `cwd` (default: `--cwd`/process cwd), env inherited plus `env`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    /// `None` = app cwd (`--cwd`).
    pub cwd: Option<String>,
    pub env: Vec<(String, String)>,
    /// UNSPEC-37: current app uses 20 s for integration CLIs.
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandError {
    /// Program not found on PATH.
    NotFound,
    Timeout,
    /// Any other spawn error (shown as `<label> register failed: <reason>`).
    Other(IoReason),
}

pub type CommandResult = Result<CommandOutput, CommandError>;

/// Registration status shown in the MCP modal (F-MCPUI-01, F-INTEG-02).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RegStatus {
    #[default]
    NotRegistered,
    Registered,
    Stale,
}

/// One agent. Object safe; registry order = MCP modal order (concrete agents: see xplain-integrations).
/// Copy-paste-only agents implement just this; registrable ones also return [`CliRegistration`].
pub trait AgentIntegration: Debug + Send + Sync {
    /// Stable id (see xplain-integrations). Only used as a key, never shown.
    fn id(&self) -> &'static str;
    /// Shown label in the MCP modal.
    fn label(&self) -> &'static str;
    /// Long-poll seconds for the watch prompt (F-INTEG-01 table).
    fn poll_seconds(&self) -> u32;

    /// F-INTEG-05 text for `c`.
    fn register_text(&self, ep: &McpEndpoint) -> String;
    /// F-INTEG-06 text for `w`.
    fn watch_prompt(&self, ep: &McpEndpoint) -> String;

    /// `None` for copy-paste-only integrations: no register/unregister/check.
    fn registration(&self) -> Option<&dyn CliRegistration> {
        None
    }
    /// False for copy-paste-only integrations.
    fn can_register(&self) -> bool {
        self.registration().is_some()
    }
}

/// The CLI-driven half of an integration (F-INTEG-02..04): check, register, unregister.
pub trait CliRegistration {
    /// Row shows `  restart needed` (UNSPEC-5).
    fn needs_restart(&self) -> bool;
    /// F-INTEG-02 check command.
    fn check_command(&self, ep: &McpEndpoint) -> CommandSpec;
    /// F-INTEG-02: interpret a *finished* check command (`Err` = spawn failure => NotRegistered).
    fn parse_check(&self, ep: &McpEndpoint, result: &CommandResult) -> RegStatus;
    /// F-INTEG-03: commands run in order; failures of all but the last are ignored (remove, then add).
    fn register_commands(&self, ep: &McpEndpoint) -> Vec<CommandSpec>;
    /// Full note text shown after a successful add, exactly as the spec composes it (F-INTEG-03:
    /// `<hint>; restart the agent session, then paste the watch prompt`). Core shows it verbatim, masking the token.
    fn register_hint(&self, ep: &McpEndpoint) -> String;
    /// F-INTEG-04 remove command.
    fn unregister_command(&self) -> CommandSpec;
}

/// Registry handed to `State::new`. Built by `xplain_integrations::all()`; tests may pass fakes.
pub type Integrations = Vec<Arc<dyn AgentIntegration>>;
