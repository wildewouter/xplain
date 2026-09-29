//! Comment focus and thread interaction: J/K focus, numbered jump, edit/delete/ask keys, thread scroll,
//! code-block copy buttons, delete confirmation.
//!
//! Spec: F-COMMENT-06 (focus `J` `K`), F-COMMENT-07 (edit), F-COMMENT-08 (delete `D`, dialog keys),
//! F-COMMENT-09 (`)` `(` numbered jump, `cannot read` note), F-ASK-07 (thread scroll), F-ASK-08 (code copy),
//! F-ASK-02/03/04 (a/A keys route to `ask`). Oracle: focus/scrolls/btn/numJump code in `src/app.tsx`.
//! Owner: component `comments` (D). Must not render.
//!
//! Runtime hook: `update` calls [`sync`] after every event (after `rows::ensure`). It finishes a numbered jump
//! that had to wait for a file read, and keeps a focused thread's window at the bottom when an answer
//! arrives while it showed the last line (F-ASK-07).

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::ask;
use crate::browse;
use crate::comments::{self, Comment};
use crate::effect::{Effect, Fx};
use crate::jump;
use crate::keys::{Key, KeyEvent};
use crate::nav;
use crate::rows;
use crate::state::{Overlay, Pending, State};
use crate::thread_layout::{self, ThreadInfo};

/// Per-thread scroll (F-ASK-07). A thread without an entry starts at the top with follow on.
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
    /// Box heads of human comments created at the cursor (`selection L3-5`, `line L2`), by id.
    pub heads: HashMap<String, String>,
    /// Editor mode chosen with Tab (`true` = ask); only counts while MCP runs (F-COMMENT-02).
    pub chosen: Option<bool>,
    /// Numbered jump waiting for its file (comment id).
    pub num_go: Option<String>,
    /// Last numbered-jump target.
    pub num_last: Option<String>,
    /// Per comment: hash of its turns/answers and whether the focused window showed the last body line.
    pub seen: HashMap<String, (u64, bool)>,
}

/// MCP stopped: the chosen editor mode is forgotten (F-COMMENT-02). Called by the MCP stop flow.
pub fn on_mcp_stopped(state: &mut State) {
    state.thread.chosen = None;
}

/// `J` `K` `)` `(` in cursor/browse/visual context (focus and numbered jumps). True when consumed.
pub fn on_cursor_key(state: &mut State, key: KeyEvent, fx: &mut Fx) -> bool {
    if key.mods.ctrl {
        return false;
    }
    match key.key {
        Key::Char(c @ ('J' | 'K')) => {
            state.nav.count = 0;
            let list = comments::ids_in_file(state);
            if list.is_empty() {
                state.set_note("no comments");
                return true;
            }
            let fo = state.nav.focused_comment.as_ref().and_then(|f| list.iter().position(|x| x == f));
            let n = list.len();
            let at = match fo {
                None => {
                    if c == 'J' {
                        0
                    } else {
                        n - 1
                    }
                }
                Some(i) => {
                    if c == 'J' {
                        (i + 1).min(n - 1)
                    } else {
                        i.saturating_sub(1)
                    }
                }
            };
            focus(state, &list[at]);
            true
        }
        Key::Char(c @ (')' | '(')) => {
            state.nav.count = 0;
            num_jump(state, if c == ')' { 1 } else { -1 }, fx);
            true
        }
        _ => false,
    }
}

