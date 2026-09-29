//! Terminal presenter: alternate screen, raw mode, size, and drawing a core `Screen`.
//!
//! Spec: F-CLI-05 (alt screen enter/leave, raw mode, exit), F-LAYOUT-01 (size, default 80x24), Colors
//! (24-bit when `COLORTERM=truecolor`), Test seams ("frame fully written" = flushed).
//! Owner: component A (runtime).
//! Must not: interpret app state; it only turns a `Screen` into bytes (diffing against the previous frame
//! is allowed). ratatui may be used purely as buffer/backend here.

use std::io::Write;

use xplain_core::screen::{Cell, Color, Screen, Size, Style};

use crate::term::RawMode;

pub struct Presenter {
    prev: Option<Screen>,
    truecolor: bool,
    raw: Option<RawMode>,
    entered: bool,
}

fn push_num(out: &mut Vec<u8>, n: u32) {
    out.extend_from_slice(n.to_string().as_bytes());
}

fn move_to(out: &mut Vec<u8>, row: usize, col: usize) {
    out.extend_from_slice(b"\x1b[");
    push_num(out, row as u32 + 1);
    out.push(b';');
    push_num(out, col as u32 + 1);
    out.push(b'H');
}

/// xterm 256-color index nearest to an RGB value (cube or gray ramp).
fn rgb_to_256(r: u8, g: u8, b: u8) -> u8 {
    if r == g && g == b {
        if r < 8 {
            return 16;
        }
        if r > 248 {
            return 231;
        }
        return (((r as f32 - 8.0) / 247.0) * 24.0).round() as u8 + 232;
    }
    let c = |v: u8| ((v as f32 / 255.0) * 5.0).round() as u8;
    16 + 36 * c(r) + 6 * c(g) + c(b)
}

/// SGR parameter list for a color; `bg` selects the background variant.
fn color_params(c: Color, bg: bool, truecolor: bool) -> String {
    let off = if bg { 10 } else { 0 };
    let named = |n: u32| (n + off).to_string();
    match c {
        Color::Black => named(30),
        Color::Red => named(31),
        Color::Green => named(32),
        Color::Yellow => named(33),
        Color::Blue => named(34),
        Color::Magenta => named(35),
        Color::Cyan => named(36),
        Color::White => named(37),
        Color::Gray => named(90),
        Color::RedBright => named(91),
        Color::GreenBright => named(92),
        Color::Rgb(r, g, b) => {
            let base = if bg { 48 } else { 38 };
            if truecolor {
                format!("{base};2;{r};{g};{b}")
            } else {
                format!("{base};5;{}", rgb_to_256(r, g, b))
            }
        }
    }
}

fn write_sgr(out: &mut Vec<u8>, style: &Style, truecolor: bool) {
    let mut p = String::from("0");
    let flags = [
        (style.bold, "1"),
        (style.dim, "2"),
        (style.italic, "3"),
        (style.underline, "4"),
        (style.reverse, "7"),
    ];
    for (on, code) in flags {
        if on {
            p.push(';');
            p.push_str(code);
        }
    }
    if let Some(c) = style.fg {
        p.push(';');
        p.push_str(&color_params(c, false, truecolor));
    }
    if let Some(c) = style.bg {
        p.push(';');
        p.push_str(&color_params(c, true, truecolor));
    }
    out.extend_from_slice(b"\x1b[");
    out.extend_from_slice(p.as_bytes());
    out.push(b'm');
}

/// Pure encoder: bytes that turn the terminal showing `prev` (or an unknown/blank one when `None`) into
/// `screen` (cursor hidden, full repaint when `prev` is `None` or the size differs; else only changed rows/
/// cells). SGR from `Style` (24-bit `38;2;r;g;b` when `truecolor`, else nearest 256/16 color), wide cells
/// skip their continuation cell. Colors table: SPEC "Colors". Unit-testable byte for byte.
pub fn encode_frame(prev: Option<&Screen>, screen: &Screen, truecolor: bool) -> Vec<u8> {
    let prev = prev.filter(|p| p.size == screen.size);
    let mut out = Vec::new();
    // Every frame ends with default style selected, so each one starts from it.
    let mut cur_style = Some(Style::default());
    if prev.is_none() {
        out.extend_from_slice(b"\x1b[?25l\x1b[0m\x1b[2J");
    }
    let blank = Cell::default();
    for (y, row) in screen.rows.iter().enumerate() {
        let prev_row = prev.and_then(|p| p.rows.get(y));
        if prev_row == Some(row) {
            continue;
        }
        let mut cursor: Option<usize> = None;
        let mut x = 0;
        while x < row.len() {
            let mut cell = row[x];
            let mut cx = x;
            let changed = match prev_row.and_then(|r| r.get(x)) {
                Some(pc) => *pc != cell,
                // Full repaint: the clear already blanked the screen.
                None => cell != blank,
            };
            if !changed {
                x += 1;
                continue;
            }
            if cell.width == 0 && x > 0 {
                // Continuation of a wide cell: repaint from its lead.
                cx = x - 1;
                cell = row[cx];
            }
            if cursor != Some(cx) {
                move_to(&mut out, y, cx);
            }
            if cur_style != Some(cell.style) {
                write_sgr(&mut out, &cell.style, truecolor);
                cur_style = Some(cell.style);
            }
            let mut buf = [0u8; 4];
            out.extend_from_slice(cell.ch.encode_utf8(&mut buf).as_bytes());
            let w = if cell.width == 2 { 2 } else { 1 };
            cursor = Some(cx + w);
            x = cx + w;
        }
    }
    if !out.is_empty() && cur_style != Some(Style::default()) {
        out.extend_from_slice(b"\x1b[0m");
    }
    out
}

