//! Copilot integration.
//!
//! Spec: F-INTEG-01..06 rows for `copilot`. Owner: integrations lead.
//! Must not: perform IO; use only `xplain_core::integration` types.

use xplain_core::integration::{AgentIntegration, CommandResult, CommandSpec, RegStatus};
use xplain_core::mcp::McpEndpoint;

#[derive(Debug, Clone, Copy, Default)]
pub struct Copilot;

impl AgentIntegration for Copilot {
    fn id(&self) -> &'static str {
        "copilot"
    }
    fn label(&self) -> &'static str {
        "Copilot"
    }
    fn poll_seconds(&self) -> u32 {
        120
    }
    fn can_register(&self) -> bool {
        true
    }
    fn needs_restart(&self) -> bool {
        true
    }
    fn register_text(&self, _ep: &McpEndpoint) -> String {
        todo!("F-INTEG-05 text")
    }
    fn watch_prompt(&self, _ep: &McpEndpoint) -> String {
        todo!("F-INTEG-06 text")
    }
    fn check_command(&self, _ep: &McpEndpoint) -> Option<CommandSpec> {
        todo!("F-INTEG-01/02 check argv")
    }
    fn parse_check(&self, _ep: &McpEndpoint, _result: &CommandResult) -> RegStatus {
        todo!("F-INTEG-02")
    }
    fn register_commands(&self, _ep: &McpEndpoint) -> Vec<CommandSpec> {
        todo!("F-INTEG-03 remove then add argv")
    }
    fn register_hint(&self, _ep: &McpEndpoint) -> String {
        todo!("F-INTEG-03 hint")
    }
    fn unregister_command(&self) -> Option<CommandSpec> {
        todo!("F-INTEG-04 remove argv")
    }
}
