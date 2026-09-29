//! Header line, rule and footer line.
//!
//! Spec: F-HEADER-01 (diff header chips + colors), F-HEADER-02 (browse header), F-HEADER-03 (cursor tag via
//! `nav::visual::tag_text`), F-LAYOUT-01/02 (footer composition: find/goto input, term, note, `too narrow for
//! split`, `(a-b/n)`, hints from `help::footer_hints`), F-LAYOUT-07 (truncation). Oracle: header/footer parts
//! of `src/app.tsx` and `src/components/DiffView.tsx`. Owner: component `viewframe` (F1).

use crate::canvas::Canvas;
use crate::state::State;
use crate::theme::Theme;

pub fn draw_header(_c: &mut Canvas, _state: &State, _theme: &Theme) {
    todo!("F-HEADER-01/02/03")
}

/// Rule row (row 2) and the footer (last row).
pub fn draw_footer(_c: &mut Canvas, _state: &State, _theme: &Theme) {
    todo!("F-LAYOUT-01/02")
}

/// Footer text without style, for reuse in tests.
pub fn footer_text(_state: &State) -> String {
    todo!("F-LAYOUT-02")
}
