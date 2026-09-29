//! Thread body window and box heights shared with `thread.rs` and `nav::viewport` (F-ASK-07, F-NAV-09).

use super::body::thread_body;
use super::text::BodyKind;
use super::{ASK_H, BODY_CAP, SENT_MAX, box_width};
use crate::comments::{self, Comment};
use crate::state::{EditorKind, Overlay, State};

/// Visible part of a thread body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BodyWin {
    pub start: usize,
    pub end: usize,
    /// Hidden lines behind `… +N more` (unfocused only).
    pub more: usize,
    pub total: usize,
    /// 1-based first/last shown line.
    pub from: usize,
    pub to: usize,
    pub max_off: usize,
    pub off: usize,
}

/// Unfocused: first `BODY_CAP` lines and a `more` count; focused: window of `v` lines at `off` (clamped).
pub fn window_body(total: usize, focused: bool, v: usize, off: usize) -> BodyWin {
    if !focused {
        let n = total.min(BODY_CAP);
        return BodyWin { start: 0, end: n, more: total - n, total, from: 1, to: n, max_off: 0, off: 0 };
    }
    let vv = v.max(1);
    let max_off = total.saturating_sub(vv);
    let o = off.min(max_off);
    let end = total.min(o + vv);
    BodyWin { start: o, end, more: 0, total, from: o + 1, to: end, max_off, off: o }
}

pub(super) fn quoted_rows(n: usize) -> usize {
    if n == 0 { 0 } else { n.min(SENT_MAX) + usize::from(n > SENT_MAX) }
}

/// Rows of a comment box without its body lines (`sentBase`): borders + head + quoted (+ hint when focused).
pub(super) fn sent_base(quoted: usize, focused: bool) -> usize {
    3 + quoted_rows(quoted) + usize::from(focused)
}

pub(super) fn unfocused_height(c: &Comment, width: usize) -> usize {
    let body = thread_body(c, width.saturating_sub(3).max(1));
    let w = window_body(body.len(), false, 0, 0);
    sent_base(comments::quoted_lines(c).len(), false) + (w.end - w.start) + usize::from(w.more > 0)
}

/// Rows of the focused comment box: same as `comment_box(.., true).lines.len()` without building it.
pub(super) fn focused_height(state: &State, c: &Comment, width: usize) -> usize {
    let total = thread_body(c, width.saturating_sub(3).max(1)).len();
    let shown = window_body(total, true, focus_room(state, c, width), 0);
    sent_base(comments::quoted_lines(c).len(), true) + (shown.end - shown.start)
}

pub(super) fn follow_up_open(state: &State) -> bool {
    matches!(&state.overlay, Overlay::Editor(e) if matches!(e.kind, EditorKind::FollowUp { .. }))
}

/// Body lines the focused box may show: what the viewport has left on its row.
pub(super) fn focus_room(state: &State, c: &Comment, width: usize) -> usize {
    let h = crate::nav::body_height(state.size) as isize;
    let others: isize = comments::row_of(state, c)
        .map(|r| {
            comments::anchored_at(state, r)
                .iter()
                .filter(|x| x.id != c.id)
                .map(|x| unfocused_height(x, width) as isize)
                .sum()
        })
        .unwrap_or(0);
    let base = sent_base(comments::quoted_lines(c).len(), true) as isize;
    let fu = if follow_up_open(state) { ASK_H as isize } else { 0 };
    (h - 1 - base - others - fu).max(1) as usize
}

/// Window offset used for display: following a live thread pins the bottom.
pub(super) fn display_off(state: &State, c: &Comment) -> usize {
    let sc = state.thread.scrolls.get(&c.id);
    let live = comments::latest_answer(c).is_some_and(comments::is_live);
    let follow = sc.is_none_or(|s| s.follow);
    if live && follow { usize::MAX } else { sc.map_or(0, |s| s.off) }
}

