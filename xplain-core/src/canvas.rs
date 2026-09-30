//! Drawing surface over [`Screen`]: clipped text, fills, boxes. All view modules draw through it.
//!
//! Spec: F-LAYOUT-01 (frame), F-LAYOUT-06 (modal placement), F-LAYOUT-07 (truncation with `…`), wide chars
//! (`Cell::width`). Owner: component `viewframe` (F1).
//! Must not: know State, themes or features.

use crate::screen::{Cell, Screen, Size, Style};
use crate::textutil::{cell_width, char_width};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
}

#[derive(Debug, Clone)]
pub struct Canvas {
    screen: Screen,
}

/// Cell width of one char as drawn: `None` for chars the canvas skips (control, zero width).
fn char_cells(ch: char) -> Option<u16> {
    match char_width(ch) {
        w @ 1..=2 => Some(w as u16),
        _ => None,
    }
}

impl Canvas {
    pub fn new(size: Size) -> Self {
        Canvas { screen: Screen::blank(size) }
    }

    pub fn size(&self) -> Size {
        self.screen.size
    }

    fn cell_at(&self, x: u16, y: u16) -> Option<&Cell> {
        self.screen.rows.get(y as usize).and_then(|r| r.get(x as usize))
    }

    fn cell_at_mut(&mut self, x: u16, y: u16) -> Option<&mut Cell> {
        self.screen.rows.get_mut(y as usize).and_then(|r| r.get_mut(x as usize))
    }

    /// Blank the other half of a wide char that a write at `x` is about to break.
    fn break_wide(&mut self, x: u16, y: u16) {
        let Some(cell) = self.cell_at(x, y).copied() else { return };
        if cell.width == 0 && x > 0 {
            if let Some(prev) = self.cell_at_mut(x - 1, y) {
                prev.ch = ' ';
                prev.width = 1;
            }
        } else if cell.width == 2 {
            if let Some(next) = self.cell_at_mut(x + 1, y) {
                next.ch = ' ';
                next.width = 1;
            }
        }
    }

    /// Write one char of `w` cells at (x, y); caller guarantees it fits.
    fn write_cell(&mut self, x: u16, y: u16, ch: char, w: u16, style: Style) {
        self.break_wide(x, y);
        if w == 2 {
            self.break_wide(x + 1, y);
        }
        if let Some(c) = self.cell_at_mut(x, y) {
            *c = Cell { ch, width: w as u8, style };
        }
        if w == 2 {
            if let Some(c) = self.cell_at_mut(x + 1, y) {
                *c = Cell { ch: ' ', width: 0, style };
            }
        }
    }

    /// Draw `text` at (x, y), clipped at the canvas edge (no ellipsis). Returns cells advanced.
    pub fn put(&mut self, x: u16, y: u16, text: &str, style: Style) -> u16 {
        let size = self.size();
        if y >= size.rows {
            return 0;
        }
        let mut adv = 0u16;
        for ch in text.chars() {
            let Some(w) = char_cells(ch) else { continue };
            if x.saturating_add(adv).saturating_add(w) > size.cols {
                break;
            }
            self.write_cell(x + adv, y, ch, w, style);
            adv += w;
        }
        adv
    }

    /// Draw `text` into a box of `width` cells; when wider, `width-1` cells + `…` (F-LAYOUT-07).
    pub fn put_trunc(&mut self, x: u16, y: u16, width: u16, text: &str, style: Style) -> u16 {
        self.put_segs_trunc(x, y, width, &[(text, style)])
    }

