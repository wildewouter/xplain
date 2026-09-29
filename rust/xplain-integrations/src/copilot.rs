//! Copilot integration.
//!
//! Spec: F-INTEG-01..06 rows for `copilot`. Owner: integrations lead.
//! Must not: perform IO; use only `xplain_core::integration` types.

use crate::common;
use xplain_core::integration::{AgentIntegration, CommandResult, CommandSpec, RegStatus};
use xplain_core::mcp::McpEndpoint;

const BIN: &str = "copilot";
const REMOVE_ARGS: &[&str] = &["mcp", "remove", "xplain"];
const HINT: &str = "Registered. Restart copilot (or use /mcp), then paste the watch prompt.";

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
    fn register_text(&self, ep: &McpEndpoint) -> String {
        format!(
            "copilot mcp add xplain {url} --transport http --header \"Authorization: Bearer {token}\" --timeout 200000\n\
\n\
Then restart copilot (or use /mcp).\n\
Allow the tools: copilot --allow-tool='xplain'  (or approve once when asked).\n\
Session-only alternative: copilot --additional-mcp-config @file --allow-tool='xplain'",
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
                &[
                    "mcp",
                    "add",
                    "xplain",
                    &ep.url,
                    "--transport",
                    "http",
                    "--header",
                    &common::bearer_header(&ep.token),
                    "--timeout",
                    "200000",
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
        let i = Copilot;
        assert_eq!(i.id(), "copilot");
        assert_eq!(i.label(), "Copilot");
        assert_eq!(i.poll_seconds(), 120);
        assert!(i.can_register());
        assert!(i.needs_restart());
    }

    #[test]
    fn watch_uses_poll() {
        assert_eq!(Copilot.watch_prompt(&ep()), common::watch_prompt(120));
    }

    #[test]
    fn parse_check_cases() {
        let e = ep();
        let i = Copilot;
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
        let h = Copilot.register_hint(&ep());
        assert!(h.ends_with("; restart the agent session, then paste the watch prompt"));
        assert!(h.starts_with("Registered."));
    }

    #[test]
    fn argv() {
        let e = ep();
        let i = Copilot;
        let c = i.check_command(&e).unwrap();
        assert_eq!(c.program, "copilot");
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
                "http://127.0.0.1:4321/mcp",
                "--transport",
                "http",
                "--header",
                "Authorization: Bearer sekret",
                "--timeout",
                "200000"
            ]
        );
        assert_eq!(i.unregister_command().unwrap().args, ["mcp", "remove", "xplain"]);
    }

    #[test]
    fn text_and_hint() {
        let want = "copilot mcp add xplain http://127.0.0.1:4321/mcp --transport http --header \"Authorization: Bearer sekret\" --timeout 200000\n\nThen restart copilot (or use /mcp).\nAllow the tools: copilot --allow-tool='xplain'  (or approve once when asked).\nSession-only alternative: copilot --additional-mcp-config @file --allow-tool='xplain'";
        assert_eq!(Copilot.register_text(&ep()), want);
        assert_eq!(
            Copilot.register_hint(&ep()),
            "Registered. Restart copilot (or use /mcp), then paste the watch prompt.; restart the agent session, then paste the watch prompt"
        );
    }
}
