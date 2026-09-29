//! OpenCode integration.
//!
//! Spec: F-INTEG-01..06 rows for `opencode`. Owner: integrations lead.
//! Must not: perform IO; use only `xplain_core::integration` types.

use crate::common;
use xplain_core::integration::AgentIntegration;
use xplain_core::mcp::McpEndpoint;

#[derive(Debug, Clone, Copy, Default)]
pub struct OpenCode;

impl AgentIntegration for OpenCode {
    fn id(&self) -> &'static str {
        "opencode"
    }
    fn label(&self) -> &'static str {
        "OpenCode"
    }
    fn poll_seconds(&self) -> u32 {
        45
    }
    fn register_text(&self, ep: &McpEndpoint) -> String {
        format!(
            "Add to opencode.json (project) or ~/.config/opencode/opencode.json:\n\
\n\
\"mcp\": {{\n\
\x20 \"xplain\": {{\n\
\x20   \"type\": \"remote\",\n\
\x20   \"url\": \"{url}\",\n\
\x20   \"enabled\": true,\n\
\x20   \"oauth\": false,\n\
\x20   \"timeout\": 120000,\n\
\x20   \"headers\": {{\"Authorization\": \"Bearer {token}\"}}\n\
\x20 }}\n\
}}\n\
\n\
Optional, to skip approval prompts:\n\
\"permission\": {{\"xplain_*\": \"allow\"}}\n\
\n\
Tool names are prefixed by the server name (xplain_next_question).\n\
Restart opencode after editing.",
            url = ep.url,
            token = ep.token
        )
    }
    fn watch_prompt(&self, _ep: &McpEndpoint) -> String {
        common::watch_prompt(self.poll_seconds(), "xplain_")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::testutil::ep;

    #[test]
    fn catalog() {
        let i = OpenCode;
        assert_eq!(i.id(), "opencode");
        assert_eq!(i.label(), "OpenCode");
        assert_eq!(i.poll_seconds(), 45);
        assert!(!i.can_register());
        assert!(i.registration().is_none());
    }

    #[test]
    fn text() {
        let want = "Add to opencode.json (project) or ~/.config/opencode/opencode.json:\n\n\"mcp\": {\n  \"xplain\": {\n    \"type\": \"remote\",\n    \"url\": \"http://127.0.0.1:4321/mcp\",\n    \"enabled\": true,\n    \"oauth\": false,\n    \"timeout\": 120000,\n    \"headers\": {\"Authorization\": \"Bearer sekret\"}\n  }\n}\n\nOptional, to skip approval prompts:\n\"permission\": {\"xplain_*\": \"allow\"}\n\nTool names are prefixed by the server name (xplain_next_question).\nRestart opencode after editing.";
        assert_eq!(OpenCode.register_text(&ep()), want);
    }

    #[test]
    fn prompt_prefixed() {
        let p = OpenCode.watch_prompt(&ep());
        assert!(p.starts_with("Loop forever: call the `xplain_next_question` tool from the `xplain` MCP server with wait_seconds=45."));
        assert!(p.contains("answer it with the `xplain_answer` tool"));
        assert!(p.contains("call the `xplain_files_changed` tool"));
        assert!(!p.contains("`next_question`"));
        assert_eq!(p.lines().count(), 7);
    }
}