    /// Like [`Canvas::put_trunc`] over styled segments; the `…` takes the style of the segment it cuts.
    pub fn put_segs_trunc(&mut self, x: u16, y: u16, width: u16, segs: &[(&str, Style)]) -> u16 {
        let total: usize = segs.iter().map(|(t, _)| cell_width(t)).sum();
        if total <= usize::from(width) {
            let mut adv = 0;
            for (t, st) in segs {
                adv += self.put(x.saturating_add(adv), y, t, *st);
            }
            return adv;
        }
        if width == 0 {
            return 0;
        }
        let cut = width - 1;
        let mut used = 0u16;
        let mut last = segs.first().map(|s| s.1).unwrap_or_default();
        'outer: for (t, st) in segs {
            last = *st;
            for ch in t.chars() {
                let Some(w) = char_cells(ch) else { continue };
                if used + w > cut {
                    break 'outer;
                }
                self.put(x.saturating_add(used), y, ch.encode_utf8(&mut [0; 4]), *st);
                used += w;
            }
        }
        while used < cut {
            self.put(x.saturating_add(used), y, " ", last);
            used += 1;
        }
        self.put(x.saturating_add(cut), y, "\u{2026}", last);
        width
    }

    /// Set the style of `w` cells starting at (x, y) without changing chars (bg fills, highlights).
    pub fn paint(&mut self, x: u16, y: u16, w: u16, style: Style) {
        for dx in 0..w {
            match x.checked_add(dx) {
                Some(cx) => {
                    if let Some(c) = self.cell_at_mut(cx, y) {
                        c.style = style;
                    }
                }
                None => break,
            }
        }
    }

    /// Fill a rect with spaces in `style`.
    pub fn fill(&mut self, rect: Rect, style: Style) {
        for dy in 0..rect.h {
            for dx in 0..rect.w {
                let (Some(x), Some(y)) = (rect.x.checked_add(dx), rect.y.checked_add(dy)) else { continue };
                if self.cell_at(x, y).is_some() {
                    self.write_cell(x, y, ' ', 1, style);
                }
            }
        }
    }

    /// Bordered box (round border, as Ink `round`) filled with `fill`; returns the inner rect.
    pub fn draw_box(&mut self, rect: Rect, border: Style, fill: Style) -> Rect {
        if rect.w < 2 || rect.h < 2 {
            self.fill(rect, fill);
            return Rect { x: rect.x, y: rect.y, w: 0, h: 0 };
        }
        let inner = Rect { x: rect.x + 1, y: rect.y + 1, w: rect.w - 2, h: rect.h - 2 };
        self.fill(inner, fill);
        let (right, bottom) = (rect.x + rect.w - 1, rect.y + rect.h - 1);
        let put1 = |c: &mut Canvas, x: u16, y: u16, ch: &str| {
            c.put(x, y, ch, border);
        };
        put1(self, rect.x, rect.y, "\u{256d}");
        put1(self, right, rect.y, "\u{256e}");
        put1(self, rect.x, bottom, "\u{2570}");
        put1(self, right, bottom, "\u{256f}");
        for x in rect.x + 1..right {
            put1(self, x, rect.y, "\u{2500}");
            put1(self, x, bottom, "\u{2500}");
        }
        for y in rect.y + 1..bottom {
            put1(self, rect.x, y, "\u{2502}");
            put1(self, right, y, "\u{2502}");
        }
        inner
    }

    /// Horizontal rule of `─` (F-LAYOUT-01 row 2).
    pub fn hline(&mut self, x: u16, y: u16, w: u16, style: Style) {
        for dx in 0..w {
            if let Some(cx) = x.checked_add(dx) {
                self.put(cx, y, "\u{2500}", style);
            }
        }
    }

    pub fn into_screen(self) -> Screen {
        self.screen
    }
}

