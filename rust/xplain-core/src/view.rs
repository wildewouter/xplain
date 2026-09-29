//! The view entry point: `State` -> [`Screen`].
//!
//! Spec: F-LAYOUT-*, F-HEADER-*, F-THEME-02, F-CURSOR-02, F-VISUAL-02, F-COMMENT-04, F-HELP-01, modals.
//! Owner: core lead (skeleton); view workers own the row/modal/help renderers.
//! Must not: mutate state, do IO. Viewport `top`/`x_shift` are *state* (updated in `update`), the view
//! only reads them.

use crate::screen::Screen;
use crate::state::State;

/// Render the full screen at `state.size`. Pure. `Loading...` (dim) while `LoadState::Loading`;
/// error screen for `LoadState::Error`.
pub fn view(_state: &State) -> Screen {
    todo!("render")
}
