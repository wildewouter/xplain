//! Comment / thread / answer data model shared by UI, ask flow, export and MCP hub.
//!
//! Spec: F-COMMENT-03 (record), F-ASK-01/05/09 (turns, answers), F-COMMENT-10 (agent notes), F-EXPORT-02.
//! Owner: core lead (comments component). Types frozen at skeleton so mcp/export/view workers agree.
//! Must not: reference rendering or MCP transport types.
//! Component `comments` (D) also owns the functions below (creation, anchoring, lookup, deletion).

use crate::diff::LineKind;
use crate::nav::{self, visual};
use crate::rows::{self, ShownRow};
use crate::state::State;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaneSide {
    Old,
    New,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnswerStatus {
    Pending,
    Streaming,
    Done,
    /// No black-box trigger (UNSPEC-40); may be dropped.
    Error,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    pub status: AnswerStatus,
    pub text: String,
    /// MCP client name from `initialize`, if known (F-ASK-05).
    pub agent: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Turn {
    pub message: String,
    pub answer: Option<Answer>,
    /// Earlier answers on this turn, oldest first (second answer on a done turn, F-ASK-09).
    pub prior: Vec<Answer>,
}

/// Selection recorded with a comment (1-based lines and inclusive cols, F-COMMENT-03).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionInfo {
    pub start_line: Option<u32>,
    pub end_line: Option<u32>,
    pub start_col: u32,
    pub end_col: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    Human,
    /// Added by the agent via `annotate`.
    Agent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment {
    /// `q1`, `q2`, ... creation order; agent notes take ids too (F-ASK-01).
    pub id: String,
    pub file: String,
    /// Anchor row index (used when no line number).
    pub row: usize,
    pub side: PaneSide,
    pub line: Option<u32>,
    /// Anchored to a deleted line (unified del row); see F-COMMENT-05.
    pub deleted: bool,
    /// Line text or selected text.
    pub text: String,
    pub message: String,
    pub selection: Option<SelectionInfo>,
    /// Code rows around the anchor: +-3 rows / +-15 rows.
    pub context: Vec<String>,
    pub wide: Vec<String>,
    pub turns: Vec<Turn>,
    pub origin: Origin,
    /// Order label for agent notes; `(`/`)` jump (F-COMMENT-09).
    pub number: Option<u32>,
    /// Creation sequence for ordering ties.
    pub seq: u64,
}

/// Rows of context kept around the anchor (F-COMMENT-03).
pub const CTX: usize = 3;
/// Rows of wide context (F-COMMENT-03).
pub const CTX_WIDE: usize = 15;

/// Next free id `q<n>` (uses `state.thread.next_id`); also used by agent notes (F-ASK-01).
pub fn alloc_id(state: &mut State) -> String {
    state.thread.next_id += 1;
    format!("q{}", state.thread.next_id)
}

/// Next creation sequence number (ordering of comments, boxes under one row).
pub fn alloc_seq(state: &mut State) -> u64 {
    state.thread.next_seq += 1;
    state.thread.next_seq
}

/// Path of the shown file: browse path, else the diff file, else empty (no-changes screen, UNSPEC-13).
pub fn current_path(state: &State) -> String {
    match &state.browse {
        Some(b) => b.path.clone(),
        None => state.files.get(state.nav.file_index).map(|f| f.path.clone()).unwrap_or_default(),
    }
}

/// Deleted-line row: unified del row, or split pair without a right side (F-COMMENT-05).
fn is_del(row: &ShownRow) -> bool {
    match row {
        ShownRow::Line(l) => l.kind == LineKind::Del,
        ShownRow::Pair { r, .. } => r.is_none(),
        _ => false,
    }
}

/// Raw (tab-unexpanded) text of a row for the cursor pane; hunk/note rows: their own text.
fn raw_text(row: &ShownRow, side: PaneSide) -> String {
    match row {
        ShownRow::Hunk(t) | ShownRow::Note(t) => t.clone(),
        ShownRow::Line(l) => l.text.clone(),
        ShownRow::Pair { l, r } => {
            let cell = if rows::pane_of(row, side) == PaneSide::Old { l } else { r };
            cell.as_ref().map(|c| c.text.clone()).unwrap_or_default()
        }
    }
}

/// Does `row` carry the anchor (line number, deleted flag, pane side)? F-COMMENT-05.
fn row_matches(row: &ShownRow, no: u32, del: bool, side: PaneSide) -> bool {
    let n = Some(no);
    match row {
        ShownRow::Line(l) => {
            if side == PaneSide::Old {
                l.kind != LineKind::Add && l.old_no == n
            } else if del {
                l.kind == LineKind::Del && l.old_no == n
            } else {
                l.new_no == n
            }
        }
        ShownRow::Pair { l, r } => {
            if side == PaneSide::Old {
                l.as_ref().is_some_and(|x| x.old_no == n)
            } else if del {
                r.is_none() && l.as_ref().is_some_and(|x| x.old_no == n)
            } else {
                r.as_ref().is_some_and(|x| x.new_no == n)
            }
        }
        _ => false,
    }
}

/// Whether comment `c` is shown on row `row` of the current rows (F-COMMENT-05).
fn anchored_here(state: &State, c: &Comment, row: usize) -> bool {
    let Some(r) = state.rows.rows.get(row) else { return false };
    match c.line {
        Some(no) => row_matches(r, no, c.deleted, c.side),
        None => c.row == row && (c.side == PaneSide::Old || rows::row_no(r, c.side).is_none()),
    }
}

/// Row index of the comment in the current view; `None` when it is in another file or its line is absent.
pub fn row_of(state: &State, c: &Comment) -> Option<usize> {
    if c.file != current_path(state) {
        return None;
    }
    match c.line {
        None => (c.row < state.rows.rows.len() && anchored_here(state, c, c.row)).then_some(c.row),
        Some(no) => state.rows.rows.iter().position(|r| row_matches(r, no, c.deleted, c.side)),
    }
}

/// Head of the editor/comment box for a new comment at the cursor: `selection <tag>`, `line L<no>`, `line r<row+1>`.
pub fn cursor_head(state: &State) -> String {
    if let Some(sel) = visual::selection(state) {
        return format!("selection {}", visual::sel_tag(state, &sel));
    }
    let side = nav::side(state);
    let cur = nav::cursor_row(state);
    match state.rows.rows.get(cur).and_then(|r| rows::row_no(r, side)) {
        Some(n) => format!("line L{n}"),
        None => format!("line r{}", cur + 1),
    }
}

/// Build a comment anchored at the current cursor/selection with `message` (F-COMMENT-03, F-COMMENT-05,
/// F-VISUAL-03): line/side/deleted flags, selection info, text, `context` (+-3 rows) and `wide` (+-15 rows).
/// Does not insert it, but reserves its id and sequence number.
pub fn from_cursor(state: &mut State, message: &str) -> Comment {
    let side = nav::side(state);
    let sel = visual::selection(state);
    let head = cursor_head(state);
    let (last, cur) = (nav::last_row(state), nav::cursor_row(state));
    let cur_row = state.rows.rows.get(cur);
    let cur_line = cur_row.and_then(|r| rows::row_no(r, side));
    let (first, ar) = match &sel {
        Some(s) => (s.sr.min(last), s.er.min(last)),
        None => (cur, cur),
    };
    let anchor = state.rows.rows.get(ar);
    let line = anchor.and_then(|r| rows::row_no(r, side));
    let deleted = anchor.is_some_and(is_del);
    let mut text = cur_row.map(|r| raw_text(r, side)).unwrap_or_default();
    let mut selection = None;
    if let Some(s) = &sel {
        text = visual::selection_text(state, s);
        let lab = |i: usize| state.rows.rows.get(i).and_then(|r| rows::row_no(r, side)).or(cur_line);
        let end_len = state.rows.rows.get(s.er).map(|r| rows::row_code(r, side).chars().count()).unwrap_or(0);
        selection = Some(SelectionInfo {
            start_line: lab(s.sr),
            end_line: lab(s.er),
            start_col: if s.line { 1 } else { s.sc as u32 + 1 },
            end_col: if s.line { end_len.max(1) as u32 } else { s.ec as u32 + 1 },
        });
    }
    let mut context = Vec::new();
    let mut wide = Vec::new();
    if !state.rows.rows.is_empty() {
        for i in first.saturating_sub(CTX_WIDE)..=(ar + CTX_WIDE).min(last) {
            if let Some(r @ (ShownRow::Line(_) | ShownRow::Pair { .. })) = state.rows.rows.get(i) {
                let t = raw_text(r, side);
                wide.push(t.clone());
                if i + CTX >= first && i <= ar + CTX {
                    context.push(t);
                }
            }
        }
    }
    let file = current_path(state);
    let id = alloc_id(state);
    let seq = alloc_seq(state);
    state.thread.heads.insert(id.clone(), head);
    Comment {
        id,
        file,
        row: ar,
        side,
        line,
        deleted,
        text,
        message: message.to_string(),
        selection,
        context,
        wide,
        turns: vec![Turn { message: message.to_string(), answer: None, prior: Vec::new() }],
        origin: Origin::Human,
        number: None,
        seq,
    }
}

/// Insert keeping `seq` order (stable for equal seq); returns its id. An empty id or zero seq is filled in.
pub fn insert(state: &mut State, mut comment: Comment) -> String {
    if comment.id.is_empty() {
        comment.id = alloc_id(state);
    }
    if comment.seq == 0 {
        comment.seq = alloc_seq(state);
    }
    let id = comment.id.clone();
    let at = state.comments.partition_point(|c| c.seq <= comment.seq);
    state.comments.insert(at, comment);
    id
}

pub fn find<'a>(state: &'a State, id: &str) -> Option<&'a Comment> {
    state.comments.iter().find(|c| c.id == id)
}

