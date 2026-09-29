//! The view entry point: `State` -> [`Screen`].
//!
//! Spec: F-LAYOUT-01 (frame: header, rule, viewport, footer), F-MODE-05 (no changes screen), F-MODE-04 (error
//! screen), `Loading...` (dim), F-LAYOUT-06 (modal stacking: overlay, then help panel over it).
//! Owner: component `viewframe` (F1); `viewrows` (F2) owns `view/body.rs`, `view/thread_box.rs`.
//! Must not: mutate state, do IO. Viewport `top`/`x_shift` are *state* (updated in `update`), the view
//! only reads them. Submodules are registered here up front.

pub mod body;
pub mod header;
pub mod help_panel;
pub mod layout;
pub mod modals;
pub mod thread_box;

use crate::canvas::{Canvas, Rect};
use crate::screen::{Color, Screen, Size, Style, fg};
use crate::state::{LoadState, State};
use crate::theme::Theme;

/// Viewport height H = max(3, R-3) (F-LAYOUT-01).
pub(crate) fn viewport_height(size: Size) -> u16 {
    size.rows.saturating_sub(3).max(3)
}

/// Rows the frame spans: H + 3 (header, rule, viewport, footer).
pub(crate) fn frame_height(size: Size) -> u16 {
    viewport_height(size) + 3
}

/// Render the full screen at `state.size`. Pure. `Loading...` (dim) while `LoadState::Loading`;
/// error screen for `LoadState::Error`.
pub fn view(state: &State) -> Screen {
    match &state.load {
        LoadState::Loading => loading_screen(state.size),
        LoadState::Error(text) => error_screen(state.size, text),
        LoadState::Ready => frame(state, &Theme::of(state.settings.theme)),
    }
}

/// `Loading...` dim at the top-left (no header, no footer).
fn loading_screen(size: Size) -> Screen {
    let mut c = Canvas::new(size);
    c.put(0, 0, "Loading...", Style { dim: true, ..Style::default() });
    c.into_screen()
}

/// F-MODE-04: whole screen replaced by the error text in red, wrapped at the width.
fn error_screen(size: Size, text: &str) -> Screen {
    let mut c = Canvas::new(size);
    let mut y = 0u16;
    'lines: for line in text.split('\n') {
        for piece in wrap_hard(line, usize::from(size.cols)) {
            if y >= size.rows {
                break 'lines;
            }
            c.put(0, y, &piece, fg(Color::Red));
            y += 1;
        }
    }
    c.into_screen()
}

/// Word wrap keeping spaces; over-long words are cut at `width` chars. Empty line stays one empty piece.
fn wrap_hard(line: &str, width: usize) -> Vec<String> {
    let w = width.max(1);
    let chars: Vec<char> = line.chars().collect();
    let mut out = Vec::new();
    let mut start = 0;
    while chars.len() - start > w {
        let window = &chars[start..start + w + 1];
        let cut = match window.iter().rposition(|c| *c == ' ') {
            Some(k) if k > 0 => k,
            _ => w,
        };
        out.push(chars[start..start + cut].iter().collect::<String>().trim_end().to_string());
        start += cut;
        while start < chars.len() && chars[start] == ' ' {
            start += 1;
        }
    }
    out.push(chars[start..].iter().collect());
    out
}

/// The frame: header, rule, viewport, footer, then modal, then help panel on top.
fn frame(state: &State, theme: &Theme) -> Screen {
    let mut c = Canvas::new(state.size);
    let h = viewport_height(state.size);
    header::draw_header(&mut c, state, theme);
    header::draw_footer(&mut c, state, theme);
    body::draw_body(&mut c, state, theme, Rect { x: 0, y: 2, w: state.size.cols, h });
    modals::draw(&mut c, state, theme);
    help_panel::draw(&mut c, state, theme);
    c.into_screen()
}

#[cfg(test)]
pub(crate) mod testutil {
    use super::*;

    /// Solarized chrome palette (SPEC Colors), independent of `Theme::of`.
    pub fn theme() -> Theme {
        let rgb = |h: u32| Color::Rgb((h >> 16) as u8, (h >> 8) as u8, h as u8);
        Theme {
            add_bg: rgb(0x0b3b1f),
            del_bg: rgb(0x4a1a1f),
            add_mark: rgb(0x859900),
            del_mark: rgb(0xdc322f),
            gutter: rgb(0x586e75),
            hunk: rgb(0x2aa198),
            mode: rgb(0xb58900),
            view: rgb(0x6c71c4),
            file: rgb(0x268bd2),
            adds: rgb(0x859900),
            dels: rgb(0xdc322f),
            dim: rgb(0x586e75),
            accent: rgb(0xcb4b16),
            modal_border: rgb(0x268bd2),
            modal_bg: rgb(0x002b36),
            modal_fg: rgb(0x93a1a1),
            sel_bg: rgb(0x073642),
            sel_fg: rgb(0x93a1a1),
            cur_bg: rgb(0x22586b),
            vis_bg: rgb(0x6b4f00),
            vis_fg: rgb(0xfdf6e3),
        }
    }

    pub fn rgb(h: u32) -> Color {
        Color::Rgb((h >> 16) as u8, (h >> 8) as u8, h as u8)
    }

    /// Ready state at `cols` x `rows`, no files, two fake integrations (Alpha registers, Beta copy-paste).
    pub fn state(cols: u16, rows: u16) -> State {
        let mut st = crate::state::testutil::fake_state();
        st.size = Size { cols, rows };
        st.load = LoadState::Ready;
        st.ready = true;
        st
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f_layout_01_frame_heights() {
        assert_eq!(viewport_height(Size { cols: 80, rows: 24 }), 21);
        assert_eq!(viewport_height(Size { cols: 80, rows: 6 }), 3);
        assert_eq!(frame_height(Size { cols: 80, rows: 24 }), 24);
    }

    #[test]
    fn loading_is_dim_and_alone() {
        let mut st = testutil::state(40, 8);
        st.load = LoadState::Loading;
        let s = view(&st);
        assert_eq!(s.row_text(0).trim_end(), "Loading...");
        assert!(s.rows[0][0].style.dim);
        assert_eq!(s.row_text(1).trim(), "");
        assert_eq!(s.row_text(7).trim(), "");
    }

    #[test]
    fn f_mode_04_error_screen_red_verbatim() {
        let mut st = testutil::state(30, 8);
        st.load = LoadState::Error("fatal: not a git repository\nsecond".to_string());
        let s = view(&st);
        assert_eq!(s.row_text(0).trim_end(), "fatal: not a git repository");
        assert_eq!(s.row_text(1).trim_end(), "second");
        assert_eq!(s.rows[0][0].style.fg, Some(Color::Red));
        assert_eq!(s.row_text(7).trim(), "");
    }

    #[test]
    fn error_screen_wraps_and_clips() {
        let s = error_screen(Size { cols: 10, rows: 2 }, "aaaa bbbb cccc dddd\n\nzzz");
        assert_eq!(s.row_text(0).trim_end(), "aaaa bbbb");
        assert_eq!(s.row_text(1).trim_end(), "cccc dddd");
    }

    #[test]
    fn wrap_hard_cuts_long_words() {
        assert_eq!(wrap_hard("abcdefghij", 4), vec!["abcd", "efgh", "ij"]);
        assert_eq!(wrap_hard("", 4), vec![""]);
    }
}