impl Presenter {
    /// Enter alternate screen + raw mode (only if stdin is a TTY for raw mode; stdin non-TTY: keys ignored,
    /// UI still renders).
    /// Writes `ESC[?1049h`; uses `crate::term::RawMode` for raw mode.
    pub fn enter(out: &mut dyn Write) -> std::io::Result<Self> {
        out.write_all(b"\x1b[?1049h\x1b[?25l")?;
        out.flush()?;
        Ok(Presenter { prev: None, truecolor: false, raw: Some(RawMode::enable()), entered: true })
    }

    /// Use 24-bit SGR (else 256-color fallback).
    pub fn with_truecolor(mut self, truecolor: bool) -> Self {
        self.truecolor = truecolor;
        self
    }

    /// Leave alternate screen, restore terminal. Idempotent; also called on panic/exit paths.
    pub fn leave(&mut self, out: &mut dyn Write) -> std::io::Result<()> {
        if let Some(mut r) = self.raw.take() {
            r.disable();
        }
        if !self.entered {
            return Ok(());
        }
        self.entered = false;
        self.prev = None;
        out.write_all(b"\x1b[0m\x1b[?25h\x1b[?1049l")?;
        out.flush()
    }

    /// Current terminal size (80x24 when unknown).
    pub fn size() -> Size {
        crate::term::size()
    }