/// Remove (F-COMMENT-08); clears focus when it was focused.
pub fn remove(state: &mut State, id: &str) {
    state.comments.retain(|c| c.id != id);
    if state.nav.focused_comment.as_deref() == Some(id) {
        state.nav.focused_comment = None;
    }
    if state.thread.picked_block.as_ref().is_some_and(|(p, _)| p == id) {
        state.thread.picked_block = None;
    }
    state.thread.scrolls.remove(id);
    state.thread.seen.remove(id);
    state.thread.heads.remove(id);
}

/// Comments of the current file shown under row `row` (anchor rules F-COMMENT-05: by row index + side,
/// or by line number), in creation order.
pub fn anchored_at(state: &State, row: usize) -> Vec<&Comment> {
    let path = current_path(state);
    state.comments.iter().filter(|c| c.file == path && anchored_here(state, c, row)).collect()
}

/// Ids of the current file's comments sorted by anchor row then seq (J/K order, F-COMMENT-06).
pub fn ids_in_file(state: &State) -> Vec<String> {
    let mut v: Vec<(usize, &Comment)> =
        state.comments.iter().filter_map(|c| row_of(state, c).map(|r| (r, c))).collect();
    v.sort_by_key(|(r, c)| (*r, c.seq));
    v.into_iter().map(|(_, c)| c.id.clone()).collect()
}

