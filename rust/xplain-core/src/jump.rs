//! File and change jumps, initial position, cursor memory across reloads.
//!
//! Spec: F-NAV-05 (`]` `[`), F-NAV-06 (Tab/S-Tab), F-NAV-08 (initial position per file), F-NAV-10 (top reset
//! cases), F-RELOAD-03 (cursor kept, hops to comments in unchanged files), F-FILES-02 (open by index).
//! Oracle: `sw`, `changeStarts` use, reload cursor mapping in `src/app.tsx`.
//! Owner: component `navops` (C). Uses `nav::place`, `nav::viewport`. Must not: know the picker/search UI.

use crate::comments::PaneSide;
use crate::diff::LineKind;
use crate::effect::Fx;
use crate::keys::{Key, KeyEvent};
use crate::nav::viewport;
use crate::rows::{self, ShownRow};
use crate::state::{PaneChoice, State};

/// Context rows kept above the first change on landing (`CTX` in app.tsx).
const CTX: usize = 3;

/// `]` `[` Tab BackTab in cursor context. Returns true when consumed.
pub fn on_key(state: &mut State, key: KeyEvent, _fx: &mut Fx) -> bool {
    if key.mods.ctrl {
        return false;
    }
    match key.key {
        Key::Char(c @ (']' | '[')) if !key.mods.alt => {
            state.nav.count = 0;
            // a motion key unfocuses a focused comment
            state.nav.focused_comment = None;
            state.thread.picked_block = None;
            let cur = state.nav.row;
            let starts = &state.rows.change_starts;
            let target = if c == ']' {
                starts.iter().copied().find(|s| *s > cur)
            } else {
                starts.iter().rev().copied().find(|s| *s < cur)
            };
            if let Some(row) = target {
                crate::nav::place(state, row, None);
            }
            true
        }
        Key::Tab | Key::BackTab => {
            state.nav.count = 0;
            if state.browse.is_some() || state.files.is_empty() {
                return true;
            }
            let n = state.files.len();
            let cur = state.nav.file_index.min(n - 1);
            let next = if key.key == Key::Tab { (cur + 1) % n } else { (cur + n - 1) % n };
            open_file(state, next);
            true
        }
        _ => false,
    }
}

/// Switch to file `index` (clamped): reset selection/focus, `rows::ensure`, initial position (F-NAV-08),
/// top per F-NAV-10 when landing on the same file.
pub fn open_file(state: &mut State, index: usize) {
    if state.files.is_empty() {
        return;
    }
    let index = index.min(state.files.len() - 1);
    if state.browse.is_none() && state.nav.file_index == index {
        // rows unchanged: cursor untouched, top reset (F-NAV-10)
        viewport::reset_top_keep_cursor(state);
        return;
    }
    state.browse = None;
    state.nav.file_index = index;
    state.nav.selection = None;
    state.nav.focused_comment = None;
    state.thread.picked_block = None;
    rows::ensure(state);
    initial_position(state);
}

/// F-NAV-08 initial cursor row/col/top for the current file.
pub fn initial_position(state: &mut State) {
    let first = if state.settings.full { state.rows.change_starts.first().copied() } else { None };
    state.nav.count = 0;
    state.nav.selection = None;
    state.nav.pane = PaneChoice::New;
    state.nav.col = 0;
    state.nav.sticky_end = false;
    state.nav.x_shift = 0;
    state.nav.row = first.unwrap_or(0);
    state.nav.top = first.map_or(0, |f| f.saturating_sub(CTX));
    viewport::clamp_top(state);
    viewport::follow(state);
}

/// Cursor identity captured before a reload to restore afterwards (F-RELOAD-03).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CursorMemo {
    /// Path of the diff file shown (kept while browsing so the file selection survives a reload).
    pub path: Option<String>,
    pub row: usize,
    pub col: usize,
    pub top: usize,
    pub line_no: Option<u32>,
    pub deleted: bool,
    pub side_new: bool,
}

/// Deleted-only row (`isDel` in app.tsx).
fn is_del(row: &ShownRow) -> bool {
    match row {
        ShownRow::Line(l) => l.kind == LineKind::Del,
        ShownRow::Pair { r, .. } => r.is_none(),
        _ => false,
    }
}

/// Does `row` carry source line `no` (`anchors` in app.tsx)?
fn anchors(row: &ShownRow, no: u32, del: bool, side: PaneSide) -> bool {
    match row {
        ShownRow::Pair { l, r } => {
            if side == PaneSide::Old {
                l.as_ref().is_some_and(|x| x.old_no == Some(no))
            } else if del {
                r.is_none() && l.as_ref().is_some_and(|x| x.old_no == Some(no))
            } else {
                r.as_ref().is_some_and(|x| x.new_no == Some(no))
            }
        }
        ShownRow::Line(x) => {
            if side == PaneSide::Old {
                x.kind != LineKind::Add && x.old_no == Some(no)
            } else if del {
                x.kind == LineKind::Del && x.old_no == Some(no)
            } else {
                x.new_no == Some(no)
            }
        }
        _ => false,
    }
}

