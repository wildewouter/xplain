//! Visual selection: model, selected text, cursor/selection tag for the header.
//!
//! Spec: F-VISUAL-01 (start/end), F-VISUAL-02 (state side of render), F-VISUAL-03 (selection info for comments),
//! F-HEADER-03 (`[cursor L12:C3]` / `[visual ...]` tag). Oracle: `vsel`, `selText`, `selTag`, `curTag` in `src/app.tsx`.
//! Owner: component `nav` (B). Must not render.

use crate::state::{Selection, SelectionKind, State};

use super::{cursor_row, last_row, row_text, shown_col, side};

/// Ordered inclusive selection (rows, cols in chars); `line` = whole rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sel {
    pub sr: usize,
    pub sc: usize,
    pub er: usize,
    pub ec: usize,
    pub line: bool,
}

/// Current selection ordered start <= end, `None` when none active.
pub fn selection(state: &State) -> Option<Sel> {
    let s = state.nav.selection?;
    let a = (s.anchor_row.min(last_row(state)), s.anchor_col);
    let b = (cursor_row(state), shown_col(state));
    let (p, q) = if a <= b { (a, b) } else { (b, a) };
    Some(Sel { sr: p.0, sc: p.1, er: q.0, ec: q.1, line: s.kind == SelectionKind::Line })
}