/// Latest turn's answer (what the UI shows as the thread state).
pub fn latest_answer(c: &Comment) -> Option<&Answer> {
    c.turns.last().and_then(|t| t.answer.as_ref())
}

pub fn is_live(a: &Answer) -> bool {
    matches!(a.status, AnswerStatus::Pending | AnswerStatus::Streaming)
}

pub fn is_agent_note(c: &Comment) -> bool {
    c.origin == Origin::Agent
}

/// Number of turns (a comment without turns counts as one).
pub fn turn_count(c: &Comment) -> usize {
    c.turns.len().max(1)
}

/// A follow-up may be sent: last turn answered and not live; agent note with no reply yet (F-ASK-03).
pub fn can_follow_up(c: &Comment) -> bool {
    match c.turns.last().and_then(|t| t.answer.as_ref()) {
        Some(a) => !is_live(a),
        None => is_agent_note(c) && turn_count(c) == 1,
    }
}

/// The `a` key opens the follow-up editor: latest answer done, or an agent note awaiting a first reply (F-ASK-02).
pub fn can_reply(c: &Comment) -> bool {
    latest_answer(c).is_some_and(|a| a.status == AnswerStatus::Done) || (is_agent_note(c) && can_follow_up(c))
}

/// Replace the message of comment `id` (and its first turn). Refused once follow-ups exist (F-COMMENT-07).
pub fn edit_message(state: &mut State, id: &str, message: &str) -> bool {
    let Some(c) = state.comments.iter_mut().find(|c| c.id == id) else { return false };
    if c.turns.len() > 1 {
        return false;
    }
    c.message = message.to_string();
    match c.turns.first_mut() {
        Some(t) => t.message = message.to_string(),
        None => c.turns.push(Turn { message: message.to_string(), answer: None, prior: Vec::new() }),
    }
    true
}