/// Numbered comments across files, by number; wraps at the ends (F-COMMENT-09).
fn num_jump(state: &mut State, dir: i32, fx: &mut Fx) {
    let mut list: Vec<&Comment> = state.comments.iter().filter(|c| c.number.is_some()).collect();
    if list.is_empty() {
        state.set_note("no numbered comments");
        return;
    }
    list.sort_by_key(|c| (c.number, c.seq));
    let n = list.len();
    let start = [state.nav.focused_comment.as_ref(), state.thread.num_last.as_ref()]
        .into_iter()
        .flatten()
        .find_map(|id| list.iter().position(|c| &c.id == id));
    let at = match start {
        None => {
            if dir > 0 {
                0
            } else {
                n - 1
            }
        }
        Some(i) => {
            if dir > 0 {
                (i + 1) % n
            } else {
                (i + n - 1) % n
            }
        }
    };
    let (id, file) = (list[at].id.clone(), list[at].file.clone());
    state.thread.num_last = Some(id.clone());
    state.thread.num_go = Some(id);
    match state.files.iter().position(|f| f.path == file) {
        Some(fi) => {
            if state.browse.is_some() {
                browse::close(state);
            }
            if fi != state.nav.file_index {
                jump::open_file(state, fi);
            }
            rows::ensure(state);
        }
        None => {
            if state.browse.as_ref().map(|b| b.path.as_str()) != Some(file.as_str()) {
                browse::open(state, &file, fx);
            }
        }
    }
    finish_jump(state);
}

/// Complete a pending numbered jump once its file is shown; drop it when the file cannot be opened.
fn finish_jump(state: &mut State) {
    let Some(id) = state.thread.num_go.clone() else { return };
    let (present, same_file, visible) = match comments::find(state, &id) {
        Some(c) => (true, c.file == comments::current_path(state), comments::row_of(state, c).is_some()),
        None => (false, false, false),
    };
    if !present {
        state.thread.num_go = None;
        return;
    }
    if !same_file {
        let reading = state.loader.pending.values().any(|p| matches!(p, Pending::Browse { .. }));
        if !reading {
            state.thread.num_go = None; // open failed (note set by browse)
        }
        return;
    }
    state.thread.num_go = None;
    if visible {
        focus(state, &id);
    } else {
        state.set_note("comment not in view");
    }
}

/// Key while a comment is focused (`Nav::focused_comment`): e Enter D a A d u j k g G up/down esc, motions
/// unfocus (F-COMMENT-06..08, F-ASK-07/08). True when consumed; otherwise the caller falls through to nav
/// (motion keys unfocus first).
pub fn on_focused_key(state: &mut State, key: KeyEvent, fx: &mut Fx) -> bool {
    let Some(id) = state.nav.focused_comment.clone() else { return false };
    if key.mods.ctrl {
        return false;
    }
    let Some(c) = comments::find(state, &id) else { return false };
    if comments::row_of(state, c).is_none() {
        return false;
    }
    let info = thread_layout::thread_info(state, c);
    let picked =
        state.thread.picked_block.as_ref().filter(|(p, i)| *p == id && *i < info.btns.len()).map(|(_, i)| *i);
    let k = key.key;
    if k == Key::Esc {
        state.nav.count = 0;
        if picked.is_some() {
            state.thread.picked_block = None;
        } else {
            unfocus(state);
        }
        return true;
    }
    if matches!(k, Key::Char('J' | 'K' | ')' | '(')) {
        return false;
    }
    if k == Key::Enter {
        if let Some(i) = picked {
            copy_block(state, &info, i, fx);
            return true;
        }
    }
    if !info.btns.is_empty() && matches!(k, Key::Up | Key::Down) {
        state.nav.count = 0;
        pick_block(state, &id, &info, picked, if k == Key::Down { 1 } else { -1 });
        return true;
    }
    if matches!(k, Key::Enter | Key::Char('e')) {
        state.nav.count = 0;
        crate::editor::open_edit(state, &id);
        return true;
    }
    match k {
        Key::Char('D') => {
            state.nav.count = 0;
            state.overlay = Overlay::DeleteComment { id };
            return true;
        }
        Key::Char('a') => {
            state.nav.count = 0;
            ask::ask_focused(state, &id, fx);
            return true;
        }
        Key::Char('A') => {
            state.nav.count = 0;
            ask::ask_all(state, fx);
            return true;
        }
        Key::Char(ch @ ('j' | 'k' | 'd' | 'u' | 'g' | 'G')) if info.max_off > 0 => {
            state.nav.count = 0;
            scroll(state, &id, &info, ch);
            return true;
        }
        _ => {}
    }
    let motion = match k {
        Key::Left | Key::Right | Key::Up | Key::Down | Key::PageUp | Key::PageDown => true,
        Key::Char(ch) => "hjklwbevVdugG0$^[] p".contains(ch),
        _ => false,
    };
    if motion {
        unfocus(state);
    }
    false
}