/// Geometry of the focused thread window (`info` in app.tsx).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadInfo {
    /// Window height.
    pub v: usize,
    pub off: usize,
    pub max_off: usize,
    pub total: usize,
    /// Copy buttons: `(body line index, raw code)`.
    pub btns: Vec<(usize, String)>,
}

/// Window geometry of `c` as if focused, at the current scroll state and size.
pub fn thread_info(state: &State, c: &Comment) -> ThreadInfo {
    let width = box_width(state);
    let body = thread_body(c, width.saturating_sub(3).max(1));
    let v = focus_room(state, c, width);
    let w = window_body(body.len(), true, v, display_off(state, c));
    let btns = body
        .iter()
        .enumerate()
        .filter(|(_, l)| l.kind == BodyKind::Btn)
        .map(|(i, l)| (i, l.code.clone().unwrap_or_default()))
        .collect();
    ThreadInfo { v, off: w.off, max_off: w.max_off, total: w.total, btns }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::comments::AnswerStatus;
    use crate::comments::testutil::{answer, comment};
    use crate::state::EditorState;
    use crate::thread_layout::fixtures::*;
    use crate::thread_layout::*;

    #[test]
    fn f_ask_07_window_body_unfocused_cap_and_focused_clamp() {
        let w = window_body(20, false, 0, 0);
        assert_eq!((w.end, w.more, w.from, w.to), (BODY_CAP, 6, 1, 14));
        let w = window_body(5, false, 0, 0);
        assert_eq!((w.end, w.more), (5, 0));
        let w = window_body(30, true, 10, 5);
        assert_eq!((w.start, w.end, w.max_off, w.from, w.to), (5, 15, 20, 6, 15));
        let w = window_body(30, true, 10, usize::MAX);
        assert_eq!((w.off, w.to), (20, 30));
        let w = window_body(3, true, 10, 4);
        assert_eq!((w.off, w.max_off, w.end), (0, 0, 3));
        assert_eq!(window_body(3, true, 0, 0).end, 1, "window at least 1");
    }

    #[test]
    fn f_nav_09_focused_height_matches_box_lines() {
        let mut s = st();
        s.comments.push(comment("q1", 1, 2, "a"));
        s.nav.focused_comment = Some("q1".into());
        let n = boxes_at(&s, 1)[0].lines.len();
        assert_eq!(row_extra_height(&s, 1), n);
        assert_eq!(n, 5, "borders + head + body + hint");
    }

    #[test]
    fn f_ask_07_window_room_shrinks_by_other_boxes_and_follow_up_editor() {
        let mut s = st();
        s.size = crate::screen::Size { cols: 80, rows: 24 };
        let big = (1..=40).map(|i| format!("m{i}")).collect::<Vec<_>>().join("\n");
        s.comments.push(comment("q1", 1, 2, &big));
        s.comments.push(comment("q2", 2, 2, "small"));
        let c = s.comments[0].clone();
        let h = crate::nav::body_height(s.size);
        let base = thread_info(&s, &c).v;
        assert_eq!(base, h - 1 - 4 - 4, "other box height 4, own base 4");
        s.overlay = Overlay::Editor(EditorState {
            kind: EditorKind::FollowUp { id: "q1".into() },
            text: String::new(),
            caret: 0,
            ask_mode: false,
        });
        assert_eq!(thread_info(&s, &c).v, base - ASK_H);
    }

    #[test]
    fn f_ask_07_live_answer_follows_bottom_unless_scrolled_up() {
        let mut s = st();
        s.size = crate::screen::Size { cols: 80, rows: 12 };
        let mut c = comment("q1", 1, 2, &(1..=40).map(|i| format!("m{i}")).collect::<Vec<_>>().join("\n"));
        c.turns[0].answer = Some(answer(AnswerStatus::Streaming, ""));
        s.comments.push(c.clone());
        let i = thread_info(&s, &c);
        assert_eq!(i.off, i.max_off, "follow initially on");
        s.thread.scrolls.insert("q1".into(), crate::thread::ThreadScroll { off: 2, follow: false });
        assert_eq!(thread_info(&s, &c).off, 2);
    }
}
