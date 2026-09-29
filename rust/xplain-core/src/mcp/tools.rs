//! MCP tool table and tool calls: `next_question`, `answer`, `annotate`, `get_questions`, `files_changed`.
//!
//! Spec: F-MCPSRV-05..10 (schemas, argument validation and clamps, result texts, `closed`), F-ASK-09
//! (answer arrival), F-COMMENT-10 (annotate). Oracle: `src/mcp/tools.ts`, `src/mcp/hub.ts` (sanitize).
//! Owner: component `agent` (E). Must not: do HTTP framing (rpc.rs).

use serde_json::Value;

use super::{ConnId, HubEvent, McpOutput, McpState};

/// Server instructions text and version constants (F-MCPSRV-04).
pub const SERVER_VERSION: &str = "0.1.0";
pub const PROTOCOL_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];
/// Default and max `wait_seconds` (F-MCPSRV-06).
pub const DEFAULT_WAIT_S: u64 = 45;
pub const MAX_WAIT_S: u64 = 120;

/// `tools/list` result array (F-MCPSRV-04).
pub fn tool_list() -> Value {
    todo!("F-MCPSRV-04")
}

/// Instructions string of `initialize` (F-MCPSRV-04).
pub fn instructions() -> &'static str {
    todo!("F-MCPSRV-04")
}

/// Result of one call: either finished with a JSON-RPC `result` value, or parked (long poll).
#[derive(Debug, Clone, PartialEq)]
pub enum ToolOutcome {
    Done {
        result: Value,
        events: Vec<HubEvent>,
    },
    /// `next_question` waiting: caller must arm `TimerId::PollTimeout(conn)` (background) for `wait_ms`.
    Parked {
        wait_ms: u64,
    },
}

/// Run tool `name` for client `client_id` (session id or `anon:<port>`). Unknown tool handled by rpc.rs.
pub fn call(
    _state: &mut McpState,
    _client_id: &str,
    _conn: ConnId,
    _name: &str,
    _args: &Value,
    _now_ms: i64,
) -> ToolOutcome {
    todo!("F-MCPSRV-05..10")
}

/// Helper for tests/rpc: wraps text into `{content:[{type:"text",text}], isError?}`.
pub fn text_result(_text: &str, _is_error: bool) -> Value {
    todo!("F-MCPSRV-05")
}

/// Strip ANSI escapes and control chars (keep `\n`, `\t`), cap 20000 chars (F-MCPSRV-07/08).
pub fn sanitize(_s: &str) -> String {
    todo!("F-MCPSRV-07")
}

/// Emit an `McpOutput` with no effects for a finished call (used by tests).
pub fn events_only(events: Vec<HubEvent>) -> McpOutput {
    McpOutput { effects: Vec::new(), events }
}