/// Append an unanswered follow-up turn to comment `id`. Returns the new 1-based turn number.
pub fn append_turn(state: &mut State, id: &str, message: &str) -> Option<u32> {
    let c = state.comments.iter_mut().find(|c| c.id == id)?;
    c.turns.push(Turn { message: message.to_string(), answer: None, prior: Vec::new() });
    Some(c.turns.len() as u32)
}

/// Human comment, one turn, no answer or error/cancelled answer (F-ASK-02).
pub fn can_ask(c: &Comment) -> bool {
    !is_agent_note(c)
        && turn_count(c) <= 1
        && latest_answer(c).is_none_or(|a| matches!(a.status, AnswerStatus::Error | AnswerStatus::Cancelled))
}

/// Box head (F-COMMENT-04, F-COMMENT-10): `#<n> agent note L<line>`, `selection <tag>`, `line L<no>`, `line r<row+1>`.
pub fn head(state: &State, c: &Comment) -> String {
    if is_agent_note(c) {
        let num = c.number.map(|n| format!("#{n} ")).unwrap_or_default();
        let at = c.line.map(|l| format!(" L{l}")).unwrap_or_default();
        return format!("{num}agent note{at}");
    }
    if let Some(h) = state.thread.heads.get(&c.id) {
        return h.clone();
    }
    match (&c.selection, c.line) {
        (Some(s), _) => match (s.start_line, s.end_line) {
            (Some(a), Some(b)) if a != b => format!("selection L{a}-{b}"),
            (Some(a), _) => format!("selection L{a}"),
            _ => format!("selection r{}", c.row + 1),
        },
        (None, Some(n)) => format!("line L{n}"),
        (None, None) => format!("line r{}", c.row + 1),
    }
}

/// Quoted selection lines shown in the box head area (human comments with a selection).
pub fn quoted_lines(c: &Comment) -> Vec<String> {
    if c.origin == Origin::Human && c.selection.is_some() {
        c.text.split('\n').map(str::to_string).collect()
    } else {
        Vec::new()
    }
}

#[cfg(test)]
pub(crate) mod testutil {
    use super::*;
    use crate::diff::{FileDiff, Status};
    use crate::rows::RowLine;
    use crate::state::LoadState;

    pub fn file(path: &str) -> FileDiff {
        FileDiff {
            path: path.to_string(),
            old_path: None,
            status: Status::Modified,
            adds: 0,
            dels: 0,
            hunks: Vec::new(),
            note: None,
        }
    }

    pub fn line(kind: LineKind, old_no: Option<u32>, new_no: Option<u32>, text: &str) -> ShownRow {
        ShownRow::Line(RowLine { kind, old_no, new_no, text: text.to_string(), no_newline_marker: false })
    }

    /// Context line `n` (old = new = n), text `l<n>`.
    pub fn ctx(n: u32) -> ShownRow {
        line(LineKind::Context, Some(n), Some(n), &format!("l{n}"))
    }

