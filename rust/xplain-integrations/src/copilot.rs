//! Copilot integration.
//!
//! Spec: F-INTEG-01..06 rows for `copilot`. Owner: integrations lead.
//! Must not: perform IO; use only `xplain_core::integration` types.

use crate::cli::CliAgent;
use crate::common;
use xplain_core::mcp::McpEndpoint;

const BIN: &str = "copilot";
const CHECK_ARGS: &[&str] = &["mcp", "get", "xplain", "--json"];
const REMOVE_ARGS: &[&str] = &["mcp", "remove", "xplain"];
const HINT: &str = "Registered. Restart copilot (or use /mcp), then paste the watch prompt.";

fn add_args(ep: &McpEndpoint) -> Vec<String> {
    let header = common::bearer_header(&ep.token);
    ["mcp", "add", "xplain", &ep.url, "--transport", "http", "--header", &header, "--timeout", "200000"]
        .map(String::from)
        .to_vec()
}

fn register_text(ep: &McpEndpoint) -> String {
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

pub fn agent() -> CliAgent {
    CliAgent {
        id: "copilot",
        label: "Copilot",
        poll_seconds: 120,
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
        assert_eq!(i.id(), "copilot");
        assert_eq!(i.label(), "Copilot");
        assert_eq!(i.poll_seconds(), 120);
        assert!(i.can_register());
        assert!(registration(&i).needs_restart());
    }

    #[test]
    fn watch_uses_poll() {
        assert_eq!(agent().watch_prompt(&ep()), common::watch_prompt(120, ""));
    }

    #[test]
    fn argv() {
        let e = ep();
        let i = agent();
        let reg = registration(&i);
        let c = reg.check_command(&e);
        assert_eq!(c.program, "copilot");
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
                "http://127.0.0.1:4321/mcp",
                "--transport",
                "http",
                "--header",
                "Authorization: Bearer sekret",
                "--timeout",
                "200000"
            ]
        );
        assert_eq!(reg.unregister_command().args, ["mcp", "remove", "xplain"]);
    }

    #[test]
    fn text_and_hint() {
        let want = "copilot mcp add xplain http://127.0.0.1:4321/mcp --transport http --header \"Authorization: Bearer sekret\" --timeout 200000\n\nThen restart copilot (or use /mcp).\nAllow the tools: copilot --allow-tool='xplain'  (or approve once when asked).\nSession-only alternative: copilot --additional-mcp-config @file --allow-tool='xplain'";
        let i = agent();
        assert_eq!(i.register_text(&ep()), want);
        assert_eq!(
            registration(&i).register_hint(&ep()),
            "Registered. Restart copilot (or use /mcp), then paste the watch prompt.; restart the agent session, then paste the watch prompt"
        );
    }
}
