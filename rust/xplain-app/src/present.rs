//! Terminal presenter: alternate screen, raw mode, size, and drawing a core `Screen`.
//!
//! Spec: F-CLI-05 (alt screen enter/leave, raw mode, exit), F-LAYOUT-01 (size, default 80x24), Colors
//! (24-bit when `COLORTERM=truecolor`), Test seams ("frame fully written" = flushed).
//! Owner: component A (runtime).
//! Must not: interpret app state; it only turns a `Screen` into bytes (diffing against the previous frame
//! is allowed). ratatui may be used purely as buffer/backend here.

use std::io::Write;

use xplain_core::screen::{Screen, Size};

pub struct Presenter {
    prev: Option<Screen>,
}

/// Pure encoder: bytes that turn the terminal showing `prev` (or an unknown/blank one when `None`) into
/// `screen` (cursor hidden, full repaint when `prev` is `None` or the size differs; else only changed rows/
/// cells). SGR from `Style` (24-bit `38;2;r;g;b` when `truecolor`, else nearest 256/16 color), wide cells
/// skip their continuation cell. Colors table: SPEC "Colors". Unit-testable byte for byte.
pub fn encode_frame(_prev: Option<&Screen>, _screen: &Screen, _truecolor: bool) -> Vec<u8> {
    todo!("Colors, diff")
}

impl Presenter {
    /// Enter alternate screen + raw mode (only if stdin is a TTY for raw mode; stdin non-TTY: keys ignored,
    /// UI still renders).
    /// Writes `ESC[?1049h`; uses `crate::term::RawMode` for raw mode.
    pub fn enter(_out: &mut dyn Write) -> std::io::Result<Self> {
        Ok(Presenter { prev: None })
    }

    /// Leave alternate screen, restore terminal. Idempotent; also called on panic/exit paths.
    pub fn leave(&mut self, _out: &mut dyn Write) -> std::io::Result<()> {
        todo!("ESC[?1049l, show cursor, disable raw mode")
    }

    /// Current terminal size (80x24 when unknown).
    pub fn size() -> Size {
        crate::term::size()
    }

    /// Draw `screen` and flush before returning (the barrier reply is written after this returns).
    pub fn draw(&mut self, _screen: &Screen, _out: &mut dyn Write) -> std::io::Result<()> {
        let _ = &self.prev;
        todo!("diff + write escape sequences")
    }
}
