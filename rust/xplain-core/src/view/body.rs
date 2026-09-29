//! Viewport body: unified, split and browse rows with gutters, marks, cursor, selection, find hits, syntax
//! highlighting (token classes read from the highlight cache, never parsed here), and the comment/editor boxes interleaved under rows.
//!
//! Spec: F-LAYOUT-03/04/05 (row formats, split panes, widths), F-LAYOUT-07 (truncation), F-CURSOR-02
//! (cursor row bg `curBg`, cursor cell `▶`, char cursor inverse), F-VISUAL-02 (selection colors), F-FIND-02
//! (hit colors `FIND_HIT_*`), F-EDGE-08 (content chars), F-EDGE-06 (no-newline marker row), F-THEME-02,
//! F-CURSOR-07 (x_shift), F-NAV-09 (what rows the window shows, blocks never cut at top).
//! Oracle: `src/components/DiffView.tsx`. Owner: component `viewrows` (F2).
//! Uses `rows::*`, `highlight::*`, `thread_layout::boxes_at` via `thread_box::draw_boxes`. Must not mutate state.
//!
//! Decisions on UNSPEC: cursor row is padded past the edge, last cell always `…` (UNSPEC-16), control
//! chars in content are drawn as U+FFFD (UNSPEC-26), columns count chars (UNSPEC-28).

use crate::canvas::{Canvas, Rect};
use crate::comments::PaneSide;
use crate::diff::LineKind;
use crate::highlight::{ClassRun, run_style};
use crate::hlcache::{HlCache, line_key};
use crate::nav::visual::Sel;
use crate::rows::{RowLine, ShownRow, find_all, pane_of};
use crate::screen::{Color, Style};
use crate::state::State;
use crate::textutil::{char_width, expand_tabs};
use crate::theme::{FIND_HIT_BG, FIND_HIT_FG, Theme, ThemeId};
use crate::thread_layout::boxes_at;
use crate::view::thread_box::draw_box;

/// Cursor cell glyph (F-CURSOR-02).
pub const CUR_MARK: char = '▶';

/// Everything `draw_rows` reads, gathered from state by [`draw_body`] (also lets tests skip `State`).
#[derive(Debug, Clone)]
pub struct BodyView<'a> {
    pub rows: &'a [ShownRow],
    /// First shown row (`nav.top`).
    pub top: usize,
    /// Cursor row (`nav.row`).
    pub cursor: usize,
    /// Shown (clamped) char column of the cursor.
    pub col: usize,
    /// Pane choice of the cursor (split).
    pub pane: PaneSide,
    pub sel: Option<Sel>,
    /// Horizontal code shift in chars (F-CURSOR-07).
    pub x_shift: usize,
    /// Active find term (F-FIND-02).
    pub find: Option<&'a str>,
    /// File path keying the highlight cache.
    pub path: &'a str,
    /// Highlight cache; `None` draws plain text.
    pub hl: Option<&'a HlCache>,
    pub theme_id: ThemeId,
    /// Browse: single number column (F-LAYOUT-05).
    pub single: bool,
}

/// Draw the viewport into `area` (rows 3..H+2 of the frame): rows from `nav.top` until the area is full.
pub fn draw_body(c: &mut Canvas, state: &State, theme: &Theme, area: Rect) {
    let path = match &state.browse {
        Some(b) => b.path.as_str(),
        None => state.files.get(state.nav.file_index).map(|f| f.path.as_str()).unwrap_or(""),
    };
    let view = BodyView {
        rows: &state.rows.rows,
        top: state.nav.top,
        cursor: state.nav.row,
        col: crate::nav::shown_col(state),
        pane: state.nav.pane.into(),
        sel: crate::nav::visual::selection(state),
        x_shift: state.nav.x_shift,
        find: state.find.term.as_deref().filter(|t| !t.is_empty()),
        path,
        hl: Some(&state.hl),
        theme_id: state.settings.theme,
        single: state.browse.is_some(),
    };
    let mut boxes = |c: &mut Canvas, ri: usize, y: u16, max_y: u16| -> u16 {
        let mut used = 0u16;
        for b in boxes_at(state, ri) {
            let at = y.saturating_add(used);
            if at >= max_y {
                break;
            }
            used = used.saturating_add(draw_box(c, state, theme, &b, at, max_y));
        }
        used
    };
    draw_rows(c, &view, theme, area, &mut boxes);
}

/// Width of the two number columns of unified rows for `n` (widens above 9999) (F-LAYOUT-03).
/// `OOOO NNNN` without the trailing space: `2 * max(4, digits) + 1`.
#[cfg(test)]
pub fn gutter_width(max_no: u32) -> u16 {
    let w = pad_width(max_no);
    (2 * w + 1) as u16
}

