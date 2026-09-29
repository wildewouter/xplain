//! Cursor motions: vertical/horizontal moves, word motions dispatch, counts, pane switch.
//!
//! Spec: F-CURSOR-03 (vertical, col clamp, sticky end), F-CURSOR-04 (horizontal, wraps?), F-CURSOR-05 (count),
//! F-CURSOR-06 (`p`), F-CURSOR-08/09 (global keys, start position), F-NAV-01..04.
//! Owner: component `nav` (B). Must not: touch viewport `top` except via `viewport::follow`.

use crate::keys::KeyEvent;
use crate::state::State;

/// One motion key with the pending count. Returns true when the key was a motion (consumed).
pub fn apply(_state: &mut State, _key: KeyEvent) -> bool {
    todo!("F-CURSOR-03..06")
}

/// Digit handling for count prefix (`1-9` start, `0` continues) (F-CURSOR-05).
pub fn push_count_digit(_state: &mut State, _digit: char) -> bool {
    todo!("F-CURSOR-05")
}
