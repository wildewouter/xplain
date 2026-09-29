//! Drawing of `thread_layout::ThreadBox` (comment thread, agent note, editor) under a code row.
//!
//! Spec: F-COMMENT-04 (box render, focus style), F-COMMENT-10 (agent note look), F-ASK-05 (status, spinner
//! frame from `state.spinner`), F-ASK-06 (code block styling, highlight via `highlight`), F-ASK-08 (buttons),
//! F-THEME-02. Oracle: `src/components/AskBox.tsx`. Owner: component `viewrows` (F2).
//! Layout is decided by `thread_layout` (component `comments`); this file only maps tones to theme styles and
//! puts cells. Must not mutate state.

use crate::canvas::Canvas;
use crate::state::State;
use crate::theme::Theme;
use crate::thread_layout::ThreadBox;

/// Draw `b` starting at row `y` of the canvas; returns lines drawn (clipped at `max_y`).
pub fn draw_box(
    _c: &mut Canvas,
    _state: &State,
    _theme: &Theme,
    _b: &ThreadBox,
    _y: u16,
    _max_y: u16,
) -> u16 {
    todo!("F-COMMENT-04, F-ASK-05..08")
}
