//! Visual selection: model, selected text, cursor/selection tag for the header.
//!
//! Spec: F-VISUAL-01 (start/end), F-VISUAL-02 (state side of render), F-VISUAL-03 (selection info for comments),
//! F-HEADER-03 (`[cursor L12:C3]` / `[visual ...]` tag). Oracle: `vsel`, `selText`, `selTag`, `curTag` in `src/app.tsx`.
//! Owner: component `nav` (B). Must not render.

use crate::state::State;

/// Ordered inclusive selection (rows, cols in chars); `line` = whole rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sel {
    pub sr: usize,
    pub sc: usize,
    pub er: usize,
    pub ec: usize,
    pub line: bool,
}

/// Current selection ordered start <= end, `None` when none active.
pub fn selection(_state: &State) -> Option<Sel> {
    todo!("F-VISUAL-01")
}

/// Text of the selection, rows joined with `\n` (F-VISUAL-03).
pub fn selection_text(_state: &State, _sel: &Sel) -> String {
    todo!("F-VISUAL-03")
}

/// Char range `[from, to)` of `sel` inside row `row` for render (None when the row is outside).
pub fn sel_range_in_row(_state: &State, _sel: &Sel, _row: usize) -> Option<(usize, usize)> {
    todo!("F-VISUAL-02")
}

/// Header tag text WITHOUT trailing space: `cursor new L12:C3` or `visual L4:C1` (F-HEADER-03); caller wraps
/// in `[..] `.
pub fn tag_text(_state: &State) -> String {
    todo!("F-HEADER-03")
}

/// End selection (`v`/`V`/Esc) (F-VISUAL-01).
pub fn end(_state: &mut State) {
    todo!("F-VISUAL-01")
}
