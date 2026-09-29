//! JSON-RPC 2.0 envelope and method dispatch.
//!
//! Spec: F-MCPSRV-03 (envelope: parse errors, batch, notifications -> 202, ids), F-MCPSRV-04 (initialize with
//! protocol negotiation and session id, ping, tools/list), F-MCPSRV-05 (tools/call routing, errors),
//! F-MCPSRV-11 (client registration). Oracle: `src/mcp/server.ts`. Owner: component `agent` (E).
//! Must not: implement tool bodies (tools.rs) or queue logic (hub.rs).

use super::{HttpRequest, McpOutput, McpState};

/// Dispatch a request that passed `http::precheck`: parse body, handle each message, build the response
/// effect (`Effect::HttpReply`) or park a long poll (no reply yet).
pub fn dispatch(_state: &mut McpState, _req: HttpRequest, _now_ms: i64) -> McpOutput {
    todo!("F-MCPSRV-03..05")
}
