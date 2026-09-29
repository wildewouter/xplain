//! Cursor, selection and viewport: keys and the primitives other modules use to move the cursor.
//!
//! Spec: F-CURSOR-01..10, F-VISUAL-01/02 (state part), F-NAV-01..04, F-NAV-07, F-NAV-09, F-NAV-10.
//! Oracle: `src/app.tsx` (cur/col/anchor/hoff/off handling, wordMove, fitOff).
//! Owner: component `nav` (B). Submodules registered here up front.
//! Must not: handle `]` `[` Tab (jump.rs), find/goto (find.rs), comment keys (thread.rs/editor.rs), or render.
//! Row heights of comment/editor boxes come from `thread_layout::row_extra_height` (component comments).

pub mod motion;
pub mod viewport;
pub mod visual;
pub mod word;

use crate::comments::PaneSide;
use crate::effect::Fx;
use crate::keys::KeyEvent;
use crate::screen::Size;
use crate::state::State;

/// Private nav state (add fields here). Example: remembered goal column for vertical moves.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NavExt {}

/// Viewport height H = max(3, rows-3) (F-LAYOUT-01).
pub fn body_height(_size: Size) -> usize {
    todo!("F-LAYOUT-01")
}

/// Handle a key in cursor/visual/browse context: hjkl w b e 0 ^ $ d u Space PgUp PgDn g G Home End
/// arrows, digits (count), `p`, `v` `V`, Esc chain (F-CURSOR-10, selection end). Ctrl combos ignored
/// (F-NAV-07). Returns true when consumed. Runs after overlays and comment focus had their turn.
pub fn on_key(_state: &mut State, _key: KeyEvent, _fx: &mut Fx) -> bool {
    todo!("F-CURSOR-*, F-NAV-01..04, F-VISUAL-01")
}

/// Put the cursor on `row` (clamped), optionally at `col`, clamp col, end no selection change, then follow.
/// Used by find/goto/jump/comments/reload.
pub fn place(_state: &mut State, _row: usize, _col: Option<usize>) {
    todo!("F-NAV-09 placement")
}

/// Shown (clamped) 0-based column of the cursor (F-HEADER-03).
pub fn shown_col(_state: &State) -> usize {
    todo!("F-CURSOR-04")
}

/// Pane the cursor really sits in (`rows::pane_of` on the current row and `nav.pane`).
pub fn cursor_pane(_state: &State) -> PaneSide {
    todo!("F-CURSOR-06")
}

/// Keep the cursor legal after rows changed (row < len, col clamp, pane).
pub fn clamp_cursor(_state: &mut State) {
    todo!("clamping")
}
