//! Row model of what the viewport shows: unified rows, split (paired) rows, browse rows, plus the row-level
//! text helpers every other module uses to talk about "row i".
//!
//! Spec: F-LAYOUT-03/04/05 (row structure, pairing), F-NAV-05 (change starts), F-CURSOR-06 (pane),
//! F-HEADER-03 (row numbers), F-FIND-02 (search texts), F-EDGE-02..04 (note row). Oracle:
//! `src/components/DiffView.tsx` (Row, SRow, toRows, toSplit, changeStarts, rowNo, paneOf, rowCode, findAll).
//! Owner: component `nav` (B).
//! Must not: render, mutate anything but `state.rows`, or know comments. Browse rows are `Line` rows of kind
//! `Context` with `new_no = index+1`.

use crate::comments::PaneSide;
use crate::diff::{DiffLine, FileDiff, LineKind, Note};
use crate::state::{Browse, State};
use crate::textutil::expand_tabs;

/// One code line of a row (a diff line, or a browse line).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowLine {
    pub kind: LineKind,
    pub old_no: Option<u32>,
    pub new_no: Option<u32>,
    /// Raw text (tabs not yet expanded).
    pub text: String,
    pub no_newline_marker: bool,
}

/// One shown row. In unified/browse mode only `Hunk`, `Note`, `Line`; in effective split `Pair` replaces `Line`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShownRow {
    Hunk(String),
    Note(String),
    Line(RowLine),
    /// Split pair: `l` old side, `r` new side; a context line is `l == r`; `None` = empty cell.
    Pair {
        l: Option<RowLine>,
        r: Option<RowLine>,
    },
}

/// What the cached rows were built for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowsKey {
    pub file_index: usize,
    pub split: bool,
    pub browse: Option<String>,
    pub files_gen: u64,
}

#[derive(Debug, Clone, Default)]
pub struct Rows {
    pub rows: Vec<ShownRow>,
    /// Indices where a run of changed rows starts (F-NAV-05).
    pub change_starts: Vec<usize>,
    pub key: Option<RowsKey>,
}

/// Split layout in effect: setting on, cols >= 100, not browse (F-LAYOUT-04).
pub fn effective_split(state: &State) -> bool {
    state.settings.split && state.size.cols >= 100 && state.browse.is_none()
}

fn current_key(state: &State) -> RowsKey {
    RowsKey {
        file_index: state.nav.file_index,
        split: effective_split(state),
        browse: state.browse.as_ref().map(|b| b.path.clone()),
        files_gen: state.files_gen,
    }
}

/// Rebuild `state.rows` when its key differs from the current one. Handlers call this after changing
/// `nav.file_index`, `settings.split`, `browse`, `files`; `update` calls it after every event.
pub fn ensure(state: &mut State) {
    let key = current_key(state);
    if state.rows.key.as_ref() == Some(&key) {
        return;
    }
    let rows = if let Some(b) = &state.browse {
        build_browse_rows(b)
    } else if let Some(f) = state.files.get(key.file_index) {
        build_rows(f, key.split)
    } else {
        Vec::new()
    };
    state.rows.change_starts = change_starts(&rows);
    state.rows.rows = rows;
    state.rows.key = Some(key);
}

fn note_text(n: Note) -> &'static str {
    match n {
        Note::RenamedNoChanges => "Renamed, no content changes",
        Note::Binary => "Binary file",
        Note::NoTextualChanges => "No textual changes",
    }
}

fn row_line(l: &DiffLine) -> RowLine {
    RowLine {
        kind: l.kind,
        old_no: l.old_no,
        new_no: l.new_no,
        text: l.text.clone(),
        no_newline_marker: l.no_newline_marker,
    }
}

/// Rows of one diff file (note row XOR hunks); `split` pairs dels/adds (F-LAYOUT-04).
pub fn build_rows(file: &FileDiff, split: bool) -> Vec<ShownRow> {
    let mut out = Vec::new();
    if let Some(n) = file.note {
        out.push(ShownRow::Note(note_text(n).to_string()));
    }
    for h in &file.hunks {
        out.push(ShownRow::Hunk(h.header.clone()));
        if split {
            pair_lines(&h.lines, &mut out);
        } else {
            out.extend(h.lines.iter().map(|l| ShownRow::Line(row_line(l))));
        }
    }
    out
}