#[cfg(test)]
fn pad_width(n: u32) -> usize {
    n.to_string().len().max(4)
}

fn pad(n: Option<u32>) -> String {
    match n {
        Some(n) => format!("{n:>4}"),
        None => "    ".to_string(),
    }
}

/// State-free drawing loop. `boxes(canvas, row, y, max_y)` draws the boxes under row `row` starting at `y` and
/// returns the lines used (F-COMMENT-04). Rows are drawn from `view.top` until `area` is full; the last row's
/// boxes are clipped, never the row itself (F-NAV-09).
pub fn draw_rows(
    c: &mut Canvas,
    view: &BodyView<'_>,
    theme: &Theme,
    area: Rect,
    boxes: &mut dyn FnMut(&mut Canvas, usize, u16, u16) -> u16,
) {
    let max_y = area.y.saturating_add(area.h);
    let mut y = area.y;
    let mut ri = view.top;
    while ri < view.rows.len() && y < max_y {
        draw_row(c, view, theme, area, ri, y);
        y += 1;
        if y < max_y {
            y = y.saturating_add(boxes(c, ri, y, max_y));
        }
        ri += 1;
    }
}

type Cells = Vec<(char, Style)>;

fn sanitize(ch: char) -> char {
    if ch.is_control() { '\u{fffd}' } else { ch }
}

fn push_str(out: &mut Cells, s: &str, style: Style) {
    out.extend(s.chars().map(|ch| (ch, style)));
}

/// Put `cells` at (x, y) into `width` cells: cut with `…` in the last cell when wider (F-LAYOUT-07); `pad`
/// fills the rest with spaces of that style (cursor row bg).
fn emit(c: &mut Canvas, x: u16, y: u16, width: u16, cells: &Cells, pad: Option<Style>) {
    let width = width as usize;
    if width == 0 {
        return;
    }
    // Oracle: a cursor row text is followed by `width` spaces, so it always runs past the edge and ends in `…`
    // (UNSPEC-16). An empty cell only gets its bg.
    let widened: Cells;
    let cells = match pad {
        Some(p) if !cells.is_empty() => {
            widened = cells.iter().copied().chain(std::iter::repeat_n((' ', p), width)).collect();
            &widened
        }
        _ => cells,
    };
    let total: usize = cells.iter().map(|(ch, _)| char_width(*ch)).sum();
    let mut out: Cells = Vec::with_capacity(cells.len() + 1);
    let mut acc = 0usize;
    if total > width {
        let mut cut_style = None;
        for (ch, st) in cells {
            let w = char_width(*ch);
            if acc + w > width - 1 {
                cut_style = Some(*st);
                break;
            }
            acc += w;
            out.push((*ch, *st));
        }
        out.push(('…', cut_style.unwrap_or_default()));
        acc += 1;
    } else {
        out.extend(cells.iter().copied());
        acc = total;
    }
    if let Some(p) = pad {
        for _ in acc..width {
            out.push((' ', p));
        }
    }
    let mut cx = x;
    let mut run = String::new();
    let mut run_style = None;
    for (ch, st) in out {
        if run_style != Some(st) {
            if let Some(rs) = run_style {
                cx = cx.saturating_add(c.put(cx, y, &run, rs));
            }
            run.clear();
            run_style = Some(st);
        }
        run.push(ch);
    }
    if let Some(rs) = run_style {
        c.put(cx, y, &run, rs);
    }
}

/// What marks chars of one code text.
struct Paint<'a> {
    /// Token classes of the line (`None` = plain).
    runs: Option<&'a [ClassRun]>,
    theme_id: ThemeId,
    theme: &'a Theme,
    hoff: usize,
    /// Base bg (cursor row).
    bg: Option<Color>,
    /// Char cursor column when this pane carries it.
    cursor: Option<usize>,
    sel: Option<(usize, usize)>,
    hits: Vec<(usize, usize)>,
    /// Plain text (no syntax), e.g. the no-newline marker.
    plain: bool,
}

