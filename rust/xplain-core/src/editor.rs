//! Comment editor overlay: open new/edit/follow-up, typing, caret, submit.
//!
//! Spec: F-COMMENT-01 (open), F-COMMENT-02 (keys, paste newline collapse, Tab save/ask), F-COMMENT-03 (submit),
//! F-COMMENT-07 (edit), F-ASK-02/03 (ask on send, follow-up), F-VISUAL-03. Oracle: `ask`/`askText`/`askPos`
//! handling in `src/app.tsx`, `src/components/AskBox.tsx` (text model only).
//! Owner: component `comments` (D). Submitting in ask mode calls `ask::ask_comment` (component `agent`).
//! Must not render (box drawing = `view::thread_box`, layout = `thread_layout`).

use crate::ask;
use crate::comments::{self, Turn};
use crate::effect::Fx;
use crate::keys::{Key, KeyEvent};
use crate::nav;
use crate::state::{EditorKind, EditorState, Overlay, State};
use crate::thread::ThreadScroll;

const MCP_OFF: &str = "MCP is off (M to start)";

/// Private editor state (add fields here).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EditorExt {}

/// Save/ask mode in effect for `ed`: ask only while MCP runs (F-COMMENT-02).
pub fn effective_ask(state: &State, ed: &EditorState) -> bool {
    state.mcp.running && ed.ask_mode
}

/// Mode a new editor starts in: `ask` while MCP runs unless the user chose `save`; else `save`.
fn default_ask(state: &mut State) -> bool {
    if !state.mcp.running {
        state.thread.chosen = None;
        return false;
    }
    state.thread.chosen.unwrap_or(true)
}

fn open(state: &mut State, kind: EditorKind, text: String) {
    let ask_mode = matches!(kind, EditorKind::New) && default_ask(state);
    let caret = text.chars().count();
    state.nav.count = 0;
    state.overlay = Overlay::Editor(EditorState { kind, text, caret, ask_mode, ext: EditorExt::default() });
    nav::viewport::follow(state);
}

/// `Enter`/`a` in cursor/browse/visual context opens a new comment editor (F-COMMENT-01). True when consumed.
pub fn on_cursor_key(state: &mut State, key: KeyEvent, _fx: &mut Fx) -> bool {
    if key.mods.ctrl {
        return false;
    }
    if !matches!(key.key, Key::Enter | Key::Char('a')) {
        return false;
    }
    state.nav.focused_comment = None;
    open(state, EditorKind::New, String::new());
    true
}

/// Open the editor for editing comment `id` (only without follow-ups, F-COMMENT-07).
pub fn open_edit(state: &mut State, id: &str) {
    let Some(c) = comments::find(state, id) else { return };
    if comments::turn_count(c) > 1 {
        state.note = Some("can't edit after follow-ups".to_string());
        return;
    }
    let text = c.message.clone();
    open(state, EditorKind::Edit { id: id.to_string() }, text);
}

/// Open the follow-up input for a done thread (F-ASK-03).
pub fn open_follow_up(state: &mut State, id: &str) {
    if comments::find(state, id).is_none() {
        return;
    }
    open(state, EditorKind::FollowUp { id: id.to_string() }, String::new());
}

fn byte_at(s: &str, chars: usize) -> usize {
    s.char_indices().nth(chars).map_or(s.len(), |(i, _)| i)
}

fn insert_text(ed: &mut EditorState, s: &str) {
    let at = byte_at(&ed.text, ed.caret);
    ed.text.insert_str(at, s);
    ed.caret += s.chars().count();
}

