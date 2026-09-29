//! Claude Code integration.
//!
//! Spec: F-INTEG-01..06 rows for `claude`. Owner: integrations lead.
//! Must not: perform IO; use only `xplain_core::integration` types.

use crate::cli::CliAgent;
use crate::common;
use xplain_core::mcp::McpEndpoint;

const BIN: &str = "claude";
const CHECK_ARGS: &[&str] = &["mcp", "get", "xplain"];
const REMOVE_ARGS: &[&str] = &["mcp", "remove", "xplain", "-s", "local"];
const HINT: &str =
    "Registered. Restart or resume the session (claude --resume), then paste the watch prompt.";

fn add_args(ep: &McpEndpoint) -> Vec<String> {
    let header = common::bearer_header(&ep.token);
    ["mcp", "add", "xplain", &ep.url, "--transport", "http", "--scope", "local", "--header", &header]
        .map(String::from)
        .to_vec()
}

fn register_text(ep: &McpEndpoint) -> String {
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

pub fn agent() -> CliAgent {
    CliAgent {
        id: "claude",
        label: "Claude Code",
        poll_seconds: 100,
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
    use crate::common::testutil::{ep, registration};
    use xplain_core::integration::AgentIntegration;

    #[test]
    fn catalog() {
        let i = agent();
        assert_eq!(i.id(), "claude");
        assert_eq!(i.label(), "Claude Code");
        assert_eq!(i.poll_seconds(), 100);
        assert!(i.can_register());
        assert!(registration(&i).needs_restart());
    }

    #[test]
    fn watch_uses_poll() {
        assert_eq!(agent().watch_prompt(&ep()), common::watch_prompt(100, ""));
    }

    #[test]
    fn argv() {
        let e = ep();
        let i = agent();
        let reg = registration(&i);
        let c = reg.check_command(&e);
        assert_eq!(c.program, "claude");
        assert_eq!(c.args, ["mcp", "get", "xplain"]);
        assert_eq!(c.timeout_ms, 20_000);
        let r = reg.register_commands(&e);
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
        let u = reg.unregister_command();
        assert_eq!(u.program, "claude");
        assert_eq!(u.args, r[0].args);
    }

    #[test]
    fn text_and_hint() {
        let want = "claude mcp add xplain http://127.0.0.1:4321/mcp --transport http --scope local --header \"Authorization: Bearer sekret\"\n\nThen restart/resume the session: claude --resume\nAllow the tools: claude --allowedTools \"mcp__xplain\"  (or add permission rule mcp__xplain)\nOptional: set env CLAUDE_CODE_MCP_AUTO_BACKGROUND_MS=0 to avoid auto-backgrounding long calls.";
        let i = agent();
        assert_eq!(i.register_text(&ep()), want);
        assert_eq!(
            registration(&i).register_hint(&ep()),
            "Registered. Restart or resume the session (claude --resume), then paste the watch prompt.; restart the agent session, then paste the watch prompt"
        );
    }
}
