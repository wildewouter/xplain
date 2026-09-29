//! The view entry point: `State` -> [`Screen`].
//!
//! Spec: F-LAYOUT-01 (frame: header, rule, viewport, footer), F-MODE-05 (no changes screen), F-MODE-04 (error
//! screen), `Loading...` (dim), F-LAYOUT-06 (modal stacking: overlay, then help panel over it).
//! Owner: component `viewframe` (F1); `viewrows` (F2) owns `view/rows.rs`, `view/thread_box.rs`.
//! Must not: mutate state, do IO. Viewport `top`/`x_shift` are *state* (updated in `update`), the view
//! only reads them. Submodules are registered here up front.

pub mod header;
pub mod help_panel;
pub mod modals;
pub mod rows;
pub mod thread_box;

use crate::screen::Screen;
use crate::state::State;

/// Render the full screen at `state.size`. Pure. `Loading...` (dim) while `LoadState::Loading`;
/// error screen for `LoadState::Error`.
pub fn view(_state: &State) -> Screen {
    todo!("render")
}