/// Key while `Overlay::Editor`.
pub fn on_key(state: &mut State, key: KeyEvent, fx: &mut Fx) {
    state.nav.count = 0;
    let Overlay::Editor(ed) = &mut state.overlay else { return };
    match key.key {
        Key::Esc => {
            state.overlay = Overlay::None;
            nav::viewport::follow(state);
        }
        Key::Enter => submit(state, fx),
        Key::Tab => {
            if !matches!(ed.kind, EditorKind::New) {
                return;
            }
            if state.mcp.running && ed.ask_mode {
                ed.ask_mode = false;
                state.thread.chosen = Some(false);
            } else if state.mcp.running {
                ed.ask_mode = true;
                state.thread.chosen = Some(true);
            } else {
                state.thread.chosen = None;
                state.note = Some(MCP_OFF.to_string());
            }
        }
        Key::Left => ed.caret = ed.caret.saturating_sub(1),
        Key::Right => ed.caret = (ed.caret + 1).min(ed.text.chars().count()),
        Key::Backspace | Key::Delete => {
            if ed.caret > 0 {
                let from = byte_at(&ed.text, ed.caret - 1);
                let to = byte_at(&ed.text, ed.caret);
                ed.text.replace_range(from..to, "");
                ed.caret -= 1;
            }
        }
        Key::Char(c) if !key.mods.ctrl && !key.mods.alt => {
            if c == '\r' || c == '\n' {
                insert_text(ed, " ");
            } else {
                insert_text(ed, c.encode_utf8(&mut [0; 4]));
            }
        }
        _ => {}
    }
}

/// Bracketed paste into the editor: each CR/LF run becomes one space (F-COMMENT-02). True when consumed.
pub fn on_paste(state: &mut State, text: &str) -> bool {
    let Overlay::Editor(ed) = &mut state.overlay else { return false };
    let mut s = String::with_capacity(text.len());
    let mut in_break = false;
    for c in text.chars() {
        if c == '\r' || c == '\n' {
            if !in_break {
                s.push(' ');
            }
            in_break = true;
        } else {
            in_break = false;
            s.push(c);
        }
    }
    insert_text(ed, &s);
    true
}

/// Enter: submit per editor kind (F-COMMENT-03 / F-COMMENT-07 / F-ASK-03). Empty (trimmed) text does nothing.
fn submit(state: &mut State, fx: &mut Fx) {
    let Overlay::Editor(ed) = &state.overlay else { return };
    let message = ed.text.trim().to_string();
    if message.is_empty() {
        return;
    }
    let kind = ed.kind.clone();
    let ask_now = effective_ask(state, ed);
    match kind {
        EditorKind::FollowUp { id } => follow_up(state, &id, message, fx),
        EditorKind::Edit { id } => {
            if let Some(c) = state.comments.iter_mut().find(|c| c.id == id) {
                if c.turns.len() <= 1 {
                    c.message = message.clone();
                    match c.turns.first_mut() {
                        Some(t) => t.message = message,
                        None => c.turns.push(Turn { message, answer: None, prior: Vec::new() }),
                    }
                }
            }
            state.note = Some("comment updated".to_string());
            close(state);
        }
        EditorKind::New => {
            let comment = comments::from_cursor(state, &message);
            let id = comments::insert(state, comment);
            nav::visual::end(state);
            if ask_now {
                if state.mcp.running {
                    ask::ask_comment(state, &id, fx);
                    state.note = Some("question sent to agent".to_string());
                } else {
                    state.note = Some(MCP_OFF.to_string());
                }
            } else {
                state.note = Some(format!("question saved ({})", state.comments.len()));
            }
            close(state);
        }
    }
}

fn close(state: &mut State) {
    state.overlay = Overlay::None;
    nav::viewport::follow(state);
}