    /// Draw `screen` and flush before returning (the barrier reply is written after this returns).
    pub fn draw(&mut self, screen: &Screen, out: &mut dyn Write) -> std::io::Result<()> {
        let bytes = encode_frame(self.prev.as_ref(), screen, self.truecolor);
        if !bytes.is_empty() {
            out.write_all(&bytes)?;
        }
        out.flush()?;
        self.prev = Some(screen.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(cols: u16, rows: u16, lines: &[&str]) -> Screen {
        let mut s = Screen::blank(Size { cols, rows });
        for (y, l) in lines.iter().enumerate() {
            for (x, ch) in l.chars().enumerate() {
                s.rows[y][x].ch = ch;
            }
        }
        s
    }
    fn text(b: &[u8]) -> String {
        String::from_utf8_lossy(b).into_owned()
    }
    fn wide(ch: char) -> [Cell; 2] {
        [Cell { ch, width: 2, style: Style::default() }, Cell { ch: ' ', width: 0, style: Style::default() }]
    }

    #[test]
    fn f_cli_05_full_repaint_when_no_prev() {
        let s = screen(4, 2, &["ab", "cd"]);
        let out = text(&encode_frame(None, &s, true));
        assert_eq!(out, "\x1b[?25l\x1b[0m\x1b[2J\x1b[1;1Hab\x1b[2;1Hcd");
    }

    #[test]
    fn f_layout_01_full_repaint_when_size_differs() {
        let a = screen(4, 2, &["ab"]);
        let b = screen(5, 2, &["ab"]);
        let out = text(&encode_frame(Some(&a), &b, true));
        assert!(out.starts_with("\x1b[?25l"));
        assert!(out.contains("\x1b[2J"));
    }

    #[test]
    fn f_cli_05_identical_frame_is_empty() {
        let s = screen(4, 2, &["ab"]);
        assert!(encode_frame(Some(&s), &s, true).is_empty());
    }

    #[test]
    fn f_cli_05_diff_only_changed_cells() {
        let a = screen(6, 2, &["abcdef", "zzzzzz"]);
        let b = screen(6, 2, &["abXdeY", "zzzzzz"]);
        assert_eq!(text(&encode_frame(Some(&a), &b, true)), "\x1b[1;3HX\x1b[1;6HY");
    }

    #[test]
    fn f_cli_05_diff_contiguous_run_no_move() {
        let a = screen(6, 1, &["abcdef"]);
        let b = screen(6, 1, &["aXYZef"]);
        assert_eq!(text(&encode_frame(Some(&a), &b, true)), "\x1b[1;2HXYZ");
    }

    #[test]
    fn colors_truecolor_sgr() {
        let mut s = screen(2, 1, &["a"]);
        s.rows[0][0].style = Style {
            fg: Some(Color::Rgb(1, 2, 3)),
            bg: Some(Color::Rgb(4, 5, 6)),
            bold: true,
            ..Style::default()
        };
        let out = text(&encode_frame(None, &s, true));
        assert!(out.contains("\x1b[0;1;38;2;1;2;3;48;2;4;5;6ma"), "{out:?}");
        assert!(out.ends_with("\x1b[0m"), "style reset before end: {out:?}");
    }

    #[test]
    fn colors_256_fallback() {
        let mut s = screen(2, 1, &["a"]);
        s.rows[0][0].style = Style {
            fg: Some(Color::Rgb(255, 0, 0)),
            bg: Some(Color::Rgb(128, 128, 128)),
            ..Style::default()
        };
        let out = text(&encode_frame(None, &s, false));
        assert!(out.contains("\x1b[0;38;5;196;48;5;244ma"), "{out:?}");
    }

    #[test]
    fn colors_named_ansi() {
        let named = [
            (Color::Black, "30"),
            (Color::Red, "31"),
            (Color::Green, "32"),
            (Color::Yellow, "33"),
            (Color::Blue, "34"),
            (Color::Magenta, "35"),
            (Color::Cyan, "36"),
            (Color::White, "37"),
            (Color::Gray, "90"),
            (Color::RedBright, "91"),
            (Color::GreenBright, "92"),
        ];
        for (c, n) in named {
            assert_eq!(color_params(c, false, true), n);
        }
        assert_eq!(color_params(Color::Cyan, true, false), "46");
        assert_eq!(color_params(Color::Gray, true, false), "100");
    }

    #[test]
    fn colors_attrs() {
        let mut s = screen(1, 1, &["a"]);
        s.rows[0][0].style =
            Style { dim: true, italic: true, underline: true, reverse: true, ..Style::default() };
        assert!(text(&encode_frame(None, &s, true)).contains("\x1b[0;2;3;4;7ma"));
    }

    #[test]
    fn rgb_to_256_values() {
        assert_eq!(rgb_to_256(0, 0, 0), 16);
        assert_eq!(rgb_to_256(255, 255, 255), 231);
        assert_eq!(rgb_to_256(0, 255, 0), 46);
        assert_eq!(rgb_to_256(128, 128, 128), 244);
    }

    #[test]
    fn f_layout_01_wide_cells_skip_continuation() {
        let mut s = Screen::blank(Size { cols: 4, rows: 1 });
        s.rows[0][..2].copy_from_slice(&wide('中'));
        s.rows[0][2].ch = 'x';
        let out = text(&encode_frame(None, &s, true));
        assert!(out.ends_with("\x1b[1;1H中x"), "{out:?}");
    }

    #[test]
    fn f_layout_01_wide_diff_repaints_from_lead() {
        let mut a = Screen::blank(Size { cols: 4, rows: 1 });
        a.rows[0][..2].copy_from_slice(&wide('中'));
        let mut b = a.clone();
        b.rows[0][..2].copy_from_slice(&wide('文'));
        assert_eq!(text(&encode_frame(Some(&a), &b, true)), "\x1b[1;1H文");
        let c = screen(4, 1, &["ab"]);
        assert_eq!(text(&encode_frame(Some(&a), &c, true)), "\x1b[1;1Hab");
    }

    #[test]
    fn f_cli_05_enter_leave_bytes_and_idempotent() {
        let mut out = Vec::new();
        let mut p = Presenter::enter(&mut out).unwrap();
        assert!(text(&out).starts_with("\x1b[?1049h"));
        out.clear();
        p.leave(&mut out).unwrap();
        assert!(text(&out).contains("\x1b[?25h"));
        assert!(text(&out).ends_with("\x1b[?1049l"));
        out.clear();
        p.leave(&mut out).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn f_cli_05_draw_writes_and_tracks_prev() {
        let mut out = Vec::new();
        let mut p = Presenter::enter(&mut out).unwrap();
        out.clear();
        let s = screen(3, 1, &["hi"]);
        p.draw(&s, &mut out).unwrap();
        assert!(text(&out).contains("hi"));
        out.clear();
        p.draw(&s, &mut out).unwrap();
        assert!(out.is_empty());
        p.leave(&mut out).unwrap();
    }
}
