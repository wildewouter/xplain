//! Shared helpers for the agent adapters (no agent names here either, keep it generic).
//!
//! Spec: F-INTEG-02 (check parsing, URL extraction), F-INTEG-06 (watch prompt template),
//! UNSPEC-37 (20 s timeout). Owner: integrations worker.
//! Must not: perform IO; know a specific agent; render notes (core composes and masks notes).

use xplain_core::integration::{CommandResult, CommandSpec, RegStatus};
use xplain_core::mcp::McpEndpoint;

/// Timeout for every integration CLI (UNSPEC-37).
pub const CLI_TIMEOUT_MS: u64 = 20_000;

/// `CommandSpec { program, args, cwd: None, env: [], timeout_ms: CLI_TIMEOUT_MS }`.
pub fn cli_command(_program: &str, _args: &[&str]) -> CommandSpec {
    todo!("F-INTEG-01 argv builder")
}

/// `Authorization: Bearer <token>` (single argv element value, no quotes).
pub fn bearer_header(_token: &str) -> String {
    todo!("F-INTEG-01")
}

/// F-INTEG-06 template with `<P>` = `poll_seconds`, tool names in backticks unchanged.
pub fn watch_prompt(_poll_seconds: u32) -> String {
    todo!("F-INTEG-06")
}

/// F-INTEG-02 URL extraction: JSON `url`, `xplain.url`, `mcpServers.xplain.url`, else first
/// `http(s)://` URL in text (ends at whitespace, `"`, `'`, `,`).
pub fn parse_url(_stdout: &str) -> Option<String> {
    todo!("F-INTEG-02")
}

/// F-INTEG-02 generic check: `Err` or non-zero exit => NotRegistered; exit 0 => Registered, or Stale
/// when a URL was found and differs from `ep.url`.
pub fn parse_check(_ep: &McpEndpoint, _result: &CommandResult) -> RegStatus {
    todo!("F-INTEG-02")
}

/// F-INTEG-03/05 composition: `<hint>` as shown after register, agent modules pass their hint text.
/// Returns the note exactly as the spec composes it: `<hint>; restart the agent session, then paste the watch prompt`.
/// NOTE: per trait doc, `register_hint` returns the full note; this helper builds it from the bare hint.
pub fn compose_register_note(_hint: &str) -> String {
    todo!("F-INTEG-03")
}