/// F-LAYOUT-06: top-left of a `w` x `h` box centered in `size` (extra cell/row goes before the box).
pub fn center(size: Size, w: u16, h: u16) -> (u16, u16) {
    let x = size.cols.saturating_sub(w).div_ceil(2);
    let y = size.rows.saturating_sub(h).div_ceil(2);
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screen::Color;

    fn cv(cols: u16, rows: u16) -> Canvas {
        Canvas::new(Size { cols, rows })
    }

    fn red() -> Style {
        Style { fg: Some(Color::Red), ..Style::default() }
    }

    #[test]
    fn put_clips_at_edge() {
        let mut c = cv(5, 1);
        assert_eq!(c.put(3, 0, "abcdef", Style::default()), 2);
        assert_eq!(c.into_screen().row_text(0), "   ab");
    }

    #[test]
    fn put_outside_is_noop() {
        let mut c = cv(5, 1);
        assert_eq!(c.put(0, 3, "x", Style::default()), 0);
        assert_eq!(c.put(9, 0, "x", Style::default()), 0);
    }

    #[test]
    fn wide_char_takes_two_cells() {
        let mut c = cv(6, 1);
        assert_eq!(c.put(1, 0, "a\u{4e2d}b", red()), 4);
        let s = c.into_screen();
        assert_eq!(s.rows[0][2].width, 2);
        assert_eq!(s.rows[0][3].width, 0);
        assert_eq!(s.rows[0][3].style, red());
        assert_eq!(s.row_text(0), " a\u{4e2d}b ");
    }

    #[test]
    fn wide_char_clipped_when_half_fits() {
        let mut c = cv(3, 1);
        assert_eq!(c.put(2, 0, "\u{4e2d}", Style::default()), 0);
        assert_eq!(c.into_screen().row_text(0), "   ");
    }

    #[test]
    fn overwriting_half_a_wide_char_blanks_the_rest() {
        let mut c = cv(4, 1);
        c.put(0, 0, "\u{4e2d}", Style::default());
        c.put(1, 0, "x", Style::default());
        let s = c.into_screen();
        assert_eq!(s.rows[0][0].ch, ' ');
        assert_eq!(s.rows[0][0].width, 1);
        assert_eq!(s.row_text(0), " x  ");
    }

    #[test]
    fn f_layout_07_trunc_ellipsis() {
        let mut c = cv(20, 1);
        assert_eq!(c.put_trunc(0, 0, 5, "abcdefgh", Style::default()), 5);
        assert_eq!(c.clone().into_screen().row_text(0).trim_end(), "abcd\u{2026}");
        let mut c2 = cv(20, 1);
        c2.put_trunc(0, 0, 5, "abcde", Style::default());
        assert_eq!(c2.into_screen().row_text(0).trim_end(), "abcde");
    }

    #[test]
    fn f_layout_07_trunc_wide_boundary_pads() {
        let mut c = cv(20, 1);
        c.put_trunc(0, 0, 4, "ab\u{4e2d}\u{4e2d}", Style::default());
        assert_eq!(c.into_screen().row_text(0).trim_end(), "ab \u{2026}");
    }

    #[test]
    fn trunc_width_zero_and_one() {
        let mut c = cv(5, 1);
        assert_eq!(c.put_trunc(0, 0, 0, "abc", Style::default()), 0);
        assert_eq!(c.put_trunc(0, 0, 1, "abc", Style::default()), 1);
        assert_eq!(c.into_screen().row_text(0), "\u{2026}    ");
    }

    #[test]
    fn segs_trunc_uses_cut_segment_style() {
        let mut c = cv(10, 1);
        c.put_segs_trunc(0, 0, 4, &[("ab", Style::default()), ("cdef", red())]);
        let s = c.into_screen();
        assert_eq!(s.row_text(0).trim_end(), "abc\u{2026}");
        assert_eq!(s.rows[0][3].style, red());
        assert_eq!(s.rows[0][0].style, Style::default());
    }

    #[test]
    fn paint_changes_style_only() {
        let mut c = cv(4, 1);
        c.put(0, 0, "abcd", Style::default());
        c.paint(1, 0, 2, red());
        let s = c.into_screen();
        assert_eq!(s.row_text(0), "abcd");
        assert_eq!(s.rows[0][0].style, Style::default());
        assert_eq!(s.rows[0][1].style, red());
        assert_eq!(s.rows[0][2].style, red());
        assert_eq!(s.rows[0][3].style, Style::default());
    }

    #[test]
    fn fill_and_box() {
        let mut c = cv(6, 4);
        let inner = c.draw_box(Rect { x: 1, y: 0, w: 4, h: 3 }, red(), Style::default());
        assert_eq!(inner, Rect { x: 2, y: 1, w: 2, h: 1 });
        let s = c.into_screen();
        assert_eq!(s.row_text(0), " \u{256d}\u{2500}\u{2500}\u{256e} ");
        assert_eq!(s.row_text(1), " \u{2502}  \u{2502} ");
        assert_eq!(s.row_text(2), " \u{2570}\u{2500}\u{2500}\u{256f} ");
        assert_eq!(s.rows[0][1].style, red());
        assert_eq!(s.rows[1][2].style, Style::default());
    }

    #[test]
    fn box_clipped_at_screen() {
        let mut c = cv(3, 2);
        c.draw_box(Rect { x: 1, y: 1, w: 5, h: 5 }, red(), Style::default());
        assert_eq!(c.into_screen().row_text(1), " \u{256d}\u{2500}");
    }

    #[test]
    fn hline_draws_rule() {
        let mut c = cv(5, 1);
        c.hline(0, 0, 4, red());
        assert_eq!(c.into_screen().row_text(0), "\u{2500}\u{2500}\u{2500}\u{2500} ");
    }

    #[test]
    fn f_layout_06_center_delete_modal() {
        assert_eq!(center(Size { cols: 80, rows: 24 }, 25, 3), (28, 11));
    }

    #[test]
    fn f_layout_06_center_odd_leftover_goes_before() {
        assert_eq!(center(Size { cols: 81, rows: 25 }, 20, 4), (31, 11));
        assert_eq!(center(Size { cols: 10, rows: 3 }, 20, 9), (0, 0));
    }
}