/// Text of the selection, rows joined with `\n` (F-VISUAL-03).
pub fn selection_text(state: &State, sel: &Sel) -> String {
    (sel.sr..=sel.er)
        .map(|r| {
            let t = row_text(state, r);
            if sel.line {
                return t;
            }
            let from = if r == sel.sr { sel.sc } else { 0 };
            let to = if r == sel.er { sel.ec + 1 } else { usize::MAX };
            t.chars().skip(from).take(to.saturating_sub(from)).collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Char range `[from, to)` of `sel` inside row `row` for render (None when the row is outside).
pub fn sel_range_in_row(state: &State, sel: &Sel, row: usize) -> Option<(usize, usize)> {
    if row < sel.sr || row > sel.er {
        return None;
    }
    let n = row_text(state, row).chars().count();
    let a = if row == sel.sr && !sel.line { sel.sc } else { 0 };
    let b = if row == sel.er && !sel.line { sel.ec + 1 } else { n };
    if n == 0 {
        return Some((0, 1));
    }
    Some((a.min(n), b.max(a + 1).min(n)))
}

fn label(state: &State, i: usize) -> String {
    let side = side(state);
    match state.rows.rows.get(i).and_then(|r| crate::rows::row_no(r, side)) {
        Some(n) => format!("L{n}"),
        None => format!("r{}", i + 1),
    }
}

/// Selection tag for comment heads (F-VISUAL-03): `L5`, `L5-9`, `L5:C3-C7`, `L5:C3-L9:C2`.
pub fn sel_tag(state: &State, sel: &Sel) -> String {
    let (a, b) = (label(state, sel.sr), label(state, sel.er));
    if sel.line {
        return if a == b { a } else { format!("{a}-{}", b.chars().skip(1).collect::<String>()) };
    }
    if a == b {
        format!("{a}:C{}-C{}", sel.sc + 1, sel.ec + 1)
    } else {
        format!("{a}:C{}-{b}:C{}", sel.sc + 1, sel.ec + 1)
    }
}

/// Header tag text WITHOUT trailing space: `cursor new L12:C3` or `visual L4:C1` (F-HEADER-03); caller wraps
/// in `[..] `.
pub fn tag_text(state: &State) -> String {
    let kind = if state.nav.selection.is_some() { "visual" } else { "cursor" };
    let pane = if super::can_side(state) {
        match side(state) {
            crate::comments::PaneSide::Old => " old",
            crate::comments::PaneSide::New => " new",
        }
    } else {
        ""
    };
    let row = cursor_row(state);
    format!("{kind}{pane} {}:C{}", label(state, row), shown_col(state) + 1)
}

/// Start (or switch kind of) a selection at the cursor; same kind again ends it (`v`/`V`, F-VISUAL-01).
pub fn toggle(state: &mut State, line: bool) {
    let kind = if line { SelectionKind::Line } else { SelectionKind::Char };
    match state.nav.selection {
        Some(s) if s.kind == kind => end(state),
        Some(s) => state.nav.selection = Some(Selection { kind, ..s }),
        None => {
            state.nav.selection =
                Some(Selection { kind, anchor_row: cursor_row(state), anchor_col: shown_col(state) });
        }
    }
}

/// End selection (`v`/`V`/Esc) (F-VISUAL-01).
pub fn end(state: &mut State) {
    state.nav.selection = None;
}

#[cfg(test)]
mod tests {
    use super::super::testutil::*;
    use super::*;
    use crate::diff::{DiffLine, LineKind};
    use crate::keys::Key;

    fn st() -> State {
        state_with_lines(80, 24, &["hello", "", "world wide"])
    }

    #[test]
    fn f_visual_01_start_switch_end() {
        let mut s = st();
        press(&mut s, "jl");
        press(&mut s, "v");
        assert_eq!(s.nav.selection.map(|x| x.kind), Some(SelectionKind::Char));
        press(&mut s, "V");
        let sel = s.nav.selection.expect("sel");
        assert_eq!(sel.kind, SelectionKind::Line);
        assert_eq!((sel.anchor_row, sel.anchor_col), (1, 1));
        press(&mut s, "V");
        assert!(s.nav.selection.is_none());
        press(&mut s, "v");
        key(&mut s, Key::Esc);
        assert!(s.nav.selection.is_none());
        press(&mut s, "vi");
        assert!(s.nav.selection.is_some());
    }

    #[test]
    fn f_visual_01_moves_extend_and_order() {
        let mut s = st();
        press(&mut s, "jjllv");
        press(&mut s, "k");
        let sel = selection(&s).expect("sel");
        assert_eq!((sel.sr, sel.er), (1, 2));
        assert_eq!((sel.sc, sel.ec), (0, 0)); // `l` on an empty row resets desired col
        let mut s = st();
        press(&mut s, "jvjj");
        let sel = selection(&s).expect("sel");
        assert_eq!((sel.sr, sel.er), (1, 3));
    }

    #[test]
    fn f_visual_03_selection_text() {
        let mut s = st();
        press(&mut s, "jll");
        press(&mut s, "v");
        press(&mut s, "jjlll");
        let sel = selection(&s).expect("sel");
        assert_eq!(selection_text(&s, &sel), "llo\n\nworld ");
        press(&mut s, "V");
        let sel = selection(&s).expect("sel");
        assert_eq!(selection_text(&s, &sel), "hello\n\nworld wide");
    }

    #[test]
    fn f_visual_02_range_in_row() {
        let mut s = st();
        press(&mut s, "jll");
        press(&mut s, "v");
        press(&mut s, "jjlll");
        let sel = selection(&s).expect("sel");
        assert_eq!(sel_range_in_row(&s, &sel, 1), Some((2, 5)));
        assert_eq!(sel_range_in_row(&s, &sel, 2), Some((0, 1)));
        assert_eq!(sel_range_in_row(&s, &sel, 3), Some((0, 6)));
        assert_eq!(sel_range_in_row(&s, &sel, 0), None);
        press(&mut s, "V");
        let sel = selection(&s).expect("sel");
        assert_eq!(sel_range_in_row(&s, &sel, 3), Some((0, 10)));
    }

    #[test]
    fn f_header_03_tag() {
        let mut s = st();
        assert_eq!(tag_text(&s), "cursor r1:C1");
        press(&mut s, "jll");
        assert_eq!(tag_text(&s), "cursor L1:C3");
        press(&mut s, "v");
        assert_eq!(tag_text(&s), "visual L1:C3");
        press(&mut s, "$");
        assert_eq!(tag_text(&s), "visual L1:C5");
        press(&mut s, "j");
        assert_eq!(tag_text(&s), "visual L2:C1");
    }

    #[test]
    fn f_header_03_split_tag_has_pane() {
        use LineKind::*;
        let f = file_of(vec![DiffLine {
            kind: Del,
            old_no: Some(3),
            new_no: None,
            text: "x".into(),
            no_newline_marker: false,
        }]);
        let mut s = state_with_files(120, 24, vec![f]);
        s.settings.split = true;
        crate::rows::ensure(&mut s);
        press(&mut s, "j");
        assert_eq!(tag_text(&s), "cursor new L3:C1");
        press(&mut s, "p");
        assert_eq!(tag_text(&s), "cursor old L3:C1");
    }

    #[test]
    fn f_visual_03_sel_tag() {
        let mut s = st();
        press(&mut s, "jV");
        assert_eq!(sel_tag(&s, &selection(&s).expect("s")), "L1");
        press(&mut s, "jj");
        assert_eq!(sel_tag(&s, &selection(&s).expect("s")), "L1-3");
        press(&mut s, "V");
        press(&mut s, "gv");
        assert_eq!(sel_tag(&s, &selection(&s).expect("s")), "r1:C1-C1");
        press(&mut s, "jjjl");
        assert_eq!(sel_tag(&s, &selection(&s).expect("s")), "r1:C1-L3:C2");
        press(&mut s, "V");
        assert_eq!(sel_tag(&s, &selection(&s).expect("s")), "r1-3");
    }

    #[test]
    fn f_cursor_10_esc_chain() {
        let mut s = st();
        press(&mut s, "jv");
        s.thread.picked_block = Some(("q1".into(), 0));
        s.nav.focused_comment = Some("q1".into());
        assert!(key(&mut s, Key::Esc));
        assert!(s.thread.picked_block.is_none());
        assert!(s.nav.focused_comment.is_some());
        key(&mut s, Key::Esc);
        assert!(s.nav.focused_comment.is_none());
        assert!(s.nav.selection.is_some());
        key(&mut s, Key::Esc);
        assert!(s.nav.selection.is_none());
        let before = s.nav.row;
        key(&mut s, Key::Esc);
        assert_eq!(s.nav.row, before);
    }
}