/// Thread window keys (F-ASK-07): `j`/`k` +-1, `d`/`u` half window, `g` top, `G` bottom.
fn scroll(state: &mut State, id: &str, info: &ThreadInfo, ch: char) {
    let half = (info.v / 2).max(1);
    let want = match ch {
        'j' => info.off + 1,
        'k' => info.off.saturating_sub(1),
        'd' => info.off + half,
        'u' => info.off.saturating_sub(half),
        'g' => 0,
        _ => info.max_off,
    };
    let off = want.min(info.max_off);
    let prev = state.thread.scrolls.get(id).is_none_or(|s| s.follow);
    let follow = if ch == 'G' {
        true
    } else if off < info.off {
        false
    } else {
        prev
    };
    state.thread.scrolls.insert(id.to_string(), ThreadScroll { off, follow });
}

/// Up/Down: select the next/previous copy button (wrapping), scrolling it into view (F-ASK-08).
fn pick_block(state: &mut State, id: &str, info: &ThreadInfo, picked: Option<usize>, dir: i32) {
    let n = info.btns.len();
    let i = match picked {
        None => {
            if dir > 0 {
                0
            } else {
                n - 1
            }
        }
        Some(p) => {
            if dir > 0 {
                (p + 1) % n
            } else {
                (p + n - 1) % n
            }
        }
    };
    state.thread.picked_block = Some((id.to_string(), i));
    let at = info.btns[i].0;
    if at >= info.off && at < info.off + info.v {
        return;
    }
    let off = at.min(info.max_off);
    let prev = state.thread.scrolls.get(id).is_none_or(|s| s.follow);
    let follow = if off < info.max_off { false } else { prev };
    state.thread.scrolls.insert(id.to_string(), ThreadScroll { off, follow });
}

/// Enter on a picked button: OSC 52 copy and `copied <k> line(s)` (F-ASK-08).
fn copy_block(state: &mut State, info: &ThreadInfo, i: usize, fx: &mut Fx) {
    let code = info.btns[i].1.clone();
    let n = code.split('\n').count();
    fx.push(Effect::Clipboard(code));
    state.set_note(format!("copied {n} line{}", if n == 1 { "" } else { "s" }));
}

/// Key while `Overlay::DeleteComment` (y/Enter delete, n/Esc cancel; `q` quits).
pub fn on_delete_dialog_key(state: &mut State, key: KeyEvent, _fx: &mut Fx) {
    if key.mods.ctrl {
        return;
    }
    let Overlay::DeleteComment { id } = &state.overlay else { return };
    let id = id.clone();
    match key.key {
        Key::Char('y') | Key::Enter => {
            let list = comments::ids_in_file(state);
            let next = list.iter().position(|x| *x == id).and_then(|i| list.get(i + 1)).cloned();
            comments::remove(state, &id);
            state.overlay = Overlay::None;
            state.set_note("comment deleted");
            match next {
                Some(n) => focus(state, &n),
                None => unfocus(state),
            }
            nav::viewport::clamp_top(state);
        }
        Key::Char('n') | Key::Esc => state.overlay = Overlay::None,
        _ => {}
    }
}

/// Focus comment `id`: cursor to its row, selection ends (F-COMMENT-06). A changed focus drops the picked block.
pub fn focus(state: &mut State, id: &str) {
    if state.nav.focused_comment.as_deref() != Some(id) {
        state.thread.picked_block = None;
    }
    state.nav.focused_comment = Some(id.to_string());
    if let Some(row) = comments::find(state, id).and_then(|c| comments::row_of(state, c)) {
        nav::place(state, row, None);
        nav::visual::end(state);
    }
}