/// Code text as styled cells: syntax colors, then hit, selection, char cursor (precedence F-FIND-02).
fn code_cells(text: &str, p: &Paint<'_>) -> Cells {
    let mut chars: Vec<char> = text.chars().map(sanitize).collect();
    let n_text = chars.len();
    let need = p.cursor.map_or(0, |c| c + 1).max(p.sel.map_or(0, |s| s.1));
    while chars.len() < need {
        chars.push(' ');
    }
    let mut syn: Vec<Style> = Vec::with_capacity(chars.len());
    for r in p.runs.filter(|_| !p.plain).unwrap_or_default() {
        let (fg, bold) = run_style(p.theme_id, r.class);
        let st = Style { fg, bg: p.bg, bold, ..Style::default() };
        syn.extend(std::iter::repeat_n(st, r.len as usize));
    }
    syn.resize(chars.len(), Style { bg: p.bg, ..Style::default() });
    let base = Style { bg: p.bg, ..Style::default() };
    let mut out = Cells::with_capacity(chars.len());
    for (i, ch) in chars.iter().enumerate().skip(p.hoff) {
        let in_sel = p.sel.is_some_and(|(a, b)| i >= a && i < b);
        let in_hit = p.hits.iter().any(|(a, b)| i >= *a && i < *b) && i < n_text;
        let style = if p.cursor == Some(i) {
            if in_sel {
                Style { fg: Some(p.theme.vis_fg), bg: Some(p.theme.vis_bg), reverse: true, ..base }
            } else {
                Style { reverse: true, ..base }
            }
        } else if in_sel {
            Style { fg: Some(p.theme.vis_fg), bg: Some(p.theme.vis_bg), ..Style::default() }
        } else if in_hit {
            Style { fg: Some(FIND_HIT_FG), bg: Some(FIND_HIT_BG), ..Style::default() }
        } else {
            syn.get(i).copied().unwrap_or(base)
        };
        out.push((*ch, style));
    }
    out
}

/// Selection range `[from, to)` inside a row text of `n` chars (F-VISUAL-02): first row from anchor col, middle
/// rows full, last row to cursor col inclusive; line mode whole rows; empty row one cell.
fn sel_range(sel: &Sel, ri: usize, n: usize) -> Option<(usize, usize)> {
    if ri < sel.sr || ri > sel.er {
        return None;
    }
    if n == 0 {
        return Some((0, 1));
    }
    let a = if ri == sel.sr && !sel.line { sel.sc } else { 0 };
    let b = if ri == sel.er && !sel.line { sel.ec + 1 } else { n };
    Some((a.min(n), b.max(a + 1).min(n)))
}

struct LineLook {
    bg: Option<Color>,
    mark: char,
    fg: Color,
}

fn look(theme: &Theme, kind: LineKind) -> LineLook {
    match kind {
        LineKind::Add => LineLook { bg: Some(theme.add_bg), mark: '+', fg: theme.add_mark },
        LineKind::Del => LineLook { bg: Some(theme.del_bg), mark: '-', fg: theme.del_mark },
        LineKind::Context => LineLook { bg: None, mark: ' ', fg: theme.gutter },
    }
}

struct Ctx<'a> {
    view: &'a BodyView<'a>,
    theme: &'a Theme,
    ri: usize,
    is_cur: bool,
}

impl Ctx<'_> {
    fn cb(&self) -> Option<Color> {
        self.is_cur.then_some(self.theme.cur_bg)
    }

    /// Code cells of one line text; `active` = this pane carries char cursor and selection.
    fn code(&self, line: &RowLine, active: bool) -> Cells {
        let text = expand_tabs(&line.text);
        let n = text.chars().count();
        let hits = self.view.find.map(|t| find_all(&text, t)).unwrap_or_default();
        let sel = if active { self.view.sel.as_ref().and_then(|s| sel_range(s, self.ri, n)) } else { None };
        let runs =
            self.view.hl.zip(line_key(line)).and_then(|(h, (side, no))| h.runs(self.view.path, side, no));
        let p = Paint {
            runs,
            theme_id: self.view.theme_id,
            theme: self.theme,
            hoff: self.view.x_shift,
            bg: self.cb(),
            cursor: (self.is_cur && active).then_some(self.view.col),
            sel,
            hits,
            plain: line.no_newline_marker,
        };
        code_cells(&text, &p)
    }
}

