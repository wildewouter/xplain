//! Viewport: vertical `top`, horizontal `x_shift`, follow rules.
//!
//! Spec: F-NAV-09 (follow, margin m, comment boxes counted), F-NAV-10 (top reset keeps cursor),
//! F-CURSOR-07 (horizontal follow), F-LAYOUT-01 (H). Oracle: `fitOff`/`hoff` logic in `src/app.tsx`.
//! Owner: component `nav` (B). State only; never renders. Uses `thread_layout::row_extra_height`.

use crate::state::{Overlay, State};

use super::{body_height, cursor_row, last_row, shown_col};

/// Rows in the viewport (H).
pub fn height(state: &State) -> usize {
    body_height(state.size)
}

/// Lines of row `i` including comment/editor boxes under it.
fn row_h(state: &State, i: usize) -> usize {
    let boxes = !state.comments.is_empty() || matches!(state.overlay, Overlay::Editor(_));
    1 + if boxes { crate::thread_layout::row_extra_height(state, i) } else { 0 }
}

/// Smallest top showing everything up to `upto` within `h` lines (`fitOff`).
fn fit_off(state: &State, h: usize, upto: usize) -> usize {
    let mut o = upto;
    let mut used = row_h(state, o);
    while o > 0 && used + row_h(state, o - 1) <= h {
        o -= 1;
        used += row_h(state, o);
    }
    o
}

/// Largest legal top (whole blocks; the last row is reachable at the bottom).
fn max_top(state: &State) -> usize {
    if state.rows.rows.is_empty() { 0 } else { fit_off(state, height(state), last_row(state)) }
}

/// After a cursor change: apply F-NAV-09 (and F-CURSOR-07 horizontal follow). Editor open -> margin 0.
pub fn follow(state: &mut State) {
    follow_vertical(state);
    follow_horizontal(state);
}

fn follow_vertical(state: &mut State) {
    if state.rows.rows.is_empty() {
        state.nav.top = 0;
        return;
    }
    let h = height(state);
    let m = if matches!(state.overlay, Overlay::Editor(_)) { 0 } else { ((h - 1) / 2).min(2) };
    let cur = cursor_row(state);
    let last = last_row(state);
    let mut o = state.nav.top;
    if cur < o + m {
        o = cur.saturating_sub(m);
    }
    let end = last.min(cur + m);
    let mut need: usize = (o..=end).map(|k| row_h(state, k)).sum();
    while need > h && o < cur {
        need -= row_h(state, o);
        o += 1;
    }
    state.nav.top = o.min(fit_off(state, h, last));
}

/// Width of the code area (`cw`).
fn code_width(state: &State) -> usize {
    let cols = usize::from(state.size.cols);
    let w = if state.browse.is_some() {
        cols.saturating_sub(7)
    } else if crate::rows::effective_split(state) {
        (cols.saturating_sub(1) / 2).saturating_sub(7)
    } else {
        cols.saturating_sub(12)
    };
    w.max(1)
}

fn follow_horizontal(state: &mut State) {
    let cw = code_width(state);
    let m = ((cw - 1) / 2).min(4);
    let col = shown_col(state);
    let x = state.nav.x_shift;
    state.nav.x_shift = if col < x + m {
        col.saturating_sub(m)
    } else if col + m + 1 > x + cw {
        col + m + 1 - cw
    } else {
        x
    };
}

/// F-NAV-10: top = 0 then follow, cursor untouched.
pub fn reset_top_keep_cursor(state: &mut State) {
    state.nav.top = 0;
    follow(state);
}

/// Clamp `top` to the bottom offset (whole comment blocks; never a partial block at top).
pub fn clamp_top(state: &mut State) {
    state.nav.top = state.nav.top.min(max_top(state));
}

/// Scroll window text for the footer `(a-b/n)` (F-LAYOUT-02): returns `(a, b, n)`.
pub fn window(state: &State) -> (usize, usize, usize) {
    let n = state.rows.rows.len();
    let off = state.nav.top;
    (n.min(off + 1), n.min(off + height(state)), n)
}