    /// State with file `a.rs` and the given (unified) rows; no rows-cache rebuild needed.
    pub fn state_with(rows: Vec<ShownRow>) -> State {
        let mut st = crate::state::testutil::fake_state();
        st.loader.pending.clear();
        st.load = LoadState::Ready;
        st.ready = true;
        st.files = vec![file("a.rs")];
        st.rows.rows = rows;
        // pretend the cache is fresh so `rows::ensure` keeps the hand-built rows
        st.rows.key =
            Some(crate::rows::RowsKey { file_index: 0, split: false, browse: None, files_gen: st.files_gen });
        st
    }

    /// Plain human comment on `a.rs` line `line` (new side), for hand-built states.
    pub fn comment(id: &str, seq: u64, line: u32, message: &str) -> Comment {
        Comment {
            id: id.to_string(),
            file: "a.rs".to_string(),
            row: 0,
            side: PaneSide::New,
            line: Some(line),
            deleted: false,
            text: String::new(),
            message: message.to_string(),
            selection: None,
            context: Vec::new(),
            wide: Vec::new(),
            turns: vec![Turn { message: message.to_string(), answer: None, prior: Vec::new() }],
            origin: Origin::Human,
            number: None,
            seq,
        }
    }

    pub fn answer(status: AnswerStatus, text: &str) -> Answer {
        Answer { status, text: text.to_string(), agent: None }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::testutil::{comment, ctx, line, state_with};
    use super::*;
    use crate::rows::RowLine;
    use crate::state::{Selection, SelectionKind};

    fn many(n: u32) -> State {
        state_with((1..=n).map(ctx).collect())
    }

    #[test]
    fn f_ask_01_ids_sequence_from_q1() {
        let mut s = many(2);
        assert_eq!(alloc_id(&mut s), "q1");
        assert_eq!(alloc_id(&mut s), "q2");
        assert_eq!(alloc_seq(&mut s), 1);
        assert_eq!(alloc_seq(&mut s), 2);
    }

    #[test]
    fn f_comment_03_insert_keeps_seq_order_and_find_remove() {
        let mut s = many(3);
        insert(&mut s, comment("q3", 3, 1, "c"));
        insert(&mut s, comment("q1", 1, 1, "a"));
        insert(&mut s, comment("q2", 2, 1, "b"));
        let ids: Vec<_> = s.comments.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["q1", "q2", "q3"]);
        assert_eq!(find(&s, "q2").map(|c| c.message.as_str()), Some("b"));
        assert!(find(&s, "nope").is_none());
        s.nav.focused_comment = Some("q2".into());
        remove(&mut s, "q2");
        assert_eq!(s.comments.len(), 2);
        assert_eq!(s.nav.focused_comment, None);
        remove(&mut s, "q1");
        assert_eq!(s.comments.len(), 1);
    }

    #[test]
    fn f_comment_03_from_cursor_records_line_and_context() {
        let mut s = many(30);
        s.nav.row = 10;
        let c = from_cursor(&mut s, "why");
        assert_eq!((c.id.as_str(), c.file.as_str(), c.row), ("q1", "a.rs", 10));
        assert_eq!((c.line, c.side, c.deleted), (Some(11), PaneSide::New, false));
        assert_eq!(c.text, "l11");
        assert_eq!(c.selection, None);
        assert_eq!(c.context.len(), 7, "+-3 rows");
        assert_eq!(c.context.first().map(String::as_str), Some("l8"));
        assert_eq!(c.context.last().map(String::as_str), Some("l14"));
        assert_eq!(c.wide.len(), 26, "rows 0..=25");
        assert_eq!(c.wide.first().map(String::as_str), Some("l1"));
        assert_eq!(c.turns.len(), 1);
        assert_eq!(c.origin, Origin::Human);
        assert_eq!(head(&s, &c), "line L11");
    }

    #[test]
    fn f_comment_03_context_skips_hunk_rows_and_clamps() {
        let mut s = state_with(vec![ShownRow::Hunk("@@ x @@".into()), ctx(1), ctx(2)]);
        s.nav.row = 0;
        let c = from_cursor(&mut s, "m");
        assert_eq!(c.text, "@@ x @@");
        assert_eq!(c.line, None);
        assert_eq!(c.context, ["l1", "l2"]);
        assert_eq!(head(&s, &c), "line r1");
    }

