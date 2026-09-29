//! Find (`/`, `n`, `N`) and goto-line (`:`).
//!
//! Spec: F-FIND-01..03, F-GOTO-01/02. Oracle: `sOpen/sText/term/matchRows/findNext`, `goLine` in `src/app.tsx`.
//! Owner: component `navops` (C). Uses `rows::{find_all, search_texts}` and `nav::place`. Must not render
//! (highlights are drawn by view rows from `state.find.term`).

use crate::effect::Fx;
use crate::keys::KeyEvent;
use crate::state::State;

/// Private find state (add fields here).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FindExt {}

/// `/` `n` `N` `:` in cursor context (opens the input or jumps). True when consumed.
pub fn on_normal_key(_state: &mut State, _key: KeyEvent, _fx: &mut Fx) -> bool {
    todo!("F-FIND-01/03, F-GOTO-01")
}

/// Key while `Overlay::Find` (typing, backspace, Enter jump, Esc cancel).
pub fn on_find_key(_state: &mut State, _key: KeyEvent, _fx: &mut Fx) {
    todo!("F-FIND-01")
}

/// Key while `Overlay::Goto`.
pub fn on_goto_key(_state: &mut State, _key: KeyEvent, _fx: &mut Fx) {
    todo!("F-GOTO-01/02")
}

/// Paste into the open find/goto input (newline runs collapse). False if neither is open.
pub fn on_paste(_state: &mut State, _text: &str) -> bool {
    todo!("F-FIND-01")
}

/// Rows whose search texts contain the active term (F-FIND-02).
pub fn match_rows(_state: &State) -> Vec<usize> {
    todo!("F-FIND-02")
}
