//! Codex integration.
//!
//! Spec: F-INTEG-01..06 rows for `codex`. Owner: integrations lead.
//! Must not: perform IO; use only `xplain_core::integration` types.

use crate::cli::CliAgent;
use xplain_core::mcp::McpEndpoint;

const BIN: &str = "codex";
const CHECK_ARGS: &[&str] = &["mcp", "get", "xplain", "--json"];
const REMOVE_ARGS: &[&str] = &["mcp", "remove", "xplain"];
const HINT: &str = "Registered. Codex reads the token from env: export XPLAIN_MCP_TOKEN=<token> before starting codex, then resume (codex resume --last).";

fn add_args(ep: &McpEndpoint) -> Vec<String> {
    ["mcp", "add", "xplain", "--url", &ep.url, "--bearer-token-env-var", "XPLAIN_MCP_TOKEN"]
        .map(String::from)
        .to_vec()
}

fn register_text(ep: &McpEndpoint) -> String {
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

pub fn agent() -> CliAgent {
    CliAgent {
        id: "codex",
        label: "Codex",
        poll_seconds: 45,
        bin: BIN,
        check_args: CHECK_ARGS,
        remove_args: REMOVE_ARGS,
        add_args,
        hint: HINT,
        register_text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common;
    use crate::common::testutil::{ep, ok, registration};
    use xplain_core::integration::{AgentIntegration, RegStatus};

    #[test]
    fn catalog() {
        let i = agent();
        assert_eq!(i.id(), "codex");
        assert_eq!(i.label(), "Codex");
        assert_eq!(i.poll_seconds(), 45);
        assert!(i.can_register());
        assert!(registration(&i).needs_restart());
    }

    #[test]
    fn watch_uses_poll() {
        assert_eq!(agent().watch_prompt(&ep()), common::watch_prompt(45, ""));
    }

    #[test]
    fn argv() {
        let e = ep();
        let i = agent();
        let reg = registration(&i);
        let c = reg.check_command(&e);
        assert_eq!(c.program, "codex");
        assert_eq!(c.args, ["mcp", "get", "xplain", "--json"]);
        let r = reg.register_commands(&e);
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
        assert_eq!(reg.unregister_command().args, ["mcp", "remove", "xplain"]);
    }

    #[test]
    fn text_and_hint() {
        let want = "export XPLAIN_MCP_TOKEN=sekret\ncodex mcp add xplain --url http://127.0.0.1:4321/mcp --bearer-token-env-var XPLAIN_MCP_TOKEN\n\nCodex must be started with XPLAIN_MCP_TOKEN set in its environment.\nOptionally add to ~/.codex/config.toml under [mcp_servers.xplain] (your consent needed):\n  tool_timeout_sec = 120\n  default_tools_approval_mode = \"approve\"\nThen restart/resume: codex resume --last";
        let i = agent();
        assert_eq!(i.register_text(&ep()), want);
        assert_eq!(
            registration(&i).register_hint(&ep()),
            "Registered. Codex reads the token from env: export XPLAIN_MCP_TOKEN=<token> before starting codex, then resume (codex resume --last).; restart the agent session, then paste the watch prompt"
        );
    }

    #[test]
    fn json_check_stale() {
        let e = ep();
        let i = agent();
        let reg = registration(&i);
        assert_eq!(reg.parse_check(&e, &ok(0, r#"{"transport":{"url":"x"}}"#)), RegStatus::Registered);
        assert_eq!(reg.parse_check(&e, &ok(0, r#"{"url":"http://other/mcp"}"#)), RegStatus::Stale);
    }
}
