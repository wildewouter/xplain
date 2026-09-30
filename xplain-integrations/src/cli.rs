//! Table-driven adapter for agents with a `<bin> mcp get/add/remove` CLI.
//!
//! Spec: F-INTEG-01..06. Owner: integrations lead.
//! Must not: perform IO; know a specific agent (each agent module supplies its own table row).

use crate::common;
use xplain_core::integration::{AgentIntegration, CliRegistration, CommandResult, CommandSpec, RegStatus};
use xplain_core::mcp::McpEndpoint;

/// One registrable agent CLI: static argv pieces plus the per-endpoint builders.
#[derive(Debug, Clone, Copy)]
pub struct CliAgent {
    pub id: &'static str,
    pub label: &'static str,
    pub poll_seconds: u32,
    pub bin: &'static str,
    pub check_args: &'static [&'static str],
    pub remove_args: &'static [&'static str],
    /// Full `add` argv (without the binary).
    pub add_args: fn(&McpEndpoint) -> Vec<String>,
    /// Bare hint; the note core shows is `common::compose_register_note(hint)`.
    pub hint: &'static str,
    /// F-INTEG-05 copy-paste text.
    pub register_text: fn(&McpEndpoint) -> String,
}

impl AgentIntegration for CliAgent {
    fn id(&self) -> &'static str {
        self.id
    }
    fn label(&self) -> &'static str {
        self.label
    }
    fn poll_seconds(&self) -> u32 {
        self.poll_seconds
    }
    fn register_text(&self, ep: &McpEndpoint) -> String {
        (self.register_text)(ep)
    }
    fn watch_prompt(&self, _ep: &McpEndpoint) -> String {
        common::watch_prompt(self.poll_seconds, "")
    }
    fn registration(&self) -> Option<&dyn CliRegistration> {
        Some(self)
    }
}

impl CliRegistration for CliAgent {
    fn needs_restart(&self) -> bool {
        true
    }
    fn check_command(&self, _ep: &McpEndpoint) -> CommandSpec {
        common::cli_command(self.bin, self.check_args)
    }
    fn parse_check(&self, ep: &McpEndpoint, result: &CommandResult) -> RegStatus {
        common::parse_check(ep, result)
    }
    fn register_commands(&self, ep: &McpEndpoint) -> Vec<CommandSpec> {
        let add = (self.add_args)(ep);
        let add: Vec<&str> = add.iter().map(String::as_str).collect();
        vec![common::cli_command(self.bin, self.remove_args), common::cli_command(self.bin, &add)]
    }
    fn register_hint(&self, _ep: &McpEndpoint) -> String {
        common::compose_register_note(self.hint)
    }
    fn unregister_command(&self) -> CommandSpec {
        common::cli_command(self.bin, self.remove_args)
    }
}
