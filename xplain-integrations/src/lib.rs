//! xplain-integrations: agent CLI integrations. The only crate that contains agent names, argv shapes
//! and agent-specific texts.
//!
//! Spec: F-INTEG-01..06 (catalog, check/register/unregister argv, register text, watch prompt).
//! Owner: integrations lead. Depends on `xplain-core` only for the trait and shared types.
//! Must not: perform IO (no process spawning, no file access): implementations return
//! `CommandSpec`s and interpret `CommandResult`s; core drives them through effects and the app
//! runtime runs them. Must not be depended on by `xplain-core`.

pub mod claude;
pub mod cli;
pub mod codex;
pub mod common;
pub mod copilot;
pub mod opencode;

use xplain_core::integration::Integrations;

/// Registry in MCP modal order: Claude Code, Codex, OpenCode, Copilot (F-MCPUI-01).
/// Adding an agent = new module + one line here.
pub fn all() -> Integrations {
    use std::sync::Arc;
    vec![
        Arc::new(claude::agent()),
        Arc::new(codex::agent()),
        Arc::new(opencode::OpenCode),
        Arc::new(copilot::agent()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::testutil::ep;

    /// Invariants every registered integration must satisfy, whatever the agent.
    #[test]
    fn registry_invariants() {
        let e = ep();
        let all = all();
        let ids: Vec<_> = all.iter().map(|a| a.id()).collect();
        assert_eq!(ids, ["claude", "codex", "opencode", "copilot"]);
        for a in &all {
            assert!(!a.label().is_empty() && a.poll_seconds() > 0, "{}", a.id());
            let watch = a.watch_prompt(&e);
            assert_eq!(watch.lines().count(), 7, "{}", a.id());
            assert!(watch.contains(&format!("wait_seconds={}.", a.poll_seconds())), "{}", a.id());
            assert!(a.register_text(&e).contains(&e.token), "{}", a.id());
            assert_eq!(a.can_register(), a.registration().is_some(), "{}", a.id());
            let Some(reg) = a.registration() else { continue };
            let cmds = reg.register_commands(&e);
            assert_eq!(cmds.len(), 2, "{}", a.id());
            assert_eq!(cmds[0], reg.unregister_command(), "{}: remove step", a.id());
            assert!(!cmds[0].args.iter().any(|x| x.contains(&e.token)), "{}", a.id());
            assert_eq!(reg.check_command(&e).timeout_ms, common::CLI_TIMEOUT_MS);
            assert!(
                reg.register_hint(&e).ends_with("; restart the agent session, then paste the watch prompt"),
                "{}",
                a.id()
            );
        }
    }
}
