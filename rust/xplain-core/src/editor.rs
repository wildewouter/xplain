//! Comment editor overlay: open new/edit/follow-up, typing, caret, submit.
//!
//! Spec: F-COMMENT-01 (open), F-COMMENT-02 (keys, paste newline collapse, Tab save/ask), F-COMMENT-03 (submit),
//! F-COMMENT-07 (edit), F-ASK-02/03 (ask on send, follow-up), F-VISUAL-03. Oracle: `ask`/`askText`/`askPos`
//! handling in `src/app.tsx`, `src/components/AskBox.tsx` (text model only).
//! Owner: component `comments` (D). Submitting in ask mode calls `ask::ask_comment` (component `agent`).
//! Must not render (box drawing = `view::thread_box`, layout = `thread_layout`).

use crate::effect::Fx;
use crate::keys::KeyEvent;
use crate::state::State;

/// Private editor state (add fields here).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EditorExt {}

/// `Enter`/`a` in cursor/browse/visual context opens a new comment editor (F-COMMENT-01). True when consumed.
pub fn on_cursor_key(_state: &mut State, _key: KeyEvent, _fx: &mut Fx) -> bool {
    todo!("F-COMMENT-01")
}

/// Open the editor for editing comment `id` (only without replies, F-COMMENT-07).
pub fn open_edit(_state: &mut State, _id: &str) {
    todo!("F-COMMENT-07")
}

/// Open the follow-up input for a done thread (F-ASK-03).
pub fn open_follow_up(_state: &mut State, _id: &str) {
    todo!("F-ASK-03")
}

/// Key while `Overlay::Editor`.
pub fn on_key(_state: &mut State, _key: KeyEvent, _fx: &mut Fx) {
    todo!("F-COMMENT-02/03")
}

pub fn on_paste(_state: &mut State, _text: &str) -> bool {
    todo!("F-COMMENT-02")
}