pub fn unfocus(state: &mut State) {
    state.nav.focused_comment = None;
    state.thread.picked_block = None;
}

pub(crate) fn turns_sig(c: &Comment) -> u64 {
    let mut h = DefaultHasher::new();
    for t in &c.turns {
        t.message.hash(&mut h);
        t.prior.len().hash(&mut h);
        if let Some(a) = &t.answer {
            (a.status as u8).hash(&mut h);
            a.text.hash(&mut h);
        }
        1u8.hash(&mut h);
    }
    h.finish()
}

/// Runtime hook after every event: finish a waiting numbered jump, and when an answer arrived while the
/// focused window showed the last body line, keep it on the new last line (F-ASK-07).
pub fn sync(state: &mut State) {
    if state.thread.num_go.is_some() {
        finish_jump(state);
    }
    let focused = state.nav.focused_comment.clone();
    let sigs: Vec<(String, u64)> = state.comments.iter().map(|c| (c.id.clone(), turns_sig(c))).collect();
    for (id, sig) in sigs {
        let prev = state.thread.seen.get(&id).copied();
        if focused.as_deref() != Some(id.as_str()) {
            state.thread.seen.insert(id, (sig, false));
            continue;
        }
        let arrived = prev.is_some_and(|(p, end)| p != sig && end);
        let Some(c) = comments::find(state, &id) else { continue };
        if comments::row_of(state, c).is_none() {
            continue;
        }
        if arrived {
            let info = thread_layout::thread_info(state, c);
            let follow = state.thread.scrolls.get(&id).is_none_or(|s| s.follow);
            let live = comments::latest_answer(c).is_some_and(comments::is_live);
            if !(follow && live) {
                state.thread.scrolls.insert(id.clone(), ThreadScroll { off: info.max_off, follow });
            }
        }
        let Some(c) = comments::find(state, &id) else { continue };
        let info = thread_layout::thread_info(state, c);
        let end = info.off + info.v >= info.total;
        state.thread.seen.insert(id, (sig, end));
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::comments::testutil::{answer, comment, ctx, state_with};
    use crate::comments::{AnswerStatus, Origin};

    fn k(state: &mut State, key: KeyEvent) -> bool {
        on_focused_key(state, key, &mut Vec::new())
    }

    fn three() -> State {
        let mut s = state_with(vec![ctx(1), ctx(2), ctx(3), ctx(4)]);
        s.comments.push(comment("q1", 1, 3, "third"));
        s.comments.push(comment("q2", 2, 1, "first"));
        s.comments.push(comment("q3", 3, 1, "first b"));
        s
    }

    #[test]
    fn f_comment_06_j_k_order_clamp_and_no_comments() {
        let mut s = state_with(vec![ctx(1)]);
        assert!(on_cursor_key(&mut s, KeyEvent::ch('J'), &mut Vec::new()));
        assert_eq!(s.note.as_deref(), Some("no comments"));
        let mut s = three();
        let j = |s: &mut State| on_cursor_key(s, KeyEvent::ch('J'), &mut Vec::new());
        j(&mut s);
        assert_eq!(s.nav.focused_comment.as_deref(), Some("q2"));
        j(&mut s);
        assert_eq!(s.nav.focused_comment.as_deref(), Some("q3"));
        j(&mut s);
        assert_eq!(s.nav.focused_comment.as_deref(), Some("q1"));
        j(&mut s);
        assert_eq!(s.nav.focused_comment.as_deref(), Some("q1"), "clamped, no wrap");
        assert_eq!(s.nav.row, 2);
        on_cursor_key(&mut s, KeyEvent::ch('K'), &mut Vec::new());
        assert_eq!(s.nav.focused_comment.as_deref(), Some("q3"));
        let mut s = three();
        on_cursor_key(&mut s, KeyEvent::ch('K'), &mut Vec::new());
        assert_eq!(s.nav.focused_comment.as_deref(), Some("q1"), "unfocused K: last");
    }

    #[test]
    fn f_comment_06_motion_unfocuses_and_falls_through() {
        let mut s = three();
        focus(&mut s, "q2");
        assert!(!k(&mut s, KeyEvent::ch('l')));
        assert_eq!(s.nav.focused_comment, None);
        focus(&mut s, "q2");
        assert!(!k(&mut s, KeyEvent::plain(Key::Down)));
        assert_eq!(s.nav.focused_comment, None);
        focus(&mut s, "q2");
        assert!(!k(&mut s, KeyEvent::ch('5')), "digits keep focus");
        assert_eq!(s.nav.focused_comment.as_deref(), Some("q2"));
        assert!(!k(&mut s, KeyEvent::ch('J')));
        assert!(s.nav.focused_comment.is_some());
        assert!(k(&mut s, KeyEvent::plain(Key::Esc)));
        assert_eq!(s.nav.focused_comment, None);
    }

    #[test]
    fn f_comment_07_e_and_enter_edit_refused_with_follow_ups() {
        let mut s = three();
        focus(&mut s, "q2");
        assert!(k(&mut s, KeyEvent::ch('e')));
        assert!(matches!(s.overlay, Overlay::Editor(_)));
        s.overlay = Overlay::None;
        assert!(k(&mut s, KeyEvent::plain(Key::Enter)));
        assert!(matches!(s.overlay, Overlay::Editor(_)));
        s.overlay = Overlay::None;
        s.comments[1].turns.push(comments::Turn { message: "f".into(), answer: None, prior: Vec::new() });
        assert!(k(&mut s, KeyEvent::ch('e')));
        assert_eq!(s.note.as_deref(), Some("can't edit after follow-ups"));
        assert_eq!(s.overlay, Overlay::None);
    }

    #[test]
    fn f_comment_08_delete_dialog_flow() {
        let mut s = three();
        focus(&mut s, "q2");
        assert!(k(&mut s, KeyEvent::ch('D')));
        assert_eq!(s.overlay, Overlay::DeleteComment { id: "q2".into() });
        let mut fx = Vec::new();
        on_delete_dialog_key(&mut s, KeyEvent::ch('x'), &mut fx);
        assert!(matches!(s.overlay, Overlay::DeleteComment { .. }));
        on_delete_dialog_key(&mut s, KeyEvent::ch('n'), &mut fx);
        assert_eq!(s.overlay, Overlay::None);
        s.overlay = Overlay::DeleteComment { id: "q2".into() };
        on_delete_dialog_key(&mut s, KeyEvent::plain(Key::Esc), &mut fx);
        assert_eq!(s.comments.len(), 3);
        s.overlay = Overlay::DeleteComment { id: "q2".into() };
        on_delete_dialog_key(&mut s, KeyEvent::ch('y'), &mut fx);
        assert_eq!(s.comments.len(), 2);
        assert_eq!(s.note.as_deref(), Some("comment deleted"));
        assert_eq!(s.nav.focused_comment.as_deref(), Some("q3"), "focus next comment");
        s.overlay = Overlay::DeleteComment { id: "q1".into() };
        focus(&mut s, "q1");
        on_delete_dialog_key(&mut s, KeyEvent::plain(Key::Enter), &mut fx);
        assert_eq!(s.nav.focused_comment, None, "last deleted: unfocus");
    }

    #[test]
    fn f_comment_09_numbered_jump_wraps_and_notes() {
        let mut s = state_with(vec![ctx(1), ctx(2)]);
        let mut fx = Vec::new();
        on_cursor_key(&mut s, KeyEvent::ch(')'), &mut fx);
        assert_eq!(s.note.as_deref(), Some("no numbered comments"));
        for (id, n, line) in [("q1", 2, 2), ("q2", 1, 1)] {
            let mut c = comment(id, u64::from(n), line, "note");
            c.origin = Origin::Agent;
            c.number = Some(n);
            s.comments.push(c);
        }
        on_cursor_key(&mut s, KeyEvent::ch(')'), &mut fx);
        assert_eq!(s.nav.focused_comment.as_deref(), Some("q2"), "first by number");
        on_cursor_key(&mut s, KeyEvent::ch(')'), &mut fx);
        assert_eq!(s.nav.focused_comment.as_deref(), Some("q1"));
        on_cursor_key(&mut s, KeyEvent::ch(')'), &mut fx);
        assert_eq!(s.nav.focused_comment.as_deref(), Some("q2"), "wraps");
        on_cursor_key(&mut s, KeyEvent::ch('('), &mut fx);
        assert_eq!(s.nav.focused_comment.as_deref(), Some("q1"), "previous wraps back");
    }

    fn scrolled() -> State {
        let mut s = state_with(vec![ctx(1), ctx(2)]);
        s.size = crate::screen::Size { cols: 80, rows: 12 };
        let mut c = comment("q1", 1, 1, "m");
        c.turns[0].answer = Some(answer(
            AnswerStatus::Done,
            &(1..=30).map(|i| format!("l{i}")).collect::<Vec<_>>().join("\n"),
        ));
        s.comments.push(c);
        focus(&mut s, "q1");
        s
    }

    #[test]
    fn f_ask_07_scroll_keys_clamp_and_follow_flag() {
        let mut s = scrolled();
        let info = thread_layout::thread_info(&s, &s.comments[0]);
        assert!(info.max_off > 0 && info.off == 0);
        assert!(k(&mut s, KeyEvent::ch('j')));
        assert_eq!(s.thread.scrolls["q1"].off, 1);
        assert!(k(&mut s, KeyEvent::ch('G')));
        let sc = s.thread.scrolls["q1"];
        assert_eq!((sc.off, sc.follow), (info.max_off, true));
        assert!(k(&mut s, KeyEvent::ch('k')));
        assert_eq!(s.thread.scrolls["q1"].off, info.max_off - 1);
        assert!(!s.thread.scrolls["q1"].follow);
        assert!(k(&mut s, KeyEvent::ch('g')));
        assert_eq!(s.thread.scrolls["q1"].off, 0);
        assert!(k(&mut s, KeyEvent::ch('d')));
        assert_eq!(s.thread.scrolls["q1"].off, info.v / 2);
        assert!(k(&mut s, KeyEvent::ch('u')));
        assert_eq!(s.thread.scrolls["q1"].off, 0);
    }

    #[test]
    fn f_ask_07_answer_arrival_keeps_bottom() {
        let mut s = scrolled();
        sync(&mut s);
        k(&mut s, KeyEvent::ch('G'));
        sync(&mut s);
        let before = thread_layout::thread_info(&s, &s.comments[0]);
        assert_eq!(before.off, before.max_off);
        s.comments[0].turns[0].prior.push(answer(AnswerStatus::Done, "extra\nlines\nhere"));
        sync(&mut s);
        let after = thread_layout::thread_info(&s, &s.comments[0]);
        assert!(after.max_off > before.max_off);
        assert_eq!(after.off, after.max_off, "bottom kept on arrival");
    }

    #[test]
    fn f_ask_07_arrival_keeps_offset_when_not_at_bottom() {
        let mut s = scrolled();
        sync(&mut s);
        k(&mut s, KeyEvent::ch('g'));
        sync(&mut s);
        s.comments[0].turns[0].prior.push(answer(AnswerStatus::Done, "extra"));
        sync(&mut s);
        assert_eq!(thread_layout::thread_info(&s, &s.comments[0]).off, 0);
    }

    fn with_code() -> State {
        let mut s = state_with(vec![ctx(1), ctx(2)]);
        let mut c = comment("q1", 1, 1, "see\n```rs\nlet a = 1;\n\tb\n```\nand\n~~~\nx\n~~~");
        c.turns[0].answer = None;
        s.comments.push(c);
        focus(&mut s, "q1");
        s
    }

    #[test]
    fn f_ask_08_pick_and_copy() {
        let mut s = with_code();
        let mut fx = Vec::new();
        assert!(on_focused_key(&mut s, KeyEvent::plain(Key::Down), &mut fx));
        assert_eq!(s.thread.picked_block, Some(("q1".into(), 0)));
        on_focused_key(&mut s, KeyEvent::plain(Key::Down), &mut fx);
        assert_eq!(s.thread.picked_block, Some(("q1".into(), 1)));
        on_focused_key(&mut s, KeyEvent::plain(Key::Down), &mut fx);
        assert_eq!(s.thread.picked_block, Some(("q1".into(), 0)), "wraps");
        on_focused_key(&mut s, KeyEvent::plain(Key::Up), &mut fx);
        assert_eq!(s.thread.picked_block, Some(("q1".into(), 1)));
        on_focused_key(&mut s, KeyEvent::plain(Key::Up), &mut fx);
        on_focused_key(&mut s, KeyEvent::plain(Key::Enter), &mut fx);
        assert_eq!(fx, vec![Effect::Clipboard("let a = 1;\n\tb".to_string())]);
        assert_eq!(s.note.as_deref(), Some("copied 2 lines"));
        on_focused_key(&mut s, KeyEvent::plain(Key::Esc), &mut fx);
        assert_eq!(s.thread.picked_block, None);
        assert_eq!(s.nav.focused_comment.as_deref(), Some("q1"), "esc only unpicks");
        on_focused_key(&mut s, KeyEvent::plain(Key::Up), &mut fx);
        assert_eq!(s.thread.picked_block, Some(("q1".into(), 1)), "first Up: last");
        on_focused_key(&mut s, KeyEvent::plain(Key::Enter), &mut fx);
        assert_eq!(s.note.as_deref(), Some("copied 1 line"));
    }

    #[test]
    fn f_ask_02_a_key_notes() {
        let mut s = state_with(vec![ctx(1)]);
        let mut c = comment("q1", 1, 1, "m");
        c.turns[0].answer = Some(answer(AnswerStatus::Pending, ""));
        s.comments.push(c);
        focus(&mut s, "q1");
        k(&mut s, KeyEvent::ch('a'));
        assert_eq!(s.note.as_deref(), Some("still waiting for the agent"));
        s.comments[0].turns[0].answer = Some(answer(AnswerStatus::Done, "a"));
        k(&mut s, KeyEvent::ch('a'));
        assert!(
            matches!(&s.overlay, Overlay::Editor(e) if matches!(e.kind, crate::state::EditorKind::FollowUp { .. }))
        );
        s.overlay = Overlay::None;
        s.comments[0].turns[0].answer = Some(answer(AnswerStatus::Cancelled, "MCP stopped"));
        k(&mut s, KeyEvent::ch('a'));
        assert_eq!(s.note.as_deref(), Some("MCP is off (M to start)"));
        s.comments[0].turns.push(comments::Turn { message: "f".into(), answer: None, prior: vec![] });
        k(&mut s, KeyEvent::ch('a'));
        assert_eq!(s.note.as_deref(), Some("can't retry a follow-up yet"));
        s.comments[0].origin = Origin::Agent;
        s.comments[0].turns.truncate(1);
        s.comments[0].turns[0].answer = Some(answer(AnswerStatus::Cancelled, "x"));
        s.comments[0].turns.push(comments::Turn { message: "f".into(), answer: None, prior: vec![] });
        k(&mut s, KeyEvent::ch('a'));
        assert_eq!(s.note.as_deref(), Some("can't reply to this note yet"));
    }
}