/// Pair runs of dels and adds; context lines appear on both sides (`toSplit`).
fn pair_lines(lines: &[DiffLine], out: &mut Vec<ShownRow>) {
    let mut dels: Vec<RowLine> = Vec::new();
    let mut adds: Vec<RowLine> = Vec::new();
    fn flush(dels: &mut Vec<RowLine>, adds: &mut Vec<RowLine>, out: &mut Vec<ShownRow>) {
        let n = dels.len().max(adds.len());
        let mut d = std::mem::take(dels).into_iter();
        let mut a = std::mem::take(adds).into_iter();
        for _ in 0..n {
            out.push(ShownRow::Pair { l: d.next(), r: a.next() });
        }
    }
    for l in lines {
        match l.kind {
            LineKind::Del => dels.push(row_line(l)),
            LineKind::Add => adds.push(row_line(l)),
            LineKind::Context => {
                flush(&mut dels, &mut adds, out);
                let rl = row_line(l);
                out.push(ShownRow::Pair { l: Some(rl.clone()), r: Some(rl) });
            }
        }
    }
    flush(&mut dels, &mut adds, out);
}

/// Browse rows (F-LAYOUT-05).
pub fn build_browse_rows(browse: &Browse) -> Vec<ShownRow> {
    browse
        .lines
        .iter()
        .enumerate()
        .map(|(i, t)| {
            ShownRow::Line(RowLine {
                kind: LineKind::Context,
                old_no: None,
                new_no: u32::try_from(i + 1).ok(),
                text: t.clone(),
                no_newline_marker: false,
            })
        })
        .collect()
}

pub fn change_starts(rows: &[ShownRow]) -> Vec<usize> {
    let mut out = Vec::new();
    let mut prev = false;
    for (i, r) in rows.iter().enumerate() {
        let c = is_change(r);
        if c && !prev {
            out.push(i);
        }
        prev = c;
    }
    out
}

/// Line number the row shows for `side` (F-HEADER-03 rules): unified new else old; pair by pane.
pub fn row_no(row: &ShownRow, side: PaneSide) -> Option<u32> {
    match row {
        ShownRow::Line(l) => l.new_no.or(l.old_no),
        ShownRow::Pair { l, r } => match side {
            PaneSide::Old => l.as_ref().and_then(|x| x.old_no),
            PaneSide::New => r.as_ref().and_then(|x| x.new_no).or_else(|| l.as_ref().and_then(|x| x.old_no)),
        },
        _ => None,
    }
}

/// Pane that really carries the cursor: del-only pairs always use the left pane.
pub fn pane_of(row: &ShownRow, side: PaneSide) -> PaneSide {
    match row {
        ShownRow::Pair { r, .. } if side == PaneSide::Old || r.is_none() => PaneSide::Old,
        _ => PaneSide::New,
    }
}

/// Tab-expanded code text under the cursor for this row and pane. Hunk and note rows have their text as
/// code too, as in `rowCode`.
pub fn row_code(row: &ShownRow, side: PaneSide) -> String {
    match row {
        ShownRow::Hunk(t) | ShownRow::Note(t) => expand_tabs(t),
        ShownRow::Line(l) => expand_tabs(&l.text),
        ShownRow::Pair { l, r } => {
            let cell = if pane_of(row, side) == PaneSide::Old { l } else { r };
            cell.as_ref().map(|x| expand_tabs(&x.text)).unwrap_or_default()
        }
    }
}

/// Whether the row is a change (add/del, or pair with any non-context side).
pub fn is_change(row: &ShownRow) -> bool {
    match row {
        ShownRow::Line(l) => l.kind != LineKind::Context,
        ShownRow::Pair { l, r } => {
            l.as_ref().map(|x| x.kind) != Some(LineKind::Context)
                || r.as_ref().map(|x| x.kind) != Some(LineKind::Context)
        }
        _ => false,
    }
}