fn draw_row(c: &mut Canvas, view: &BodyView<'_>, theme: &Theme, area: Rect, ri: usize, y: u16) {
    let Some(row) = view.rows.get(ri) else { return };
    let cx = Ctx { view, theme, ri, is_cur: ri == view.cursor };
    let cb = cx.cb();
    let pad_style = cb.map(|bg| Style { bg: Some(bg), ..Style::default() });
    match row {
        ShownRow::Hunk(t) | ShownRow::Note(t) => {
            let fg = if matches!(row, ShownRow::Hunk(_)) { theme.hunk } else { theme.dim };
            let st = Style { fg: Some(fg), bg: cb, ..Style::default() };
            let mut cells = Cells::new();
            push_str(&mut cells, &t.chars().map(sanitize).collect::<String>(), st);
            emit(c, area.x, y, area.w, &cells, pad_style);
        }
        ShownRow::Line(l) => {
            let lk = look(theme, l.kind);
            let bgc = cb.or(lk.bg);
            let gut = if view.single {
                format!("{} ", pad(l.new_no))
            } else {
                format!("{} {} ", pad(l.old_no), pad(l.new_no))
            };
            let mut cells = Cells::new();
            push_str(&mut cells, &gut, Style { fg: Some(theme.gutter), bg: bgc, ..Style::default() });
            let mk = Style { fg: Some(lk.fg), bg: bgc, bold: true, ..Style::default() };
            cells.push((lk.mark, mk));
            cells.push((if cx.is_cur { CUR_MARK } else { ' ' }, mk));
            cells.extend(cx.code(l, true));
            emit(c, area.x, y, area.w, &cells, pad_style);
        }
        ShownRow::Pair { l, r } => {
            let w = area.w.saturating_sub(1) / 2;
            let pane = pane_of(row, view.pane);
            let left = pane_cells(&cx, l.as_ref(), l.as_ref().and_then(|x| x.old_no), pane == PaneSide::Old);
            emit(c, area.x, y, w, &left, pad_style);
            c.put(area.x + w, y, "│", Style { fg: Some(theme.dim), bg: cb, ..Style::default() });
            let right = pane_cells(&cx, r.as_ref(), r.as_ref().and_then(|x| x.new_no), pane == PaneSide::New);
            emit(c, area.x + w + 1, y, w, &right, pad_style);
        }
    }
}