/// Follow-up submit (F-ASK-03): new turn queued, or the editor stays with a note.
fn follow_up(state: &mut State, id: &str, message: String, fx: &mut Fx) {
    if !state.mcp.running {
        state.note = Some(MCP_OFF.to_string());
        return;
    }
    let can = comments::find(state, id).is_some_and(comments::can_follow_up);
    if !can {
        state.note = Some("can't follow up yet".to_string());
        return;
    }
    if let Some(c) = state.comments.iter_mut().find(|c| c.id == id) {
        c.turns.push(Turn { message, answer: None, prior: Vec::new() });
    }
    ask::ask_comment(state, id, fx);
    state.thread.scrolls.insert(id.to_string(), ThreadScroll { off: 0, follow: true });
    state.note = Some("follow-up queued".to_string());
    close(state);
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::comments::testutil::{comment, ctx, state_with};

    fn key(state: &mut State, k: Key) {
        let mut fx = Vec::new();
        on_key(state, KeyEvent::plain(k), &mut fx);
    }

    fn type_str(state: &mut State, s: &str) {
        for c in s.chars() {
            key(state, Key::Char(c));
        }
    }

    fn editor(state: &State) -> &EditorState {
        match &state.overlay {
            Overlay::Editor(e) => e,
            other => panic!("not editor: {other:?}"),
        }
    }

    fn st() -> State {
        state_with(vec![ctx(1), ctx(2), ctx(3)])
    }

    #[test]
    fn f_comment_01_enter_and_a_open_editor() {
        for k in [KeyEvent::plain(Key::Enter), KeyEvent::ch('a')] {
            let mut s = st();
            assert!(on_cursor_key(&mut s, k, &mut Vec::new()));
            let e = editor(&s);
            assert_eq!(e.kind, EditorKind::New);
            assert!(e.text.is_empty() && e.caret == 0 && !e.ask_mode);
        }
        let mut s = st();
        assert!(!on_cursor_key(&mut s, KeyEvent::ch('x'), &mut Vec::new()));
        assert!(!on_cursor_key(&mut s, KeyEvent::ctrl('a'), &mut Vec::new()));
    }

    #[test]
    fn f_comment_02_typing_caret_backspace() {
        let mut s = st();
        on_cursor_key(&mut s, KeyEvent::ch('a'), &mut Vec::new());
        type_str(&mut s, "héllo");
        assert_eq!(editor(&s).text, "héllo");
        assert_eq!(editor(&s).caret, 5);
        key(&mut s, Key::Left);
        key(&mut s, Key::Left);
        key(&mut s, Key::Backspace);
        assert_eq!(editor(&s).text, "hélo");
        assert_eq!(editor(&s).caret, 2);
        key(&mut s, Key::Delete);
        assert_eq!(editor(&s).text, "hlo");
        type_str(&mut s, "?");
        assert_eq!(editor(&s).text, "h?lo");
        for _ in 0..10 {
            key(&mut s, Key::Right);
        }
        assert_eq!(editor(&s).caret, 4);
        for _ in 0..10 {
            key(&mut s, Key::Left);
        }
        key(&mut s, Key::Backspace);
        assert_eq!(editor(&s).text, "h?lo");
        for k in [Key::Up, Key::Down, Key::Home, Key::End] {
            key(&mut s, k);
        }
        assert_eq!(editor(&s).text, "h?lo");
    }

    #[test]
    fn f_comment_02_ctrl_and_alt_ignored() {
        let mut s = st();
        on_cursor_key(&mut s, KeyEvent::ch('a'), &mut Vec::new());
        on_key(&mut s, KeyEvent::ctrl('x'), &mut Vec::new());
        assert_eq!(editor(&s).text, "");
    }

    #[test]
    fn f_comment_02_paste_collapses_newline_runs() {
        let mut s = st();
        on_cursor_key(&mut s, KeyEvent::ch('a'), &mut Vec::new());
        assert!(on_paste(&mut s, "a\r\n\nb\nc"));
        assert_eq!(editor(&s).text, "a b c");
        assert_eq!(editor(&s).caret, 5);
        let mut none = st();
        assert!(!on_paste(&mut none, "x"));
    }

    #[test]
    fn f_comment_02_esc_discards() {
        let mut s = st();
        on_cursor_key(&mut s, KeyEvent::ch('a'), &mut Vec::new());
        type_str(&mut s, "x");
        key(&mut s, Key::Esc);
        assert_eq!(s.overlay, Overlay::None);
        assert!(s.comments.is_empty());
    }

    #[test]
    fn f_comment_02_tab_toggles_mode_only_with_mcp() {
        let mut s = st();
        on_cursor_key(&mut s, KeyEvent::ch('a'), &mut Vec::new());
        key(&mut s, Key::Tab);
        assert_eq!(s.note.as_deref(), Some("MCP is off (M to start)"));
        assert!(!editor(&s).ask_mode);
        key(&mut s, Key::Esc);
        s.mcp.running = true;
        on_cursor_key(&mut s, KeyEvent::ch('a'), &mut Vec::new());
        assert!(editor(&s).ask_mode, "default ask while MCP runs");
        key(&mut s, Key::Tab);
        assert!(!editor(&s).ask_mode);
        key(&mut s, Key::Esc);
        on_cursor_key(&mut s, KeyEvent::ch('a'), &mut Vec::new());
        assert!(!editor(&s).ask_mode, "chosen mode kept");
        key(&mut s, Key::Tab);
        assert!(editor(&s).ask_mode);
        s.mcp.running = false;
        assert!(!effective_ask(&s, editor(&s)));
        key(&mut s, Key::Esc);
        on_cursor_key(&mut s, KeyEvent::ch('a'), &mut Vec::new());
        assert!(!editor(&s).ask_mode);
        assert_eq!(s.thread.chosen, None, "chosen reset while MCP off");
    }

    #[test]
    fn f_comment_03_enter_empty_does_nothing_and_save_inserts() {
        let mut s = st();
        s.nav.row = 1;
        on_cursor_key(&mut s, KeyEvent::ch('a'), &mut Vec::new());
        type_str(&mut s, "  ");
        key(&mut s, Key::Enter);
        assert!(matches!(s.overlay, Overlay::Editor(_)));
        type_str(&mut s, " why? ");
        key(&mut s, Key::Enter);
        assert_eq!(s.overlay, Overlay::None);
        assert_eq!(s.comments.len(), 1);
        let c = &s.comments[0];
        assert_eq!((c.id.as_str(), c.message.as_str(), c.row, c.line), ("q1", "why?", 1, Some(2)));
        assert_eq!(c.turns.len(), 1);
        assert_eq!(s.note.as_deref(), Some("question saved (1)"));
    }

    #[test]
    fn f_comment_07_edit_replaces_message() {
        let mut s = st();
        s.comments.push(comment("q1", 1, 2, "old"));
        open_edit(&mut s, "q1");
        assert_eq!(editor(&s).text, "old");
        assert_eq!(editor(&s).caret, 3);
        type_str(&mut s, "er");
        key(&mut s, Key::Enter);
        assert_eq!(s.comments[0].message, "older");
        assert_eq!(s.comments[0].turns[0].message, "older");
        assert_eq!(s.note.as_deref(), Some("comment updated"));
        assert_eq!(s.overlay, Overlay::None);
    }

    #[test]
    fn f_comment_07_edit_refused_after_follow_ups() {
        let mut s = st();
        let mut c = comment("q1", 1, 2, "m");
        c.turns.push(Turn { message: "f".into(), answer: None, prior: Vec::new() });
        s.comments.push(c);
        open_edit(&mut s, "q1");
        assert_eq!(s.overlay, Overlay::None);
        assert_eq!(s.note.as_deref(), Some("can't edit after follow-ups"));
    }

    #[test]
    fn f_ask_03_follow_up_needs_mcp_and_done_thread() {
        use crate::comments::AnswerStatus;
        use crate::comments::testutil::answer;
        let mut s = st();
        let mut c = comment("q1", 1, 2, "m");
        c.turns[0].answer = Some(answer(AnswerStatus::Done, "a"));
        s.comments.push(c);
        open_follow_up(&mut s, "q1");
        type_str(&mut s, "more");
        key(&mut s, Key::Tab);
        assert!(matches!(s.overlay, Overlay::Editor(_)));
        key(&mut s, Key::Enter);
        assert_eq!(s.note.as_deref(), Some("MCP is off (M to start)"));
        assert!(matches!(s.overlay, Overlay::Editor(_)));
        assert_eq!(s.comments[0].turns.len(), 1);
    }

    #[test]
    fn f_ask_03_follow_up_refused_while_live() {
        use crate::comments::AnswerStatus;
        use crate::comments::testutil::answer;
        let mut s = st();
        s.mcp.running = true;
        let mut c = comment("q1", 1, 2, "m");
        c.turns[0].answer = Some(answer(AnswerStatus::Streaming, ""));
        s.comments.push(c);
        open_follow_up(&mut s, "q1");
        type_str(&mut s, "more");
        key(&mut s, Key::Enter);
        assert_eq!(s.note.as_deref(), Some("can't follow up yet"));
        assert!(matches!(s.overlay, Overlay::Editor(_)));
    }
}