#[cfg(test)]
mod tests {
    use super::super::testutil::*;
    use super::*;

    fn st(n: usize, rows: u16) -> State {
        let l: Vec<String> = (0..n).map(|i| format!("l{i}")).collect();
        let r: Vec<&str> = l.iter().map(String::as_str).collect();
        state_with_lines(80, rows, &r)
    }

    #[test]
    fn f_layout_01_height() {
        assert_eq!(body_height(crate::screen::Size { cols: 80, rows: 24 }), 21);
        assert_eq!(body_height(crate::screen::Size { cols: 80, rows: 5 }), 3);
        assert_eq!(body_height(crate::screen::Size { cols: 80, rows: 1 }), 3);
    }

    #[test]
    fn f_nav_09_example_20j_then_19k() {
        // 62 rows: hunk + 61 lines
        let mut s = st(61, 24);
        assert_eq!(s.rows.rows.len(), 62);
        press(&mut s, "20j");
        assert_eq!(s.nav.row, 20);
        assert_eq!(window(&s), (3, 23, 62));
        press(&mut s, "19k");
        assert_eq!(s.nav.row, 1);
        assert_eq!(window(&s), (1, 21, 62));
    }

    #[test]
    fn f_nav_09_margin_small_viewport() {
        // R=8 -> H=5, m=2
        let mut s = st(40, 8);
        press(&mut s, "10j");
        assert_eq!(s.nav.top, 8); // cursor 10, top such that 10 is m rows above bottom (rows 8..=12)
        press(&mut s, "k");
        assert_eq!(s.nav.top, 7);
    }

    #[test]
    fn f_nav_09_bottom_clamped() {
        let mut s = st(40, 8);
        press(&mut s, "G");
        assert_eq!(window(&s), (37, 41, 41));
    }

    #[test]
    fn f_nav_10_top_reset_keeps_cursor() {
        // 62 rows, R=12 (H=9, m=2), cursor row 52
        let mut s = st(61, 12);
        press(&mut s, "51j");
        reset_top_keep_cursor(&mut s);
        assert_eq!(s.nav.row, 51);
        assert_eq!(window(&s), (46, 54, 62));
    }

    #[test]
    fn f_nav_10_short_cursor_top_zero() {
        let mut s = st(61, 12);
        press(&mut s, "20j");
        s.nav.row = 5;
        s.nav.top = 3;
        reset_top_keep_cursor(&mut s);
        assert_eq!(s.nav.top, 0);
    }

    #[test]
    fn f_layout_02_window_empty() {
        let s = state_with_files(80, 24, vec![]);
        assert_eq!(window(&s), (0, 0, 0));
    }

    #[test]
    fn f_cursor_07_horizontal_follow() {
        // 80 cols: cw 68, m 4; `$` on a 100 char line -> col 99, shift 36
        let long = "x".repeat(100);
        let mut s = state_with_lines(80, 24, &[&long]);
        press(&mut s, "j$");
        assert_eq!(shown_col(&s), 99);
        assert_eq!(s.nav.x_shift, 36);
        press(&mut s, "0");
        assert_eq!(s.nav.x_shift, 0);
    }

    #[test]
    fn f_cursor_07_shift_kept_inside_margin() {
        let long = "x".repeat(100);
        let mut s = state_with_lines(80, 24, &[&long]);
        press(&mut s, "j$");
        press(&mut s, "10h");
        assert_eq!(s.nav.x_shift, 36);
        press(&mut s, "50h");
        assert_eq!(shown_col(&s), 39);
        assert_eq!(s.nav.x_shift, 35);
    }

    #[test]
    fn f_nav_09_clamp_top_after_shrink() {
        let mut s = st(61, 24);
        s.nav.top = 50;
        clamp_top(&mut s);
        assert_eq!(s.nav.top, 62 - 21);
    }

    #[test]
    fn f_nav_09_no_rows_top_zero() {
        let mut s = state_with_files(80, 24, vec![]);
        s.nav.top = 4;
        follow(&mut s);
        assert_eq!(s.nav.top, 0);
    }
}
