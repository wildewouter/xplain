//! MCP modal (`M`), server start/stop flow, integrations flow, wiring of MCP events into state.
//!
//! Spec: F-MCPUI-01 (modal content, power row, integration rows, status, errors), F-MCPUI-02 (keys),
//! F-MCPUI-03 (start/stop effects, port busy), F-MCPUI-04 (autostart on `Event::Started`), F-INTEG-02..06
//! (check/register/unregister flow with `Integrations` trait, copy `c`, watch prompt `w`, masking, notes),
//! F-MCPSRV-11 (client list display data). Oracle: `src/components/McpModal.tsx`, `src/mcp/bridge.ts`,
//! `mcp*` code in `src/app.tsx`. Owner: component `shell` (G). Never names an agent: only the trait.
//! Must not render (`view::modals` draws from `state.overlay`, `state.mcp`, `state.integration_state`).

use crate::effect::Fx;
use crate::errors::IoReason;
use crate::event::{ReqId, TimerId};
use crate::integration::CommandResult;
use crate::keys::KeyEvent;
use crate::mcp::{ConnId, HttpRequest, McpEndpoint};
use crate::state::State;

/// `M` in cursor context. True when consumed.
pub fn on_normal_key(_state: &mut State, _key: KeyEvent, _fx: &mut Fx) -> bool {
    todo!("F-MCPUI-01")
}

/// Key while `Overlay::Mcp`.
pub fn on_key(_state: &mut State, _key: KeyEvent, _fx: &mut Fx) {
    todo!("F-MCPUI-02, F-INTEG-*")
}

/// `Event::Started`: autostart when configured (F-MCPUI-04).
pub fn on_started(_state: &mut State, _fx: &mut Fx) {
    todo!("F-MCPUI-04")
}

/// Emit `Effect::McpStart` (validates `env.mcp_port_raw` via `mcp::parse_port`).
pub fn start_server(_state: &mut State, _fx: &mut Fx) {
    todo!("F-MCPUI-03")
}

pub fn stop_server(_state: &mut State, _fx: &mut Fx) {
    todo!("F-MCPUI-03")
}

pub fn on_mcp_started(_state: &mut State, _req: ReqId, _result: Result<McpEndpoint, String>, _fx: &mut Fx) {
    todo!("F-MCPUI-03/04")
}

pub fn on_mcp_stopped(_state: &mut State, _req: ReqId, _fx: &mut Fx) {
    todo!("F-MCPUI-03")
}

/// `Event::CommandDone` for integration checks/registrations (F-INTEG-02..04).
pub fn on_command_done(_state: &mut State, _req: ReqId, _result: CommandResult, _fx: &mut Fx) {
    todo!("F-INTEG-02..04")
}

/// `Event::McpHttp`: `state.mcp.handle_http`, then `ask::apply_output`.
pub fn on_http(_state: &mut State, _req: HttpRequest, _fx: &mut Fx) {
    todo!("F-MCPSRV-*")
}

pub fn on_conn_closed(_state: &mut State, _conn: ConnId, _fx: &mut Fx) {
    todo!("F-MCPSRV-06")
}

/// `Event::Timer(PollTimeout)` / `Spinner` routing for MCP-owned timers.
pub fn on_timer(_state: &mut State, _id: TimerId, _fx: &mut Fx) {
    todo!("F-MCPSRV-06")
}

/// Textual reason helper for start failures shown in the modal.
pub fn start_error_text(_port: u16, _reason: IoReason) -> String {
    todo!("F-MCPUI-03")
}
