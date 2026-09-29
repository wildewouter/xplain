//! Viewport body: unified, split and browse rows with gutters, marks, cursor, selection, find hits, syntax
//! highlighting, and the comment/editor boxes interleaved under rows.
//!
//! Spec: F-LAYOUT-03/04/05 (row formats, split panes, widths), F-LAYOUT-07 (truncation), F-CURSOR-02
//! (cursor row bg `curBg`, cursor cell `▶`, char cursor inverse), F-VISUAL-02 (selection colors), F-FIND-02
//! (hit colors `FIND_HIT_*`), F-EDGE-08 (content chars), F-EDGE-06 (no-newline marker row), F-THEME-02,
//! F-CURSOR-07 (x_shift), F-NAV-09 (what rows the window shows, blocks never cut at top).
//! Oracle: `src/components/DiffView.tsx`. Owner: component `viewrows` (F2).
//! Uses `rows::*`, `highlight::*`, `thread_layout::boxes_at` via `thread_box::draw_boxes`. Must not mutate state.

use crate::canvas::{Canvas, Rect};
use crate::state::State;
use crate::theme::Theme;

/// Draw the viewport into `area` (rows 3..H+2 of the frame): rows from `nav.top` until the area is full.
pub fn draw_body(_c: &mut Canvas, _state: &State, _theme: &Theme, _area: Rect) {
    todo!("F-LAYOUT-03/04/05")
}

/// Width of the two number columns of unified rows for `n` (widens above 9999) (F-LAYOUT-03).
pub fn gutter_width(_max_no: u32) -> u16 {
    todo!("F-LAYOUT-03")
}
