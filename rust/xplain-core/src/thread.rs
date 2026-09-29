//! Comment focus and thread interaction: J/K focus, numbered jump, edit/delete/ask keys, thread scroll,
//! code-block copy buttons, delete confirmation.
//!
//! Spec: F-COMMENT-06 (focus `J` `K`), F-COMMENT-07 (edit), F-COMMENT-08 (delete `D`, dialog keys),
//! F-COMMENT-09 (`)` `(` numbered jump, `cannot read` note), F-ASK-07 (thread scroll), F-ASK-08 (code copy),
//! F-ASK-02/03/04 (a/A keys route to `ask`). Oracle: focus/scrolls/btn/numJump code in `src/app.tsx`.
//! Owner: component `comments` (D). Must not render.

use std::collections::HashMap;

use crate::effect::Fx;
use crate::keys::KeyEvent;
use crate::state::State;

/// Per-thread scroll (F-ASK-07).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ThreadScroll {
    pub off: usize,
    pub follow: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ThreadUi {
    /// Comment id counter behind `comments::alloc_id`.
    pub next_id: u32,
    /// Creation sequence counter.
    pub next_seq: u64,
    pub scrolls: HashMap<String, ThreadScroll>,
    /// Picked code block in the focused thread (F-ASK-08): `(comment id, block index)`.
    pub picked_block: Option<(String, usize)>,
}

/// `J` `K` `)` `(` in cursor/browse/visual context (focus and numbered jumps). True when consumed.
pub fn on_cursor_key(_state: &mut State, _key: KeyEvent, _fx: &mut Fx) -> bool {
    todo!("F-COMMENT-06/09")
}

/// Key while a comment is focused (`Nav::focused_comment`): e Enter D a A d u j k g G up/down esc, motions
/// unfocus (F-COMMENT-06..08, F-ASK-07/08). True when consumed; otherwise the caller falls through to nav
/// (which unfocuses on motion keys).
pub fn on_focused_key(_state: &mut State, _key: KeyEvent, _fx: &mut Fx) -> bool {
    todo!("F-COMMENT-06..08, F-ASK-07/08")
}

/// Key while `Overlay::DeleteComment` (y/Enter delete, n/Esc cancel; `q` quits).
pub fn on_delete_dialog_key(_state: &mut State, _key: KeyEvent, _fx: &mut Fx) {
    todo!("F-COMMENT-08")
}

pub fn focus(_state: &mut State, _id: &str) {
    todo!("F-COMMENT-06")
}

pub fn unfocus(_state: &mut State) {
    todo!("F-COMMENT-06")
}