    #[test]
    fn f_comment_05_deleted_row_anchors_new_side_with_old_number() {
        let mut s = state_with(vec![
            ctx(1),
            line(LineKind::Del, Some(2), None, "gone"),
            line(LineKind::Add, None, Some(2), "new"),
        ]);
        s.nav.row = 1;
        let c = from_cursor(&mut s, "m");
        assert_eq!((c.line, c.side, c.deleted), (Some(2), PaneSide::New, true));
        assert_eq!(c.text, "gone");
        let id = insert(&mut s, c);
        assert_eq!(anchored_at(&s, 1).len(), 1, "del row");
        assert!(anchored_at(&s, 2).is_empty(), "not the add row with the same number");
        assert!(anchored_at(&s, 0).is_empty());
        assert_eq!(ids_in_file(&s), [id]);
    }

    #[test]
    fn f_comment_05_new_side_matches_new_no_and_old_side_matches_old_no() {
        let mut s = state_with(vec![
            line(LineKind::Del, Some(5), None, "d"),
            line(LineKind::Add, None, Some(5), "a"),
            ctx(6),
        ]);
        let mut n = comment("q1", 1, 5, "new");
        n.side = PaneSide::New;
        let mut o = comment("q2", 2, 5, "old");
        o.side = PaneSide::Old;
        s.comments.extend([n, o]);
        let at = |s: &State, r| anchored_at(s, r).iter().map(|c| c.id.clone()).collect::<Vec<_>>();
        assert_eq!(at(&s, 1), ["q1"]);
        assert_eq!(at(&s, 0), ["q2"], "old side: del/context row with old no");
        assert!(at(&s, 2).is_empty());
    }

    #[test]
    fn f_comment_05_pair_rows_split_anchoring() {
        let l =
            |k, o, n| RowLine { kind: k, old_no: o, new_no: n, text: "x".into(), no_newline_marker: false };
        let del_only = ShownRow::Pair { l: Some(l(LineKind::Del, Some(3), None)), r: None };
        let changed = ShownRow::Pair {
            l: Some(l(LineKind::Del, Some(4), None)),
            r: Some(l(LineKind::Add, None, Some(4))),
        };
        let mut s = state_with(vec![del_only, changed]);
        let mut a = comment("q1", 1, 3, "a");
        a.deleted = true;
        let b = comment("q2", 2, 4, "b");
        let mut c = comment("q3", 3, 4, "c");
        c.side = PaneSide::Old;
        s.comments.extend([a, b, c]);
        let at = |s: &State, r| anchored_at(s, r).iter().map(|c| c.id.clone()).collect::<Vec<_>>();
        assert_eq!(at(&s, 0), ["q1"]);
        assert_eq!(at(&s, 1), ["q2", "q3"], "creation order, both panes");
    }

    #[test]
    fn f_comment_05_line_absent_hides_box_and_row_anchor() {
        let mut s = many(2);
        s.comments.push(comment("q1", 1, 99, "gone"));
        assert!(anchored_at(&s, 0).is_empty() && anchored_at(&s, 1).is_empty());
        assert!(ids_in_file(&s).is_empty());
        let mut r = comment("q2", 2, 1, "hunk");
        r.line = None;
        r.row = 0;
        s.rows.rows = vec![ShownRow::Hunk("@@".into()), ctx(1)];
        s.comments.push(r);
        assert_eq!(anchored_at(&s, 0).len(), 1, "row index anchor");
        let mut other = comment("q3", 3, 1, "x");
        other.file = "b.rs".into();
        s.comments.push(other);
        assert_eq!(ids_in_file(&s), ["q2"], "other file and absent line excluded");
    }

    #[test]
    fn f_comment_06_ids_sorted_by_row_then_seq() {
        let mut s = many(4);
        s.comments.push(comment("q1", 1, 3, "c"));
        s.comments.push(comment("q2", 2, 1, "a"));
        s.comments.push(comment("q3", 3, 1, "a2"));
        assert_eq!(ids_in_file(&s), ["q2", "q3", "q1"]);
    }

