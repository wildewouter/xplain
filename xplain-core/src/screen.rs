//! Cell-grid output of `view`: what the terminal should show. Terminal independent.
//!
//! Spec: F-LAYOUT-01 (frame), F-LAYOUT-07 (truncation), Colors. Owner: core lead (types frozen).
//! Must not: contain escape sequences or crossterm/ratatui types. The runtime presenter turns a
//! [`Screen`] into terminal output (diff against the previous screen).

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Size {
    pub cols: u16,
    pub rows: u16,
}

impl Size {
    /// Defaults when the terminal size is unknown (F-LAYOUT-01): 80x24.
    pub const DEFAULT: Size = Size { cols: 80, rows: 24 };
}

/// Named ANSI colors from the spec plus 24-bit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Color {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    Gray,
    RedBright,
    GreenBright,
    Rgb(u8, u8, u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Style {
    /// `None` = terminal default.
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub reverse: bool,
}

/// Styled text run: what header and modal lines are made of.
pub type Seg = (String, Style);

pub fn seg(text: impl Into<String>, style: Style) -> Seg {
    (text.into(), style)
}

/// Foreground-only style.
pub fn fg(c: Color) -> Style {
    Style { fg: Some(c), ..Style::default() }
}

pub fn bold(s: Style) -> Style {
    Style { bold: true, ..s }
}

/// One terminal cell. Wide (2-cell) chars: first cell `width == 2`, next cell `width == 0` (continuation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub width: u8,
    pub style: Style,
}

impl Default for Cell {
    fn default() -> Self {
        Cell { ch: ' ', width: 1, style: Style::default() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Screen {
    pub size: Size,
    /// `rows[y][x]`, exactly `size.rows` rows of `size.cols` cells.
    pub rows: Vec<Vec<Cell>>,
}

impl Screen {
    pub fn blank(size: Size) -> Self {
        Screen { size, rows: vec![vec![Cell::default(); size.cols as usize]; size.rows as usize] }
    }

    /// Plain text of row `y` (for unit tests and the parity debugging).
    pub fn row_text(&self, y: usize) -> String {
        self.rows.get(y).map(|r| r.iter().filter(|c| c.width > 0).map(|c| c.ch).collect()).unwrap_or_default()
    }
}
