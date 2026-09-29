//! Quit flow.
//!
//! Spec: F-QUIT-01 (`q`, confirm dialog when `confirm_quit`, dialog keys y/Enter/n/Esc, unanswered/open
//! comments? see spec, MCP stop before exit), F-CLI-05 (Ctrl+C always exits 0 immediately), F-MCPUI-03
//! (`stop` replies then `McpStop` then `Exit`). Owner: component `shell` (G). Must not render.

use crate::effect::Fx;
use crate::keys::KeyEvent;
use crate::state::State;

/// `q` in cursor context. True when consumed.
pub fn on_normal_key(_state: &mut State, _key: KeyEvent, _fx: &mut Fx) -> bool {
    todo!("F-QUIT-01")
}

/// Key while `Overlay::Quit`.
pub fn on_key(_state: &mut State, _key: KeyEvent, _fx: &mut Fx) {
    todo!("F-QUIT-01")
}

/// Leave now: stop MCP server if running (McpState::stop replies, `McpStop`), then `Exit{code:0}`.
pub fn exit_now(_state: &mut State, _fx: &mut Fx) {
    todo!("F-QUIT-01, F-MCPUI-03")
}
