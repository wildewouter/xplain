//! File and change jumps, initial position, cursor memory across reloads.
//!
//! Spec: F-NAV-05 (`]` `[`), F-NAV-06 (Tab/S-Tab), F-NAV-08 (initial position per file), F-NAV-10 (top reset
//! cases), F-RELOAD-03 (cursor kept, hops to comments in unchanged files), F-FILES-02 (open by index).
//! Oracle: `sw`, `changeStarts` use, reload cursor mapping in `src/app.tsx`.
//! Owner: component `navops` (C). Uses `nav::place`, `nav::viewport`. Must not: know the picker/search UI.

use crate::effect::Fx;
use crate::keys::KeyEvent;
use crate::state::State;

/// `]` `[` Tab BackTab in cursor context. Returns true when consumed.
pub fn on_key(_state: &mut State, _key: KeyEvent, _fx: &mut Fx) -> bool {
    todo!("F-NAV-05/06")
}

/// Switch to file `index` (clamped): reset selection/focus, `rows::ensure`, initial position (F-NAV-08),
/// top per F-NAV-10 when landing on the same file.
pub fn open_file(_state: &mut State, _index: usize) {
    todo!("F-NAV-06/08/10")
}

/// F-NAV-08 initial cursor row/col/top for the current file.
pub fn initial_position(_state: &mut State) {
    todo!("F-NAV-08")
}

/// Cursor identity captured before a reload to restore afterwards (F-RELOAD-03).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CursorMemo {
    pub path: Option<String>,
    pub row: usize,
    pub col: usize,
    pub top: usize,
    pub line_no: Option<u32>,
    pub deleted: bool,
    pub side_new: bool,
}

pub fn remember(_state: &State) -> CursorMemo {
    todo!("F-RELOAD-03")
}

/// After new `files` arrived: map the memo onto the new rows (keep path if present, else first file).
pub fn restore(_state: &mut State, _memo: &CursorMemo) {
    todo!("F-RELOAD-03")
}
