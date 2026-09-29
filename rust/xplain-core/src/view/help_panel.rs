//! Help panel drawn over everything.
//!
//! Spec: F-HELP-01 (panel content, title, levels), F-HELP-02 (context title), F-HELP-03 (entries grouped),
//! F-LAYOUT-06 (help panel placement: top at screen row 3). Oracle: `src/components/HelpModal.tsx`.
//! Owner: component `viewframe` (F1). Data from `help::{help_ctx, ctx_label, entries_for, has_motions}`.

use crate::canvas::Canvas;
use crate::state::State;
use crate::theme::Theme;

pub fn draw(_c: &mut Canvas, _state: &State, _theme: &Theme) {
    todo!("F-HELP-01")
}
