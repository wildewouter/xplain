//! Codex integration.
//!
//! Spec: F-INTEG-01..06 rows for `codex`. Owner: integrations lead.
//! Must not: perform IO; use only `xplain_core::integration` types.

use crate::common;
use xplain_core::integration::{AgentIntegration, CommandResult, CommandSpec, RegStatus};
use xplain_core::mcp::McpEndpoint;

const BIN: &str = "codex";
const REMOVE_ARGS: &[&str] = &["mcp", "remove", "xplain"];
const HINT: &str = "Registered. Codex reads the token from env: export XPLAIN_MCP_TOKEN=<token> before starting codex, then resume (codex resume --last).";

#[derive(Debug, Clone, Copy, Default)]
pub struct Codex;

impl AgentIntegration for Codex {
    fn id(&self) -> &'static str {
        "codex"
    }
    fn label(&self) -> &'static str {
        "Codex"
    }
    fn poll_seconds(&self) -> u32 {
        45
    }
    fn can_register(&self) -> bool {
        true
    }
    fn needs_restart(&self) -> bool {
        true
    }
    fn register_text(&self, ep: &McpEndpoint) -> String {
        format!(
            "export XPLAIN_MCP_TOKEN={token}\n\
codex mcp add xplain --url {url} --bearer-token-env-var XPLAIN_MCP_TOKEN\n\
\n\
Codex must be started with XPLAIN_MCP_TOKEN set in its environment.\n\
Optionally add to ~/.codex/config.toml under [mcp_servers.xplain] (your consent needed):\n\
\x20 tool_timeout_sec = 120\n\
\x20 default_tools_approval_mode = \"approve\"\n\
Then restart/resume: codex resume --last",
            url = ep.url,
            token = ep.token
        )
    }
    fn watch_prompt(&self, _ep: &McpEndpoint) -> String {
        common::watch_prompt(self.poll_seconds())
    }
    fn check_command(&self, _ep: &McpEndpoint) -> Option<CommandSpec> {
        Some(common::cli_command(BIN, &["mcp", "get", "xplain", "--json"]))
    }
    fn parse_check(&self, ep: &McpEndpoint, result: &CommandResult) -> RegStatus {
        common::parse_check(ep, result)
    }
    fn register_commands(&self, ep: &McpEndpoint) -> Vec<CommandSpec> {
        vec![
            common::cli_command(BIN, REMOVE_ARGS),
            common::cli_command(
                BIN,
                &["mcp", "add", "xplain", "--url", &ep.url, "--bearer-token-env-var", "XPLAIN_MCP_TOKEN"],
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
        let i = Codex;
        assert_eq!(i.id(), "codex");
        assert_eq!(i.label(), "Codex");
        assert_eq!(i.poll_seconds(), 45);
        assert!(i.can_register());
        assert!(i.needs_restart());
    }

    #[test]
    fn watch_uses_poll() {
        assert_eq!(Codex.watch_prompt(&ep()), common::watch_prompt(45));
    }

    #[test]
    fn parse_check_cases() {
        let e = ep();
        let i = Codex;
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
        let h = Codex.register_hint(&ep());
        assert!(h.ends_with("; restart the agent session, then paste the watch prompt"));
        assert!(h.starts_with("Registered."));
    }

    #[test]
    fn argv() {
        let e = ep();
        let i = Codex;
        let c = i.check_command(&e).unwrap();
        assert_eq!(c.program, "codex");
        assert_eq!(c.args, ["mcp", "get", "xplain", "--json"]);
        let r = i.register_commands(&e);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].args, ["mcp", "remove", "xplain"]);
        assert_eq!(
            r[1].args,
            [
                "mcp",
                "add",
                "xplain",
                "--url",
                "http://127.0.0.1:4321/mcp",
                "--bearer-token-env-var",
                "XPLAIN_MCP_TOKEN"
            ]
        );
        assert!(!r[1].args.iter().any(|a| a.contains("sekret")));
        assert_eq!(i.unregister_command().unwrap().args, ["mcp", "remove", "xplain"]);
    }

    #[test]
    fn text_and_hint() {
        let want = "export XPLAIN_MCP_TOKEN=sekret\ncodex mcp add xplain --url http://127.0.0.1:4321/mcp --bearer-token-env-var XPLAIN_MCP_TOKEN\n\nCodex must be started with XPLAIN_MCP_TOKEN set in its environment.\nOptionally add to ~/.codex/config.toml under [mcp_servers.xplain] (your consent needed):\n  tool_timeout_sec = 120\n  default_tools_approval_mode = \"approve\"\nThen restart/resume: codex resume --last";
        assert_eq!(Codex.register_text(&ep()), want);
        assert_eq!(
            Codex.register_hint(&ep()),
            "Registered. Codex reads the token from env: export XPLAIN_MCP_TOKEN=<token> before starting codex, then resume (codex resume --last).; restart the agent session, then paste the watch prompt"
        );
    }

    #[test]
    fn json_check_stale() {
        let e = ep();
        assert_eq!(Codex.parse_check(&e, &ok(0, r#"{"transport":{"url":"x"}}"#)), RegStatus::Registered);
        assert_eq!(Codex.parse_check(&e, &ok(0, r#"{"url":"http://other/mcp"}"#)), RegStatus::Stale);
    }
}