/// One split pane cell: `NNNN ` gutter, mark, cursor cell, code (F-LAYOUT-04, F-CURSOR-02).
fn pane_cells(cx: &Ctx<'_>, l: Option<&RowLine>, no: Option<u32>, active: bool) -> Cells {
    let cb = cx.cb();
    let mut cells = Cells::new();
    let Some(l) = l else {
        // empty cell: only the active one on the cursor row carries the char cursor block
        if cx.is_cur && active {
            push_str(&mut cells, "      ", Style { bg: cb, ..Style::default() });
            cells.push((CUR_MARK, Style { bg: cb, ..Style::default() }));
            let p = Paint {
                runs: None,
                theme_id: cx.view.theme_id,
                theme: cx.theme,
                hoff: 0,
                bg: cb,
                cursor: Some(cx.view.col.saturating_sub(cx.view.x_shift)),
                sel: None,
                hits: Vec::new(),
                plain: true,
            };
            cells.extend(code_cells("", &p));
        }
        return cells;
    };
    let lk = look(cx.theme, l.kind);
    let bgc = cb.or(lk.bg);
    push_str(
        &mut cells,
        &format!("{} ", pad(no)),
        Style { fg: Some(cx.theme.gutter), bg: bgc, ..Style::default() },
    );
    let mk = Style { fg: Some(lk.fg), bg: bgc, bold: true, ..Style::default() };
    cells.push((lk.mark, mk));
    cells.push((if cx.is_cur { CUR_MARK } else { ' ' }, mk));
    cells.extend(cx.code(l, active));
    cells
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screen::{Screen, Size};

    fn line(kind: LineKind, o: Option<u32>, n: Option<u32>, t: &str) -> RowLine {
        RowLine { kind, old_no: o, new_no: n, text: t.into(), no_newline_marker: false }
    }

    fn th() -> Theme {
        Theme::of(ThemeId::Solarized)
    }

    fn view<'a>(rows: &'a [ShownRow]) -> BodyView<'a> {
        BodyView {
            rows,
            top: 0,
            cursor: usize::MAX,
            col: 0,
            pane: PaneSide::New,
            sel: None,
            x_shift: 0,
            find: None,
            path: "a.txt",
            hl: None,
            theme_id: ThemeId::Solarized,
            single: false,
        }
    }

    fn render(v: &BodyView<'_>, cols: u16, h: u16) -> Screen {
        let mut c = Canvas::new(Size { cols, rows: h });
        let mut none = |_: &mut Canvas, _: usize, _: u16, _: u16| 0u16;
        draw_rows(&mut c, v, &th(), Rect { x: 0, y: 0, w: cols, h }, &mut none);
        c.into_screen()
    }

    fn unified_rows() -> Vec<ShownRow> {
        vec![
            ShownRow::Hunk("@@ -1,2 +1,2 @@".into()),
            ShownRow::Line(line(LineKind::Context, Some(1), Some(1), "a")),
            ShownRow::Line(line(LineKind::Del, Some(2), None, "b")),
            ShownRow::Line(line(LineKind::Add, None, Some(2), "B\tx")),
        ]
    }

    #[test]
    fn f_layout_03_unified_text_and_colors() {
        let rows = unified_rows();
        let s = render(&view(&rows), 40, 6);
        let t = th();
        assert_eq!(s.row_text(0).trim_end(), "@@ -1,2 +1,2 @@");
        assert_eq!(s.rows[0][0].style.fg, Some(t.hunk));
        assert_eq!(s.row_text(1).trim_end(), "   1    1   a");
        assert_eq!(s.row_text(2).trim_end(), "   2      - b");
        assert_eq!(s.row_text(3).trim_end(), "        2 + B  x");
        // gutter: number + mark cells carry the add/del bg, code has none
        assert_eq!(s.rows[3][0].style.bg, Some(t.add_bg));
        assert_eq!(s.rows[3][10].style.bg, Some(t.add_bg));
        assert_eq!(s.rows[3][10].style.fg, Some(t.add_mark));
        assert!(s.rows[3][10].style.bold);
        assert_eq!(s.rows[3][12].style.bg, None);
        assert_eq!(s.rows[2][0].style.bg, Some(t.del_bg));
        assert_eq!(s.rows[2][10].style.fg, Some(t.del_mark));
        assert_eq!(s.rows[1][0].style.bg, None);
        assert_eq!(s.rows[1][0].style.fg, Some(t.gutter));
    }

    #[test]
    fn unspec_31_syntax_colors_come_from_cached_classes_and_follow_theme() {
        use crate::highlight::ClassRun;
        use crate::theme::{SyntaxClass, syntax_color};
        let rows = vec![ShownRow::Line(line(LineKind::Context, Some(1), Some(1), "fn x"))];
        let mut cache = HlCache::default();
        let runs =
            vec![ClassRun { len: 2, class: Some(SyntaxClass::Keyword) }, ClassRun { len: 2, class: None }];
        cache.seed("a.rs", PaneSide::New, 1, vec![runs]);
        let mut v = view(&rows);
        v.path = "a.rs";
        // no cache: plain
        let s = render(&v, 40, 2);
        assert_eq!(s.rows[0][12].style.fg, None);
        v.hl = Some(&cache);
        for t in [ThemeId::Solarized, ThemeId::Vibrant] {
            v.theme_id = t;
            let s = render(&v, 40, 2);
            assert_eq!(s.rows[0][12].style.fg, syntax_color(t, SyntaxClass::Keyword));
            assert_eq!(s.rows[0][13].style.fg, syntax_color(t, SyntaxClass::Keyword));
            assert_eq!(s.rows[0][14].style.fg, None);
            assert_eq!(s.row_text(0).trim_end(), "   1    1   fn x");
        }
    }

    #[test]
    fn f_layout_03_numbers_widen_above_9999() {
        let rows = vec![ShownRow::Line(line(LineKind::Add, None, Some(12345), "x"))];
        let s = render(&view(&rows), 40, 2);
        assert_eq!(s.row_text(0).trim_end(), "     12345 + x");
        assert_eq!(gutter_width(9999), 9);
        assert_eq!(gutter_width(12345), 11);
    }

    #[test]
    fn f_layout_03_only_rows_from_top_until_full() {
        let rows = unified_rows();
        let mut v = view(&rows);
        v.top = 2;
        let s = render(&v, 40, 2);
        assert_eq!(s.row_text(0).trim_end(), "   2      - b");
        assert_eq!(s.row_text(1).trim_end(), "        2 + B  x");
        let s = render(&v, 40, 1);
        assert_eq!(s.row_text(0).trim_end(), "   2      - b");
    }

    #[test]
    fn f_layout_07_truncates_with_ellipsis() {
        let rows = vec![ShownRow::Line(line(LineKind::Context, Some(1), Some(1), "abcdefghijklmnop"))];
        let s = render(&view(&rows), 16, 1);
        assert_eq!(s.row_text(0), "   1    1   abc…");
        let s = render(&view(&rows), 30, 1);
        assert_eq!(s.row_text(0).trim_end(), "   1    1   abcdefghijklmnop");
    }

    #[test]
    fn f_edge_08_wide_chars_take_two_cells() {
        let rows = vec![ShownRow::Line(line(LineKind::Context, Some(1), Some(1), "日本語"))];
        let s = render(&view(&rows), 15, 1);
        // 11 gutter + 4 cells: two wide chars would need 4 cells but width-1 = 14 -> only one fits before …
        assert_eq!(s.row_text(0), "   1    1   日…");
    }

    #[test]
    fn f_cursor_02_row_bg_marker_and_char_cursor() {
        let rows = unified_rows();
        let mut v = view(&rows);
        v.cursor = 2;
        v.col = 0;
        let s = render(&v, 30, 5);
        let t = th();
        assert_eq!(s.row_text(2), format!("   2      -▶b{}…", " ".repeat(16)));
        assert!(s.rows[2].iter().all(|c| c.style.bg == Some(t.cur_bg)));
        assert!(s.rows[2][12].style.reverse, "char cursor");
        assert!(!s.rows[2][13].style.reverse);
        assert_eq!(s.rows[2][10].style.fg, Some(t.del_mark), "mark still shown");
        // other rows keep their look
        assert_eq!(s.rows[3][0].style.bg, Some(t.add_bg));
    }

    #[test]
    fn f_cursor_02_hunk_row_bg_only_no_char_cursor() {
        let rows = unified_rows();
        let mut v = view(&rows);
        v.cursor = 0;
        let s = render(&v, 30, 5);
        let t = th();
        assert!(s.rows[0].iter().all(|c| c.style.bg == Some(t.cur_bg) && !c.style.reverse));
        assert_eq!(s.rows[0][0].style.fg, Some(t.hunk));
    }

    #[test]
    fn f_cursor_02_empty_line_reverse_space() {
        let rows = vec![ShownRow::Line(line(LineKind::Context, Some(1), Some(1), ""))];
        let mut v = view(&rows);
        v.cursor = 0;
        let s = render(&v, 30, 1);
        assert!(s.rows[0][12].style.reverse);
        assert_eq!(s.rows[0][12].ch, ' ');
    }

    #[test]
    fn f_cursor_07_x_shift_shifts_code_only() {
        let rows = vec![ShownRow::Line(line(LineKind::Context, Some(1), Some(1), "abcdef"))];
        let mut v = view(&rows);
        v.x_shift = 2;
        let s = render(&v, 30, 1);
        assert_eq!(s.row_text(0).trim_end(), "   1    1   cdef");
    }

    #[test]
    fn f_visual_02_selection_colors_and_ranges() {
        let rows = vec![
            ShownRow::Line(line(LineKind::Context, Some(1), Some(1), "abcdef")),
            ShownRow::Line(line(LineKind::Context, Some(2), Some(2), "")),
            ShownRow::Line(line(LineKind::Context, Some(3), Some(3), "xyz")),
        ];
        let mut v = view(&rows);
        v.sel = Some(Sel { sr: 0, sc: 2, er: 2, ec: 0, line: false });
        v.cursor = 2;
        v.col = 0;
        let s = render(&v, 30, 3);
        let t = th();
        let sel = |y: usize, x: usize| {
            s.rows[y][x].style.bg == Some(t.vis_bg) && s.rows[y][x].style.fg == Some(t.vis_fg)
        };
        assert!(!sel(0, 12) && !sel(0, 13));
        assert!(sel(0, 14) && sel(0, 17));
        assert!(sel(1, 12), "empty row one cell");
        assert!(!sel(1, 13));
        // cursor char in selection: reverse with the same colors
        let cur = s.rows[2][12].style;
        assert!(cur.reverse && cur.bg == Some(t.vis_bg) && cur.fg == Some(t.vis_fg));
        assert!(!sel(2, 13));
    }

    #[test]
    fn f_visual_02_line_selection_whole_text() {
        let rows = vec![ShownRow::Line(line(LineKind::Context, Some(1), Some(1), "abc"))];
        let mut v = view(&rows);
        v.sel = Some(Sel { sr: 0, sc: 0, er: 0, ec: 0, line: true });
        let s = render(&v, 30, 1);
        let t = th();
        assert!((12..15).all(|x| s.rows[0][x].style.bg == Some(t.vis_bg)));
        assert_eq!(s.rows[0][15].style.bg, None);
    }

    #[test]
    fn f_find_02_hits_and_precedence() {
        let rows = vec![ShownRow::Line(line(LineKind::Context, Some(1), Some(1), "foo Foo foo"))];
        let mut v = view(&rows);
        v.find = Some("foo");
        let s = render(&v, 30, 1);
        let hit = |x: usize| {
            s.rows[0][x].style.bg == Some(FIND_HIT_BG) && s.rows[0][x].style.fg == Some(FIND_HIT_FG)
        };
        assert!(hit(12) && hit(14) && hit(20) && hit(22));
        assert!(!hit(15));
        // case sensitive with uppercase term
        v.find = Some("Foo");
        let s = render(&v, 30, 1);
        assert!(s.rows[0][12].style.bg != Some(FIND_HIT_BG));
        assert!(s.rows[0][16].style.bg == Some(FIND_HIT_BG));
        // cursor row: hit keeps yellow over curBg, char cursor wins on its cell, selection beats hit
        v.find = Some("foo");
        v.cursor = 0;
        v.col = 1;
        v.sel = Some(Sel { sr: 0, sc: 2, er: 0, ec: 2, line: false });
        let s = render(&v, 30, 1);
        let t = th();
        assert_eq!(s.rows[0][12].style.bg, Some(FIND_HIT_BG));
        assert!(s.rows[0][13].style.reverse && s.rows[0][13].style.bg != Some(FIND_HIT_BG));
        assert_eq!(s.rows[0][14].style.bg, Some(t.vis_bg));
        assert_eq!(s.rows[0][15].style.bg, Some(t.cur_bg));
    }

    fn split_rows() -> Vec<ShownRow> {
        vec![
            ShownRow::Hunk("@@ -1,2 +1,2 @@".into()),
            ShownRow::Pair {
                l: Some(line(LineKind::Context, Some(1), Some(1), "a")),
                r: Some(line(LineKind::Context, Some(1), Some(1), "a")),
            },
            ShownRow::Pair { l: Some(line(LineKind::Del, Some(2), None, "old")), r: None },
            ShownRow::Pair { l: None, r: Some(line(LineKind::Add, None, Some(2), "new")) },
        ]
    }

    #[test]
    fn f_layout_04_split_panes() {
        let rows = split_rows();
        let s = render(&view(&rows), 101, 4);
        let t = th();
        assert_eq!(&s.row_text(1)[..12], "   1   a    ");
        // pane width floor(100/2) = 50; separator at col 50, right pane from 51
        assert_eq!(s.rows[1][50].ch, '│');
        assert_eq!(s.rows[1][50].style.fg, Some(t.dim));
        let text: Vec<char> = s.row_text(1).chars().collect();
        assert_eq!(text[51..57].iter().collect::<String>(), "   1  ");
        let del: Vec<char> = s.row_text(2).chars().collect();
        assert_eq!(del[..10].iter().collect::<String>(), "   2 - old");
        assert_eq!(del[50], '│');
        assert_eq!(s.rows[2][0].style.bg, Some(t.del_bg));
        assert_eq!(s.rows[2][51].style.bg, None);
        let add: Vec<char> = s.row_text(3).chars().collect();
        assert_eq!(add[51..].iter().collect::<String>().trim_end(), "   2 + new");
        assert_eq!(s.rows[3][51].style.bg, Some(t.add_bg));
    }

    #[test]
    fn f_layout_04_pane_truncation_at_own_width() {
        let long = "x".repeat(80);
        let rows = vec![ShownRow::Pair {
            l: Some(line(LineKind::Context, Some(1), Some(1), &long)),
            r: Some(line(LineKind::Context, Some(1), Some(1), &long)),
        }];
        let s = render(&view(&rows), 101, 1);
        assert_eq!(s.rows[0][49].ch, '…');
        assert_eq!(s.rows[0][50].ch, '│');
        assert_eq!(s.rows[0][100].ch, '…');
    }

    #[test]
    fn f_layout_04_cursor_row_panes_end_in_ellipsis_other_rows_plain() {
        let rows = split_rows();
        let mut v = view(&rows);
        v.cursor = 1;
        let s = render(&v, 101, 4);
        assert_eq!(s.rows[1][49].ch, '…');
        assert_eq!(s.rows[1][100].ch, '…');
        assert_ne!(s.rows[0][49].ch, '…');
    }

    #[test]
    fn f_cursor_02_split_both_panes_bg_marker_in_active_only_char_cursor() {
        let rows = split_rows();
        let mut v = view(&rows);
        v.cursor = 1;
        v.col = 0;
        v.pane = PaneSide::New;
        let s = render(&v, 101, 4);
        let t = th();
        assert!(s.rows[1][..101].iter().all(|c| c.style.bg == Some(t.cur_bg)));
        assert_eq!(s.rows[1][6].ch, '▶');
        assert_eq!(s.rows[1][57].ch, '▶');
        assert!(!s.rows[1][7].style.reverse);
        assert!(s.rows[1][58].style.reverse);
    }

    #[test]
    fn f_cursor_02_split_empty_active_cell_marker_and_reverse_space() {
        let rows = split_rows();
        let mut v = view(&rows);
        v.cursor = 2; // del-only pair: cursor always in the left pane
        let s = render(&v, 101, 4);
        assert_eq!(s.rows[2][6].ch, '▶');
        assert!(s.rows[2][7].style.reverse);
        let mut v = view(&rows);
        v.cursor = 3;
        v.pane = PaneSide::Old; // add-only pair, left cell empty: pane_of stays old
        let s = render(&v, 101, 4);
        assert_eq!(s.row_text(3).chars().take(7).collect::<String>(), "      ▶");
        assert!(s.rows[3][7].style.reverse);
        assert_eq!(s.rows[3][7].ch, ' ');
    }

    #[test]
    fn f_layout_04_selection_only_in_active_pane() {
        let rows = split_rows();
        let mut v = view(&rows);
        v.cursor = 1;
        v.pane = PaneSide::New;
        v.sel = Some(Sel { sr: 1, sc: 0, er: 1, ec: 0, line: true });
        let s = render(&v, 101, 4);
        let t = th();
        assert_ne!(s.rows[1][7].style.bg, Some(t.vis_bg));
        assert_eq!(s.rows[1][58].style.bg, Some(t.vis_bg));
    }

    #[test]
    fn f_layout_05_browse_rows() {
        let rows = vec![
            ShownRow::Line(line(LineKind::Context, None, Some(1), "fn a() {}")),
            ShownRow::Line(line(LineKind::Context, None, Some(2), "")),
        ];
        let mut v = view(&rows);
        v.single = true;
        v.cursor = 0;
        v.path = "x.rs";
        let s = render(&v, 30, 2);
        assert_eq!(s.row_text(0), format!("   1  ▶fn a() {{}}{}…", " ".repeat(13)));
        assert_eq!(s.row_text(1).trim_end(), "   2");
        let t = th();
        assert_eq!(s.rows[0][0].style.bg, Some(t.cur_bg));
        assert_eq!(s.rows[1][0].style.bg, None);
        assert!(s.rows[0][7].style.reverse);
    }

    #[test]
    fn f_layout_03_note_row_dim() {
        let rows = vec![ShownRow::Note("Binary file".into())];
        let s = render(&view(&rows), 30, 2);
        assert_eq!(s.row_text(0).trim_end(), "Binary file");
        assert_eq!(s.rows[0][0].style.fg, Some(th().dim));
    }

    #[test]
    fn f_edge_06_no_newline_marker_row() {
        let mut l = line(LineKind::Del, Some(2), None, " No newline at end of file");
        l.no_newline_marker = true;
        let rows = vec![ShownRow::Line(l)];
        let s = render(&view(&rows), 40, 1);
        assert_eq!(s.row_text(0).trim_end(), "   2      -  No newline at end of file");
    }

    #[test]
    fn f_comment_04_boxes_interleaved_and_clipped() {
        let rows = unified_rows();
        let mut c = Canvas::new(Size { cols: 20, rows: 5 });
        let mut calls = Vec::new();
        let mut boxes = |c: &mut Canvas, ri: usize, y: u16, max_y: u16| -> u16 {
            calls.push((ri, y, max_y));
            if ri == 1 {
                c.put(0, y, "BOX1", Style::default());
                c.put(0, y + 1, "BOX2", Style::default());
                2
            } else {
                0
            }
        };
        draw_rows(&mut c, &view(&rows), &th(), Rect { x: 0, y: 0, w: 20, h: 5 }, &mut boxes);
        let s = c.into_screen();
        assert_eq!(s.row_text(1).trim_end(), "   1    1   a");
        assert_eq!(s.row_text(2).trim_end(), "BOX1");
        assert_eq!(s.row_text(3).trim_end(), "BOX2");
        assert_eq!(s.row_text(4).trim_end(), "   2      - b");
        assert_eq!(calls.iter().map(|c| c.0).collect::<Vec<_>>(), vec![0, 1]);
    }

    #[test]
    fn f_theme_02_syntax_colors_by_extension() {
        let rows =
            vec![ShownRow::Line(line(LineKind::Context, Some(1), Some(1), "fn main() { let x = \"s\"; }"))];
        let mut v = view(&rows);
        v.path = "src/a.rs";
        let s = render(&v, 60, 1);
        assert!(s.rows[0][11..].iter().any(|c| c.style.fg.is_some()));
        assert_eq!(s.row_text(0).trim_end(), "   1    1   fn main() { let x = \"s\"; }");
        v.path = "a.unknown";
        let s = render(&v, 60, 1);
        assert!(s.rows[0][12..].iter().all(|c| c.style.fg.is_none()));
    }

    #[test]
    fn f_edge_08_control_chars_do_not_reach_output() {
        let rows = vec![ShownRow::Line(line(LineKind::Context, Some(1), Some(1), "a\u{1b}[31mb"))];
        let s = render(&view(&rows), 30, 1);
        assert!(!s.row_text(0).contains('\u{1b}'));
    }
}
