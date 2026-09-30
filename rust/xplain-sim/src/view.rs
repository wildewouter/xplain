//! Read side of a rendered [`Screen`]: rows as text, cells with comparable colors, text search.

use xplain_core::screen::{Cell, Color, Screen};

/// One cell, decoded for assertions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellView {
    pub ch: char,
    /// `None` = terminal default.
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub reverse: bool,
}

impl From<&Cell> for CellView {
    fn from(c: &Cell) -> Self {
        CellView {
            ch: c.ch,
            fg: c.style.fg,
            bg: c.style.bg,
            bold: c.style.bold,
            dim: c.style.dim,
            italic: c.style.italic,
            underline: c.style.underline,
            reverse: c.style.reverse,
        }
    }
}

/// Palette index of a named ANSI color (`Rgb` has none).
fn palette_index(c: Color) -> Option<u8> {
    Some(match c {
        Color::Black => 0,
        Color::Red => 1,
        Color::Green => 2,
        Color::Yellow => 3,
        Color::Blue => 4,
        Color::Magenta => 5,
        Color::Cyan => 6,
        Color::White => 7,
        Color::Gray => 8,
        Color::RedBright => 9,
        Color::GreenBright => 10,
        Color::Rgb(..) => return None,
    })
}

fn name_of(c: Color) -> &'static str {
    match c {
        Color::Black => "black",
        Color::Red => "red",
        Color::Green => "green",
        Color::Yellow => "yellow",
        Color::Blue => "blue",
        Color::Magenta => "magenta",
        Color::Cyan => "cyan",
        Color::White => "white",
        Color::Gray => "gray",
        Color::RedBright => "red-bright",
        Color::GreenBright => "green-bright",
        Color::Rgb(..) => "rgb",
    }
}

/// Human form: `#rrggbb`, palette name, or `default`.
pub fn color_name(c: Option<Color>) -> String {
    match c {
        None => "default".into(),
        Some(Color::Rgb(r, g, b)) => format!("#{r:02x}{g:02x}{b:02x}"),
        Some(c) => name_of(c).into(),
    }
}

/// Does `c` equal `spec`? Spec: `default`, `#rrggbb` (any case), a palette name (`red`, `gray`, `red-bright`,
/// `green-bright`, ...) or a palette index as decimal text (`"1"`). Panics on a malformed spec.
pub fn color_is(c: Option<Color>, spec: &str) -> bool {
    let s = spec.trim().to_ascii_lowercase();
    if s == "default" {
        return c.is_none();
    }
    if let Some(hex) = s.strip_prefix('#') {
        assert!(hex.len() == 6 && hex.bytes().all(|b| b.is_ascii_hexdigit()), "bad color spec {spec:?}");
        let n = u32::from_str_radix(hex, 16).unwrap_or(0);
        return c == Some(Color::Rgb((n >> 16) as u8, (n >> 8) as u8, n as u8));
    }
    let Some(c) = c else { return false };
    if let Ok(i) = s.parse::<u8>() {
        return palette_index(c) == Some(i);
    }
    let norm = s.replace(['_', ' '], "-");
    let norm = if norm == "grey" { "gray".to_string() } else { norm };
    assert!(
        [
            "black",
            "red",
            "green",
            "yellow",
            "blue",
            "magenta",
            "cyan",
            "white",
            "gray",
            "red-bright",
            "green-bright"
        ]
        .contains(&norm.as_str()),
        "bad color spec {spec:?}"
    );
    name_of(c) == norm
}

impl CellView {
    pub fn fg_is(&self, spec: &str) -> bool {
        color_is(self.fg, spec)
    }
    pub fn bg_is(&self, spec: &str) -> bool {
        color_is(self.bg, spec)
    }
}

/// Expected properties of a cell; unset fields are not checked. Colors as in [`color_is`].
#[derive(Debug, Clone, Default)]
pub struct CellExpect {
    pub ch: Option<char>,
    pub fg: Option<String>,
    pub bg: Option<String>,
    pub bold: Option<bool>,
    pub dim: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub reverse: Option<bool>,
}

impl CellExpect {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn ch(mut self, c: char) -> Self {
        self.ch = Some(c);
        self
    }
    pub fn fg(mut self, spec: &str) -> Self {
        self.fg = Some(spec.into());
        self
    }
    pub fn bg(mut self, spec: &str) -> Self {
        self.bg = Some(spec.into());
        self
    }
    pub fn bold(mut self, v: bool) -> Self {
        self.bold = Some(v);
        self
    }
    pub fn dim(mut self, v: bool) -> Self {
        self.dim = Some(v);
        self
    }
    pub fn italic(mut self, v: bool) -> Self {
        self.italic = Some(v);
        self
    }
    pub fn underline(mut self, v: bool) -> Self {
        self.underline = Some(v);
        self
    }
    pub fn reverse(mut self, v: bool) -> Self {
        self.reverse = Some(v);
        self
    }

