//! Quit flow.
//!
//! Spec: F-QUIT-01 (`q`, confirm dialog when `confirm_quit`, dialog keys y/Enter/n/Esc, unanswered/open
//! comments? see spec, MCP stop before exit), F-CLI-05 (Ctrl+C always exits 0 immediately), F-MCPUI-03
//! (`stop` replies then `McpStop` then `Exit`). Owner: component `shell` (G). Must not render.

use crate::effect::{Effect, Fx};
use crate::keys::{Key, KeyEvent};
use crate::mcp_ui;
use crate::state::{Overlay, State};

/// `q` in cursor context. True when consumed.
pub fn on_normal_key(state: &mut State, key: KeyEvent, fx: &mut Fx) -> bool {
    if key.mods.ctrl || key.key != Key::Char('q') {
        return false;
    }
    if state.settings.confirm_quit {
        state.overlay = Overlay::Quit;
    } else {
        exit_now(state, fx);
    }
    true
}

/// Key while `Overlay::Quit`.
pub fn on_key(state: &mut State, key: KeyEvent, fx: &mut Fx) {
    if key.mods.ctrl {
        return;
    }
    match key.key {
        Key::Char('y') | Key::Enter => exit_now(state, fx),
        Key::Char('n') | Key::Char('q') | Key::Esc => state.overlay = Overlay::None,
        _ => {}
    }
}

/// Leave now: stop MCP server if running (McpState::stop replies, `McpStop`), then `Exit{code:0}`.
pub fn exit_now(state: &mut State, fx: &mut Fx) {
    if state.mcp.running || state.mcp.endpoint.is_some() {
        mcp_ui::stop_server(state, fx);
    }
    fx.push(Effect::Exit { code: 0 });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::testutil::fake_state;

    #[test]
    fn f_quit_01_q_opens_dialog_when_confirm_on() {
        let mut s = fake_state();
        let mut fx = Vec::new();
        assert!(on_normal_key(&mut s, KeyEvent::ch('q'), &mut fx));
        assert_eq!(s.overlay, Overlay::Quit);
        assert!(fx.is_empty());
    }

    #[test]
    fn f_quit_01_q_exits_when_confirm_off() {
        let mut s = fake_state();
        s.settings.confirm_quit = false;
        let mut fx = Vec::new();
        assert!(on_normal_key(&mut s, KeyEvent::ch('q'), &mut fx));
        assert_eq!(fx, vec![Effect::Exit { code: 0 }]);
    }

    #[test]
    fn f_quit_01_dialog_keys() {
        for k in [KeyEvent::ch('n'), KeyEvent::ch('q'), KeyEvent::plain(Key::Esc)] {
            let mut s = fake_state();
            s.overlay = Overlay::Quit;
            let mut fx = Vec::new();
            on_key(&mut s, k, &mut fx);
            assert_eq!(s.overlay, Overlay::None);
            assert!(fx.is_empty());
        }
        for k in [KeyEvent::ch('y'), KeyEvent::plain(Key::Enter)] {
            let mut s = fake_state();
            s.overlay = Overlay::Quit;
            let mut fx = Vec::new();
            on_key(&mut s, k, &mut fx);
            assert_eq!(fx, vec![Effect::Exit { code: 0 }]);
        }
        let mut s = fake_state();
        s.overlay = Overlay::Quit;
        let mut fx = Vec::new();
        on_key(&mut s, KeyEvent::ch('x'), &mut fx);
        assert_eq!(s.overlay, Overlay::Quit);
    }

    #[test]
    fn f_quit_01_ctrl_q_ignored() {
        let mut s = fake_state();
        let mut fx = Vec::new();
        assert!(!on_normal_key(&mut s, KeyEvent::ctrl('q'), &mut fx));
    }
}