fn fold_case(c: char) -> char {
    let mut it = c.to_lowercase();
    match (it.next(), it.next()) {
        (Some(l), None) => l,
        _ => c,
    }
}

/// Smart-case substring hits as `[start, end)` char ranges (F-FIND-02): case-insensitive unless the term has
/// an uppercase char. Hits do not overlap.
pub fn find_all(text: &str, term: &str) -> Vec<(usize, usize)> {
    if term.is_empty() {
        return Vec::new();
    }
    let ci = term == term.to_lowercase();
    let hay: Vec<char> = if ci { text.chars().map(fold_case).collect() } else { text.chars().collect() };
    let needle: Vec<char> = term.chars().collect();
    let n = needle.len();
    let mut out = Vec::new();
    let mut i = 0;
    while i + n <= hay.len() {
        if hay[i..i + n] == needle[..] {
            out.push((i, i + n));
            i += n;
        } else {
            i += 1;
        }
    }
    out
}

/// Texts of a row that find looks at (pair: both sides, deduplicated; hunk/note: none) (F-FIND-02).
pub fn search_texts(row: &ShownRow) -> Vec<String> {
    match row {
        ShownRow::Line(l) => vec![expand_tabs(&l.text)],
        ShownRow::Pair { l, r } => {
            let mut out = Vec::new();
            if let Some(x) = l {
                out.push(expand_tabs(&x.text));
            }
            if let Some(y) = r
                && l.as_ref() != Some(y)
            {
                out.push(expand_tabs(&y.text));
            }
            out
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::{Hunk, Status};

    pub(crate) fn dl(kind: LineKind, o: Option<u32>, n: Option<u32>, t: &str) -> DiffLine {
        DiffLine { kind, old_no: o, new_no: n, text: t.to_string(), no_newline_marker: false }
    }

    pub(crate) fn file(lines: Vec<DiffLine>) -> FileDiff {
        FileDiff {
            path: "f.txt".into(),
            old_path: None,
            status: Status::Modified,
            adds: 0,
            dels: 0,
            hunks: vec![Hunk { header: "@@ -1,3 +1,3 @@".into(), lines }],
            note: None,
        }
    }

    fn sample() -> FileDiff {
        use LineKind::*;
        file(vec![
            dl(Context, Some(1), Some(1), "a"),
            dl(Del, Some(2), None, "b"),
            dl(Del, Some(3), None, "c"),
            dl(Add, None, Some(2), "B"),
            dl(Context, Some(4), Some(3), "d"),
            dl(Add, None, Some(4), "e"),
        ])
    }

    #[test]
    fn f_layout_03_unified_rows() {
        let rows = build_rows(&sample(), false);
        assert_eq!(rows.len(), 7);
        assert!(matches!(&rows[0], ShownRow::Hunk(h) if h == "@@ -1,3 +1,3 @@"));
        assert!(matches!(&rows[2], ShownRow::Line(l) if l.kind == LineKind::Del));
    }

    #[test]
    fn f_layout_03_note_row_only() {
        let mut f = sample();
        f.hunks.clear();
        f.note = Some(Note::Binary);
        assert_eq!(build_rows(&f, false), vec![ShownRow::Note("Binary file".into())]);
        f.note = Some(Note::RenamedNoChanges);
        assert_eq!(build_rows(&f, true), vec![ShownRow::Note("Renamed, no content changes".into())]);
        f.note = Some(Note::NoTextualChanges);
        assert_eq!(build_rows(&f, true), vec![ShownRow::Note("No textual changes".into())]);
    }

    #[test]
    fn f_layout_04_split_pairing() {
        let rows = build_rows(&sample(), true);
        // hunk, ctx pair, (b|B), (c|-), ctx, (-|e)
        assert_eq!(rows.len(), 6);
        match &rows[1] {
            ShownRow::Pair { l, r } => assert_eq!(l, r),
            _ => panic!("pair"),
        }
        match &rows[2] {
            ShownRow::Pair { l: Some(l), r: Some(r) } => {
                assert_eq!((l.text.as_str(), r.text.as_str()), ("b", "B"));
            }
            _ => panic!("pair"),
        }
        assert!(matches!(&rows[3], ShownRow::Pair { l: Some(_), r: None }));
        assert!(matches!(&rows[5], ShownRow::Pair { l: None, r: Some(_) }));
    }

    #[test]
    fn f_nav_05_change_starts() {
        let u = build_rows(&sample(), false);
        assert_eq!(change_starts(&u), vec![2, 6]);
        let s = build_rows(&sample(), true);
        assert_eq!(change_starts(&s), vec![2, 5]);
        assert!(is_change(&s[2]));
        assert!(!is_change(&s[1]));
        assert!(!is_change(&s[0]));
    }

    #[test]
    fn f_layout_05_browse_rows() {
        let b = Browse { path: "x".into(), lines: vec!["one".into(), "two".into()] };
        let rows = build_browse_rows(&b);
        assert_eq!(rows.len(), 2);
        assert_eq!(row_no(&rows[1], PaneSide::New), Some(2));
        assert!(change_starts(&rows).is_empty());
    }

    #[test]
    fn f_header_03_row_no() {
        let u = build_rows(&sample(), false);
        assert_eq!(row_no(&u[0], PaneSide::New), None);
        assert_eq!(row_no(&u[2], PaneSide::New), Some(2)); // del: old no
        assert_eq!(row_no(&u[4], PaneSide::New), Some(2)); // add: new no
        let s = build_rows(&sample(), true);
        assert_eq!(row_no(&s[3], PaneSide::New), Some(3)); // del-only: old no
        assert_eq!(row_no(&s[3], PaneSide::Old), Some(3));
        assert_eq!(row_no(&s[5], PaneSide::Old), None); // empty left cell
        assert_eq!(row_no(&s[5], PaneSide::New), Some(4));
        assert_eq!(row_no(&s[2], PaneSide::Old), Some(2));
        assert_eq!(row_no(&s[2], PaneSide::New), Some(2));
    }

    #[test]
    fn f_cursor_06_pane_of_and_row_code() {
        let s = build_rows(&sample(), true);
        assert_eq!(pane_of(&s[3], PaneSide::New), PaneSide::Old);
        assert_eq!(pane_of(&s[2], PaneSide::New), PaneSide::New);
        assert_eq!(pane_of(&s[2], PaneSide::Old), PaneSide::Old);
        assert_eq!(pane_of(&s[0], PaneSide::Old), PaneSide::New);
        assert_eq!(row_code(&s[2], PaneSide::Old), "b");
        assert_eq!(row_code(&s[2], PaneSide::New), "B");
        assert_eq!(row_code(&s[3], PaneSide::New), "c");
        assert_eq!(row_code(&s[5], PaneSide::Old), "");
        assert_eq!(row_code(&s[0], PaneSide::New), "@@ -1,3 +1,3 @@");
    }

    #[test]
    fn f_layout_03_row_code_expands_tabs() {
        let f = file(vec![dl(LineKind::Add, None, Some(1), "\tx")]);
        let u = build_rows(&f, false);
        assert_eq!(row_code(&u[1], PaneSide::New), "  x");
    }

    #[test]
    fn f_find_02_find_all_smart_case() {
        assert_eq!(find_all("Foo foo FOO", "foo"), vec![(0, 3), (4, 7), (8, 11)]);
        assert_eq!(find_all("Foo foo FOO", "Foo"), vec![(0, 3)]);
        assert_eq!(find_all("aaaa", "aa"), vec![(0, 2), (2, 4)]);
        assert!(find_all("abc", "").is_empty());
        assert!(find_all("abc", "x").is_empty());
        assert_eq!(find_all("héllo", "l"), vec![(2, 3), (3, 4)]);
    }

    #[test]
    fn f_find_02_search_texts() {
        let s = build_rows(&sample(), true);
        assert!(search_texts(&s[0]).is_empty());
        assert_eq!(search_texts(&s[1]), vec!["a"]);
        assert_eq!(search_texts(&s[2]), vec!["b", "B"]);
        assert_eq!(search_texts(&s[3]), vec!["c"]);
        let u = build_rows(&sample(), false);
        assert_eq!(search_texts(&u[2]), vec!["b"]);
    }
}