    /// Mismatches of `cell` against this expectation, one line each (empty = match).
    pub fn diff(&self, cell: &CellView) -> Vec<String> {
        let mut d = Vec::new();
        if let Some(c) = self.ch.filter(|c| *c != cell.ch) {
            d.push(format!("char: want {c:?}, got {:?}", cell.ch));
        }
        if let Some(s) = self.fg.as_deref().filter(|s| !cell.fg_is(s)) {
            d.push(format!("fg: want {s}, got {}", color_name(cell.fg)));
        }
        if let Some(s) = self.bg.as_deref().filter(|s| !cell.bg_is(s)) {
            d.push(format!("bg: want {s}, got {}", color_name(cell.bg)));
        }
        for (name, want, got) in [
            ("bold", self.bold, cell.bold),
            ("dim", self.dim, cell.dim),
            ("italic", self.italic, cell.italic),
            ("underline", self.underline, cell.underline),
            ("reverse", self.reverse, cell.reverse),
        ] {
            if let Some(w) = want.filter(|w| *w != got) {
                d.push(format!("{name}: want {w}, got {got}"));
            }
        }
        d
    }
}

/// Position of a cell: `x` = column, `y` = row (0-based).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pos {
    pub x: usize,
    pub y: usize,
}

impl Pos {
    /// `n` columns to the right.
    pub fn right(self, n: usize) -> Pos {
        Pos { x: self.x + n, y: self.y }
    }
}

/// Row `y` as text (continuation cells of wide chars skipped), right-trimmed like the e2e screen dump.
pub fn row_text(s: &Screen, y: usize) -> String {
    s.row_text(y).trim_end().to_string()
}

/// All rows, right-trimmed.
pub fn rows(s: &Screen) -> Vec<String> {
    (0..s.rows.len()).map(|y| row_text(s, y)).collect()
}

/// Full-screen dump with row numbers, for failure messages.
pub fn dump(s: &Screen) -> String {
    let mut out = format!("--- screen {}x{} ---\n", s.size.cols, s.size.rows);
    for (y, r) in rows(s).iter().enumerate() {
        out.push_str(&format!("{y:>3}|{r}\n"));
    }
    out.push_str("--- end ---");
    out
}

/// Resolve a possibly negative row (`-1` = last).
pub fn resolve_row(s: &Screen, row: isize) -> usize {
    let n = s.rows.len() as isize;
    let y = if row < 0 { n + row } else { row };
    assert!((0..n).contains(&y), "row {row} outside screen with {n} rows");
    y as usize
}

/// Cell at column `x`, row `y` (negative counts from the bottom).
pub fn cell(s: &Screen, x: usize, y: isize) -> CellView {
    let y = resolve_row(s, y);
    let Some(c) = s.rows[y].get(x) else { panic!("column {x} outside screen with {} columns", s.size.cols) };
    CellView::from(c)
}

/// The `nth` (0-based) occurrence of `text` within row `y`, as the column of its first char.
pub fn find_in_row(s: &Screen, y: usize, text: &str, nth: usize) -> Option<usize> {
    let cells: Vec<(usize, char)> =
        s.rows[y].iter().enumerate().filter(|(_, c)| c.width > 0).map(|(x, c)| (x, c.ch)).collect();
    let want: Vec<char> = text.chars().collect();
    if want.is_empty() {
        return None;
    }
    let mut seen = 0;
    for i in 0..cells.len().saturating_sub(want.len() - 1) {
        if cells[i..i + want.len()].iter().map(|(_, c)| *c).eq(want.iter().copied()) {
            if seen == nth {
                return Some(cells[i].0);
            }
            seen += 1;
        }
    }
    None
}

/// The `nth` occurrence of `text` scanning rows top to bottom, then left to right.
pub fn find(s: &Screen, text: &str, nth: usize) -> Option<Pos> {
    let mut left = nth;
    for y in 0..s.rows.len() {
        let mut k = 0;
        while let Some(x) = find_in_row(s, y, text, k) {
            if left == 0 {
                return Some(Pos { x, y });
            }
            left -= 1;
            k += 1;
        }
    }
    None
}
