//! xplain-integrations: agent CLI integrations. The only crate that contains agent names, argv shapes
//! and agent-specific texts.
//!
//! Spec: F-INTEG-01..06 (catalog, check/register/unregister argv, register text, watch prompt).
//! Owner: integrations lead. Depends on `xplain-core` only for the trait and shared types.
//! Must not: perform IO (no process spawning, no file access): implementations return
//! `CommandSpec`s and interpret `CommandResult`s; core drives them through effects and the app
//! runtime runs them. Must not be depended on by `xplain-core`.

pub mod claude;
pub mod codex;
pub mod copilot;
pub mod opencode;

use xplain_core::integration::Integrations;

/// Registry in MCP modal order: Claude Code, Codex, OpenCode, Copilot (F-MCPUI-01).
/// Adding an agent = new module + one line here.
pub fn all() -> Integrations {
    use std::sync::Arc;
    vec![
        Arc::new(claude::Claude),
        Arc::new(codex::Codex),
        Arc::new(opencode::OpenCode),
        Arc::new(copilot::Copilot),
    ]
}
