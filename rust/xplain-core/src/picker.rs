//! File picker modal (state + keys). Rendering is `view::modals`.
//!
//! Spec: F-FILES-01 (open `f`, entries, status letters), F-FILES-02 (keys, Enter opens, top reset case).
//! Owner: component `navops` (C). Must not render.

use crate::diff::Status;
use crate::effect::Fx;
use crate::keys::KeyEvent;
use crate::state::State;

/// Scroll state of the picker window (add fields here).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PickerUi {
    pub top: usize,
}

/// One picker row for the view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerEntry {
    pub status: Status,
    pub label: String,
    pub adds: u32,
    pub dels: u32,
}

pub fn entries(_state: &State) -> Vec<PickerEntry> {
    todo!("F-FILES-01")
}

/// `f` in cursor context opens with the current file selected. True when consumed.
pub fn on_normal_key(_state: &mut State, _key: KeyEvent, _fx: &mut Fx) -> bool {
    todo!("F-FILES-01")
}

/// Key while `Overlay::Picker`.
pub fn on_key(_state: &mut State, _key: KeyEvent, _fx: &mut Fx) {
    todo!("F-FILES-02")
}

/// Visible window `[start, end)` of `entries` for a modal body of `height` rows, keeping `sel` in view.
pub fn window(_sel: usize, _total: usize, _height: usize) -> (usize, usize) {
    todo!("F-FILES-01 scroll")
}