    #[test]
    fn f_visual_03_selection_info_char_and_line() {
        let mut s = many(5);
        s.nav.row = 3;
        s.nav.col = 1;
        s.nav.selection = Some(Selection { kind: SelectionKind::Char, anchor_row: 1, anchor_col: 0 });
        let c = from_cursor(&mut s, "m");
        let sel = c.selection.clone().unwrap();
        assert_eq!((sel.start_line, sel.end_line), (Some(2), Some(4)));
        assert_eq!((sel.start_col, sel.end_col), (1, 2));
        assert_eq!(c.text, "l2\nl3\nl4", "rows cut at cols");
        assert_eq!(c.row, 3, "anchored under the last selected row");
        assert_eq!(c.line, Some(4));
        assert_eq!(head(&s, &c), "selection L2:C1-L4:C2");
        assert_eq!(quoted_lines(&c).len(), 3);
        s.nav.selection = Some(Selection { kind: SelectionKind::Line, anchor_row: 1, anchor_col: 0 });
        let c = from_cursor(&mut s, "m");
        let sel = c.selection.unwrap();
        assert_eq!((sel.start_col, sel.end_col), (1, 2));
        assert_eq!(c.text, "l2\nl3\nl4");
    }

    #[test]
    fn f_visual_03_upward_selection_anchors_at_last_row() {
        let mut s = many(5);
        s.nav.row = 1;
        s.nav.selection = Some(Selection { kind: SelectionKind::Line, anchor_row: 3, anchor_col: 0 });
        let c = from_cursor(&mut s, "m");
        assert_eq!(c.row, 3);
        assert_eq!(head(&s, &c), "selection L2-4");
    }

    #[test]
    fn f_comment_10_agent_note_head() {
        let s = many(1);
        let mut c = comment("q1", 1, 7, "n");
        c.origin = Origin::Agent;
        assert_eq!(head(&s, &c), "agent note L7");
        c.number = Some(3);
        assert_eq!(head(&s, &c), "#3 agent note L7");
        assert!(quoted_lines(&c).is_empty());
    }

    #[test]
    fn f_ask_03_follow_up_and_ask_rules() {
        use super::testutil::answer;
        let mut c = comment("q1", 1, 1, "m");
        assert!(can_ask(&c) && !can_follow_up(&c));
        c.turns[0].answer = Some(answer(AnswerStatus::Pending, ""));
        assert!(!can_ask(&c) && !can_follow_up(&c));
        c.turns[0].answer = Some(answer(AnswerStatus::Done, "a"));
        assert!(!can_ask(&c) && can_follow_up(&c));
        c.turns[0].answer = Some(answer(AnswerStatus::Cancelled, "MCP stopped"));
        assert!(can_ask(&c));
        c.origin = Origin::Agent;
        c.turns[0].answer = None;
        assert!(!can_ask(&c) && can_follow_up(&c), "agent note with no reply yet");
    }

    #[test]
    fn f_ask_02_can_reply_rule() {
        use super::testutil::answer;
        let mut c = comment("q1", 1, 1, "m");
        assert!(!can_reply(&c));
        c.turns[0].answer = Some(answer(AnswerStatus::Done, "a"));
        assert!(can_reply(&c));
        c.turns[0].answer = Some(answer(AnswerStatus::Error, "e"));
        assert!(!can_reply(&c));
        c.origin = Origin::Agent;
        c.turns[0].answer = None;
        assert!(can_reply(&c), "agent note awaiting a first reply");
    }

    #[test]
    fn f_comment_07_edit_message_and_append_turn() {
        let mut s = state_with(vec![]);
        s.comments.push(comment("q1", 1, 1, "old"));
        assert!(edit_message(&mut s, "q1", "new"));
        assert_eq!((s.comments[0].message.as_str(), s.comments[0].turns[0].message.as_str()), ("new", "new"));
        assert_eq!(append_turn(&mut s, "q1", "more"), Some(2));
        assert!(!edit_message(&mut s, "q1", "again"), "no edit after follow-ups");
        assert_eq!(s.comments[0].message, "new");
        assert_eq!(append_turn(&mut s, "zz", "x"), None);
    }
}
