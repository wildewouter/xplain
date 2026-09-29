//! Help: key table, contexts, footer hints, help panel state machine.
//!
//! Spec: F-HELP-01 (panel `?`, levels), F-HELP-02 (contexts + priority), F-HELP-03 (entries), F-HELP-04
//! (footer hints per state), F-LAYOUT-02 item 5. Oracle: `src/keys.ts` (KEYS, helpCtx, keysFor, footerFor)
//! and `src/components/HelpModal.tsx` (grouping only; drawing is `view::help_panel`).
//! Owner: component `navops` (C). Must not render.

use crate::effect::Fx;
use crate::keys::KeyEvent;
use crate::state::State;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HelpCtx {
    Cursor,
    Visual,
    Comment,
    Editor,
    Find,
    Goto,
    Picker,
    Search,
    Mcp,
    Config,
    Dialog,
    Browse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelpEntry {
    pub group: &'static str,
    pub keys: String,
    pub desc: String,
}

/// Context priority mirrors the key handler order (F-HELP-02).
pub fn help_ctx(_state: &State) -> HelpCtx {
    todo!("F-HELP-02")
}

/// Context title (`Diff view`, `Visual selection`, ...).
pub fn ctx_label(_ctx: HelpCtx) -> &'static str {
    todo!("F-HELP-02")
}

/// Entries for a context; `motions` shows vim motions (level 2) (F-HELP-03).
pub fn entries_for(_ctx: HelpCtx, _motions: bool) -> Vec<HelpEntry> {
    todo!("F-HELP-03")
}

/// Whether the context has motion entries to expand (`? more`).
pub fn has_motions(_ctx: HelpCtx) -> bool {
    todo!("F-HELP-01")
}

/// Footer key hints for the current state, e.g. `hjkl move  enter ask  J/K comments  ? help` (F-HELP-04).
pub fn footer_hints(_state: &State) -> String {
    todo!("F-HELP-04")
}

/// `?` and Esc handling while the panel is open or closed. Runs first in key dispatch: returns true when the
/// key was consumed (panel open swallows everything but its own keys, F-HELP-01).
pub fn on_key(_state: &mut State, _key: KeyEvent, _fx: &mut Fx) -> bool {
    todo!("F-HELP-01")
}