/// New-side line number per row (`newNos`).
pub(crate) fn new_nos(rows: &[ShownRow]) -> Vec<Option<u32>> {
    rows.iter()
        .map(|r| match r {
            ShownRow::Line(l) => l.new_no,
            ShownRow::Pair { r, .. } => r.as_ref().and_then(|x| x.new_no),
            _ => None,
        })
        .collect()
}

/// Index of the numbered row nearest to `n`, first on ties (`nearest`).
pub(crate) fn nearest(nos: &[Option<u32>], n: u64) -> Option<usize> {
    let mut best: Option<(usize, u64)> = None;
    for (i, no) in nos.iter().enumerate() {
        if let Some(v) = no {
            let d = u64::from(*v).abs_diff(n);
            if best.is_none_or(|(_, bd)| d < bd) {
                best = Some((i, d));
            }
        }
    }
    best.map(|(i, _)| i)
}

fn shown_path(state: &State) -> Option<String> {
    state.files.get(state.nav.file_index).map(|f| f.path.clone())
}

pub fn remember(state: &State) -> CursorMemo {
    let side = PaneSide::from(state.nav.pane);
    let row = state.rows.rows.get(state.nav.row.min(state.rows.rows.len().saturating_sub(1)));
    CursorMemo {
        path: shown_path(state),
        row: state.nav.row,
        col: state.nav.col,
        top: state.nav.top,
        line_no: row.and_then(|r| rows::row_no(r, side)),
        deleted: row.is_some_and(is_del),
        side_new: side == PaneSide::New,
    }
}

/// After new `files` arrived: map the memo onto the new rows (keep path if present, else first file).
pub fn restore(state: &mut State, memo: &CursorMemo) {
    let found = memo.path.as_ref().and_then(|p| state.files.iter().position(|f| &f.path == p));
    state.nav.file_index = found.unwrap_or(0);
    rows::ensure(state);
    if found.is_none() && state.browse.is_none() {
        state.nav.focused_comment = None;
        initial_position(state);
        return;
    }
    let side = if memo.side_new { PaneSide::New } else { PaneSide::Old };
    let rows = &state.rows.rows;
    let last = rows.len().saturating_sub(1);
    let by_line = memo.line_no.and_then(|no| {
        rows.iter()
            .position(|r| anchors(r, no, memo.deleted, side))
            .or_else(|| nearest(&new_nos(rows), u64::from(no)))
    });
    state.nav.row = by_line.unwrap_or_else(|| memo.row.min(last));
    state.nav.col = memo.col;
    state.nav.pane = if memo.side_new { PaneChoice::New } else { PaneChoice::Old };
    state.nav.top = memo.top;
    state.nav.selection = None;
    state.nav.count = 0;
    viewport::clamp_top(state);
    viewport::follow(state);
}

#[cfg(test)]
pub(crate) mod testkit {
    //! Hand-built states for the navops unit tests.
    use crate::diff::{DiffLine, FileDiff, Hunk, LineKind, Status};
    use crate::state::*;

    pub fn line(kind: LineKind, old: Option<u32>, new: Option<u32>, text: &str) -> DiffLine {
        DiffLine { kind, old_no: old, new_no: new, text: text.into(), no_newline_marker: false }
    }

    /// File with one hunk: ctx 1, ctx 2, del 3, add 3, ctx 4, ctx 5.
    pub fn file(path: &str) -> FileDiff {
        FileDiff {
            path: path.into(),
            old_path: None,
            status: Status::Modified,
            adds: 1,
            dels: 1,
            hunks: vec![Hunk {
                header: "@@ -1,5 +1,5 @@".into(),
                lines: vec![
                    line(LineKind::Context, Some(1), Some(1), "one"),
                    line(LineKind::Context, Some(2), Some(2), "two"),
                    line(LineKind::Del, Some(3), None, "three"),
                    line(LineKind::Add, None, Some(3), "THREE"),
                    line(LineKind::Context, Some(4), Some(4), "four"),
                    line(LineKind::Context, Some(5), Some(5), "five"),
                ],
            }],
            note: None,
        }
    }

    pub fn state(files: Vec<FileDiff>) -> State {
        let mut s = crate::state::testutil::fake_state();
        s.files = files;
        s.files_gen += 1;
        s.load = LoadState::Ready;
        s.ready = true;
        s.pending.clear();
        crate::rows::ensure(&mut s);
        s
    }
}

#[cfg(test)]
mod tests {
    use super::testkit::*;
    use super::*;
    use crate::state::Selection;

    fn keys(state: &mut State, k: KeyEvent) -> bool {
        on_key(state, k, &mut Vec::new())
    }

    #[test]
    fn f_nav_05_change_jumps() {
        let mut s = state(vec![file("a.rs")]);
        // rows: hunk, 1, 2, del 3, add 3, 4, 5 -> change start at row 3
        assert_eq!(s.rows.change_starts, vec![3]);
        s.nav.row = 0;
        s.nav.count = 4;
        assert!(keys(&mut s, KeyEvent::ch(']')));
        assert_eq!(s.nav.row, 3);
        assert_eq!(s.nav.count, 0);
        assert!(keys(&mut s, KeyEvent::ch(']')));
        assert_eq!(s.nav.row, 3);
        s.nav.row = 6;
        assert!(keys(&mut s, KeyEvent::ch('[')));
        assert_eq!(s.nav.row, 3);
        assert!(keys(&mut s, KeyEvent::ch('[')));
        assert_eq!(s.nav.row, 3);
    }

