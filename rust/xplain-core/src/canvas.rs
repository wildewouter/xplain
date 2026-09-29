//! Drawing surface over [`Screen`]: clipped text, fills, boxes. All view modules draw through it.
//!
//! Spec: F-LAYOUT-01 (frame), F-LAYOUT-06 (modal placement), F-LAYOUT-07 (truncation with `…`), wide chars
//! (`Cell::width`). Owner: component `viewframe` (F1).
//! Must not: know State, themes or features.

use crate::screen::{Screen, Size, Style};

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

impl Canvas {
    pub fn new(size: Size) -> Self {
        Canvas { screen: Screen::blank(size) }
    }

    pub fn size(&self) -> Size {
        self.screen.size
    }

    /// Draw `text` at (x, y), clipped at the canvas edge (no ellipsis). Returns cells advanced.
    pub fn put(&mut self, _x: u16, _y: u16, _text: &str, _style: Style) -> u16 {
        todo!("clipped text")
    }

    /// Draw `text` into a box of `width` cells; when wider, `width-1` cells + `…` (F-LAYOUT-07).
    pub fn put_trunc(&mut self, _x: u16, _y: u16, _width: u16, _text: &str, _style: Style) -> u16 {
        todo!("F-LAYOUT-07")
    }

    /// Set the style of `w` cells starting at (x, y) without changing chars (bg fills, highlights).
    pub fn paint(&mut self, _x: u16, _y: u16, _w: u16, _style: Style) {
        todo!("paint")
    }

    /// Fill a rect with spaces in `style`.
    pub fn fill(&mut self, _rect: Rect, _style: Style) {
        todo!("fill")
    }

    /// Bordered box (single-line box drawing chars) filled with `bg` style; returns the inner rect.
    pub fn draw_box(&mut self, _rect: Rect, _border: Style, _fill: Style) -> Rect {
        todo!("modal box")
    }

    /// Horizontal rule of `─` (F-LAYOUT-01 row 2).
    pub fn hline(&mut self, _x: u16, _y: u16, _w: u16, _style: Style) {
        todo!("rule")
    }

    pub fn into_screen(self) -> Screen {
        self.screen
    }
}

/// F-LAYOUT-06: top-left of a `w` x `h` box centered in `size` (extra cell/row goes before the box).
pub fn center(_size: Size, _w: u16, _h: u16) -> (u16, u16) {
    todo!("F-LAYOUT-06")
}
