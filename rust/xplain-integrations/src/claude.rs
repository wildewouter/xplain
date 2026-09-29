//! Claude Code integration.
//!
//! Spec: F-INTEG-01..06 rows for `claude`. Owner: integrations lead.
//! Must not: perform IO; use only `xplain_core::integration` types.

use crate::common;
use xplain_core::integration::{AgentIntegration, CommandResult, CommandSpec, RegStatus};
use xplain_core::mcp::McpEndpoint;

const BIN: &str = "claude";
const REMOVE_ARGS: &[&str] = &["mcp", "remove", "xplain", "-s", "local"];
const HINT: &str =
    "Registered. Restart or resume the session (claude --resume), then paste the watch prompt.";

#[derive(Debug, Clone, Copy, Default)]
pub struct Claude;

impl AgentIntegration for Claude {
    fn id(&self) -> &'static str {
        "claude"
    }
    fn label(&self) -> &'static str {
        "Claude Code"
    }
    fn poll_seconds(&self) -> u32 {
        100
    }
    fn can_register(&self) -> bool {
        true
    }
    fn needs_restart(&self) -> bool {
        true
    }
    fn register_text(&self, ep: &McpEndpoint) -> String {
        format!(
            "claude mcp add xplain {url} --transport http --scope local --header \"Authorization: Bearer {token}\"\n\
\n\
Then restart/resume the session: claude --resume\n\
Allow the tools: claude --allowedTools \"mcp__xplain\"  (or add permission rule mcp__xplain)\n\
Optional: set env CLAUDE_CODE_MCP_AUTO_BACKGROUND_MS=0 to avoid auto-backgrounding long calls.",
            url = ep.url,
            token = ep.token
        )
    }
    fn watch_prompt(&self, _ep: &McpEndpoint) -> String {
        common::watch_prompt(self.poll_seconds())
    }
    fn check_command(&self, _ep: &McpEndpoint) -> Option<CommandSpec> {
        Some(common::cli_command(BIN, &["mcp", "get", "xplain"]))
    }
    fn parse_check(&self, ep: &McpEndpoint, result: &CommandResult) -> RegStatus {
        common::parse_check(ep, result)
    }
    fn register_commands(&self, ep: &McpEndpoint) -> Vec<CommandSpec> {
        vec![
            common::cli_command(BIN, REMOVE_ARGS),
            common::cli_command(
                BIN,
                &[
                    "mcp",
                    "add",
                    "xplain",
                    &ep.url,
                    "--transport",
                    "http",
                    "--scope",
                    "local",
                    "--header",
                    &common::bearer_header(&ep.token),
                ],
            ),
        ]
    }
    fn register_hint(&self, _ep: &McpEndpoint) -> String {
        common::compose_register_note(HINT)
    }
    fn unregister_command(&self) -> Option<CommandSpec> {
        Some(common::cli_command(BIN, REMOVE_ARGS))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xplain_core::integration::{CommandError, CommandOutput};

    fn ep() -> McpEndpoint {
        McpEndpoint { url: "http://127.0.0.1:4321/mcp".into(), token: "sekret".into(), port: 4321 }
    }

    fn ok(code: i32, stdout: &str) -> CommandResult {
        Ok(CommandOutput { code, stdout: stdout.into(), stderr: String::new() })
    }

    #[test]
    fn catalog() {
        let i = Claude;
        assert_eq!(i.id(), "claude");
        assert_eq!(i.label(), "Claude Code");
        assert_eq!(i.poll_seconds(), 100);
        assert!(i.can_register());
        assert!(i.needs_restart());
    }

    #[test]
    fn watch_uses_poll() {
        assert_eq!(Claude.watch_prompt(&ep()), common::watch_prompt(100));
    }

    #[test]
    fn parse_check_cases() {
        let e = ep();
        let i = Claude;
        assert_eq!(i.parse_check(&e, &ok(0, "")), RegStatus::Registered);
        assert_eq!(i.parse_check(&e, &ok(0, "URL: http://127.0.0.1:4321/mcp")), RegStatus::Registered);
        assert_eq!(i.parse_check(&e, &ok(0, "URL: http://127.0.0.1:1/mcp")), RegStatus::Stale);
        assert_eq!(i.parse_check(&e, &ok(2, "")), RegStatus::NotRegistered);
        for err in [CommandError::NotFound, CommandError::Timeout, CommandError::Other("x".into())] {
            assert_eq!(i.parse_check(&e, &Err(err)), RegStatus::NotRegistered);
        }
    }

    #[test]
    fn hint_full_note() {
        let h = Claude.register_hint(&ep());
        assert!(h.ends_with("; restart the agent session, then paste the watch prompt"));
        assert!(h.starts_with("Registered."));
    }

    #[test]
    fn argv() {
        let e = ep();
        let i = Claude;
        let c = i.check_command(&e).unwrap();
        assert_eq!(c.program, "claude");
        assert_eq!(c.args, ["mcp", "get", "xplain"]);
        assert_eq!(c.timeout_ms, 20_000);
        let r = i.register_commands(&e);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].args, ["mcp", "remove", "xplain", "-s", "local"]);
        assert_eq!(
            r[1].args,
            [
                "mcp",
                "add",
                "xplain",
                "http://127.0.0.1:4321/mcp",
                "--transport",
                "http",
                "--scope",
                "local",
                "--header",
                "Authorization: Bearer sekret"
            ]
        );
        assert!(!r[0].args.iter().any(|a| a.contains("sekret")));
        let u = i.unregister_command().unwrap();
        assert_eq!(u.program, "claude");
        assert_eq!(u.args, r[0].args);
    }

    #[test]
    fn text_and_hint() {
        let want = "claude mcp add xplain http://127.0.0.1:4321/mcp --transport http --scope local --header \"Authorization: Bearer sekret\"\n\nThen restart/resume the session: claude --resume\nAllow the tools: claude --allowedTools \"mcp__xplain\"  (or add permission rule mcp__xplain)\nOptional: set env CLAUDE_CODE_MCP_AUTO_BACKGROUND_MS=0 to avoid auto-backgrounding long calls.";
        assert_eq!(Claude.register_text(&ep()), want);
        assert_eq!(
            Claude.register_hint(&ep()),
            "Registered. Restart or resume the session (claude --resume), then paste the watch prompt.; restart the agent session, then paste the watch prompt"
        );
    }
}