    #[test]
    fn f_nav_05_unfocuses_and_ignores_ctrl() {
        let mut s = state(vec![file("a.rs")]);
        s.nav.focused_comment = Some("q1".into());
        assert!(!keys(&mut s, KeyEvent::ctrl(']')));
        assert!(s.nav.focused_comment.is_some());
        assert!(keys(&mut s, KeyEvent::ch(']')));
        assert!(s.nav.focused_comment.is_none());
    }

    #[test]
    fn f_nav_06_tab_wraps_and_resets_position() {
        let mut s = state(vec![file("a.rs"), file("b.rs")]);
        s.nav.row = 5;
        assert!(keys(&mut s, KeyEvent::plain(Key::Tab)));
        assert_eq!(s.nav.file_index, 1);
        assert_eq!(s.nav.row, 3);
        assert!(keys(&mut s, KeyEvent::plain(Key::Tab)));
        assert_eq!(s.nav.file_index, 0);
        assert!(keys(&mut s, KeyEvent::plain(Key::BackTab)));
        assert_eq!(s.nav.file_index, 1);
    }

    #[test]
    fn f_nav_06_one_file_keeps_cursor() {
        let mut s = state(vec![file("a.rs")]);
        s.nav.row = 5;
        assert!(keys(&mut s, KeyEvent::plain(Key::Tab)));
        assert_eq!(s.nav.file_index, 0);
        assert_eq!(s.nav.row, 5);
    }

    #[test]
    fn f_nav_06_browse_and_empty_ignored() {
        let mut s = state(vec![file("a.rs"), file("b.rs")]);
        s.browse = Some(crate::state::Browse { path: "x".into(), lines: vec!["l".into()] });
        assert!(keys(&mut s, KeyEvent::plain(Key::Tab)));
        assert_eq!(s.nav.file_index, 0);
        let mut e = state(Vec::new());
        assert!(keys(&mut e, KeyEvent::plain(Key::Tab)));
    }

    #[test]
    fn f_nav_08_initial_position_scope() {
        let mut s = state(vec![file("a.rs")]);
        s.settings.full = true;
        s.nav.selection =
            Some(Selection { kind: crate::state::SelectionKind::Char, anchor_row: 0, anchor_col: 0 });
        s.nav.col = 7;
        initial_position(&mut s);
        assert_eq!((s.nav.row, s.nav.col, s.nav.top), (3, 0, 0));
        assert!(s.nav.selection.is_none());
        s.settings.full = false;
        initial_position(&mut s);
        assert_eq!(s.nav.row, 0);
    }

    #[test]
    fn f_reload_03_line_kept_after_shift() {
        let mut s = state(vec![file("a.rs")]);
        s.nav.row = 5; // ctx 4
        s.nav.col = 2;
        let memo = remember(&s);
        assert_eq!(memo.line_no, Some(4));
        // new diff: file gets an extra context line on top
        let mut f = file("a.rs");
        f.hunks[0].lines.insert(0, line(LineKind::Context, Some(0), Some(0), "zero"));
        f.hunks[0].lines.iter_mut().skip(1).for_each(|l| {
            l.old_no = l.old_no.map(|n| n + 1);
            l.new_no = l.new_no.map(|n| n + 1);
        });
        s.files = vec![f];
        s.files_gen += 1;
        restore(&mut s, &memo);
        // follows the line number: new-side line 4 is now the add row of the shifted hunk
        assert_eq!(s.nav.row, 5);
        assert_eq!(s.nav.col, 2);
    }

    #[test]
    fn f_reload_03_missing_path_goes_first_file() {
        let mut s = state(vec![file("a.rs"), file("b.rs")]);
        s.nav.file_index = 1;
        let memo = remember(&s);
        s.files = vec![file("c.rs")];
        s.files_gen += 1;
        restore(&mut s, &memo);
        assert_eq!(s.nav.file_index, 0);
        assert_eq!(s.nav.row, 3);
    }

    #[test]
    fn f_reload_03_nearest_and_clamp() {
        let mut s = state(vec![file("a.rs")]);
        s.nav.row = 6; // line 5
        let memo = remember(&s);
        let mut f = file("a.rs");
        f.hunks[0].lines.truncate(4); // lines 1,2,del3,add3: line 5 gone
        s.files = vec![f];
        s.files_gen += 1;
        restore(&mut s, &memo);
        assert_eq!(s.nav.row, 4); // nearest new-side line 3 (add row)
    }

    #[test]
    fn f_reload_03_deleted_flag_kept() {
        let mut s = state(vec![file("a.rs")]);
        s.nav.row = 3; // del 3
        let memo = remember(&s);
        assert!(memo.deleted);
        s.files_gen += 1;
        restore(&mut s, &memo);
        assert_eq!(s.nav.row, 3);
    }
}
