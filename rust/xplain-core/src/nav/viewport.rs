//! Viewport: vertical `top`, horizontal `x_shift`, follow rules.
//!
//! Spec: F-NAV-09 (follow, margin m, comment boxes counted), F-NAV-10 (top reset keeps cursor),
//! F-CURSOR-07 (horizontal follow), F-LAYOUT-01 (H). Oracle: `fitOff`/`hoff` logic in `src/app.tsx`.
//! Owner: component `nav` (B). State only; never renders. Uses `thread_layout::row_extra_height`.

use crate::state::State;

/// Rows in the viewport (H).
pub fn height(_state: &State) -> usize {
    todo!("F-LAYOUT-01")
}

/// After a cursor change: apply F-NAV-09 (and F-CURSOR-07 horizontal follow). Editor open -> margin 0.
pub fn follow(_state: &mut State) {
    todo!("F-NAV-09, F-CURSOR-07")
}

/// F-NAV-10: top = 0 then follow, cursor untouched.
pub fn reset_top_keep_cursor(_state: &mut State) {
    todo!("F-NAV-10")
}

/// Clamp `top` to the bottom offset (whole comment blocks; never a partial block at top).
pub fn clamp_top(_state: &mut State) {
    todo!("F-NAV-09")
}

/// Scroll window text for the footer `(a-b/n)` (F-LAYOUT-02): returns `(a, b, n)`.
pub fn window(_state: &State) -> (usize, usize, usize) {
    todo!("F-LAYOUT-02")
}
