//! Drawing of `thread_layout::ThreadBox` (comment thread, agent note, editor) under a code row.
//!
//! Spec: F-COMMENT-04 (box render, focus style), F-COMMENT-10 (agent note look), F-ASK-05 (status, spinner
//! frame from `state.spinner`), F-ASK-06 (code block styling, highlight via `highlight`), F-ASK-08 (buttons),
//! F-THEME-02. Oracle: `src/components/AskBox.tsx`. Owner: component `viewrows` (F2).
//! Layout is decided by `thread_layout` (component `comments`); this file only maps tones to theme styles and
//! puts cells. Must not mutate state.
//!
//! Conventions: border glyphs are part of the layout lines (`Tone::Border`). A comment box (non-empty id)
//! has dim borders, accent + bold when focused (the layout picks the glyphs, we pick the color); the editor
//! box (empty id) uses the modal palette (`modal_border` / `modal_bg` / `modal_fg`). A spinner is an
//! `Accent` span holding exactly one spinner frame glyph: it is replaced by the frame for `state.spinner`.

use unicode_width::UnicodeWidthStr;

use crate::canvas::Canvas;
use crate::highlight::{language_for_fence, run_style};
use crate::hlcache::HlCache;
use crate::screen::Style;
use crate::state::State;
use crate::theme::{Theme, ThemeId};
use crate::thread_layout::{BoxLine, Span, ThreadBox, Tone};

/// Spinner frames, one per 80 ms tick (F-ASK-05).
pub const SPIN_FRAMES: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// Draw `b` starting at row `y` of the canvas; returns lines drawn (clipped at `max_y`).
pub fn draw_box(c: &mut Canvas, state: &State, theme: &Theme, b: &ThreadBox, y: u16, max_y: u16) -> u16 {
    draw_box_at(c, theme, state.settings.theme, Some(&state.hl), state.spinner, b, 0, y, max_y)
}

/// State-free core of [`draw_box`]: box at column `x`, rows `y..max_y`, syntax colors of `theme_id`.
#[allow(clippy::too_many_arguments)]
pub fn draw_box_at(
    c: &mut Canvas,
    theme: &Theme,
    theme_id: ThemeId,
    hl: Option<&HlCache>,
    spinner: usize,
    b: &ThreadBox,
    x: u16,
    y: u16,
    max_y: u16,
) -> u16 {
    let mut drawn = 0u16;
    for (i, line) in b.lines.iter().enumerate() {
        let row = y.saturating_add(drawn);
        if row >= max_y {
            break;
        }
        draw_line(c, theme, theme_id, hl, spinner, b, i, line, x, row);
        drawn += 1;
    }
    drawn
}

fn is_editor(b: &ThreadBox) -> bool {
    b.id.is_empty()
}

fn base_style(theme: &Theme, b: &ThreadBox) -> Style {
    if is_editor(b) {
        Style { fg: Some(theme.modal_fg), bg: Some(theme.modal_bg), ..Style::default() }
    } else {
        Style { fg: Some(theme.modal_fg), ..Style::default() }
    }
}

/// Style of a non-code span (F-COMMENT-04, F-ASK-05, F-ASK-08).
fn tone_style(theme: &Theme, b: &ThreadBox, line_index: usize, tone: Tone) -> Style {
    let mut s = base_style(theme, b);
    match tone {
        Tone::Normal | Tone::Code => {}
        Tone::Dim => s.fg = Some(theme.dim),
        Tone::Bold => s.bold = true,
        Tone::Accent => {
            s.fg = Some(theme.accent);
            // focused head row (line 0 is the top border) is bold
            s.bold = b.focused && !is_editor(b) && line_index == 1;
        }
        Tone::Error => s.fg = Some(theme.dels),
        Tone::Button => s.fg = Some(theme.accent),
        Tone::ButtonSelected => {
            s.fg = Some(theme.accent);
            s.bold = true;
            s.reverse = true;
        }
        Tone::Caret => s.reverse = true,
        Tone::Border => {
            if is_editor(b) {
                s.fg = Some(theme.modal_border);
            } else if b.focused {
                s.fg = Some(theme.accent);
                s.bold = true;
            } else {
                s.fg = Some(theme.dim);
            }
        }
    }
    s
}

fn spinner_text(text: &str, tone: Tone, spinner: usize) -> Option<String> {
    let mut it = text.chars();
    let ch = it.next()?;
    if tone == Tone::Accent && it.next().is_none() && SPIN_FRAMES.contains(&ch) {
        return Some(SPIN_FRAMES[spinner % SPIN_FRAMES.len()].to_string());
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn draw_line(
    c: &mut Canvas,
    theme: &Theme,
    theme_id: ThemeId,
    hl: Option<&HlCache>,
    spinner: usize,
    b: &ThreadBox,
    line_index: usize,
    line: &BoxLine,
    x: u16,
    y: u16,
) {
    let total: usize = line.spans.iter().map(|s| s.text.width()).sum();
    if is_editor(b) {
        // modal background under the whole line
        c.paint(x, y, total.min(u16::MAX as usize) as u16, base_style(theme, b));
    }
    let mut cx = x;
    for s in &line.spans {
        cx = cx.saturating_add(draw_span(c, theme, theme_id, hl, spinner, b, line_index, s, cx, y));
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_span(
    c: &mut Canvas,
    theme: &Theme,
    theme_id: ThemeId,
    hl: Option<&HlCache>,
    spinner: usize,
    b: &ThreadBox,
    line_index: usize,
    s: &Span,
    x: u16,
    y: u16,
) -> u16 {
    if s.tone == Tone::Code {
        let base = base_style(theme, b);
        let lang = s.lang.as_deref().and_then(language_for_fence);
        let runs = hl.zip(lang).and_then(|(h, l)| h.code_runs(l, &s.text));
        let Some(runs) = runs.filter(|r| !r.is_empty()) else {
            return c.put(x, y, &s.text, base);
        };
        let mut cx = x;
        let mut rest = s.text.as_str();
        for r in runs {
            let cut = rest.char_indices().nth(r.len as usize).map_or(rest.len(), |(i, _)| i);
            let (part, tail) = rest.split_at(cut);
            rest = tail;
            let (fg, bold) = run_style(theme_id, r.class);
            let st = Style { fg: fg.or(base.fg), bold, ..base };
            cx = cx.saturating_add(c.put(cx, y, part, st));
        }
        if !rest.is_empty() {
            cx = cx.saturating_add(c.put(cx, y, rest, base));
        }
        return cx - x;
    }
    let style = tone_style(theme, b, line_index, s.tone);
    match spinner_text(&s.text, s.tone, spinner) {
        Some(t) => c.put(x, y, &t, style),
        None => c.put(x, y, &s.text, style),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screen::Size;

    fn span(t: &str, tone: Tone) -> Span {
        Span { text: t.into(), tone, lang: None }
    }

    fn theme() -> Theme {
        Theme::of(ThemeId::Solarized)
    }

    fn one_box(id: &str, focused: bool, lines: Vec<Vec<Span>>) -> ThreadBox {
        ThreadBox {
            id: id.into(),
            focused,
            lines: lines.into_iter().map(|spans| BoxLine { spans }).collect(),
        }
    }

    fn cv() -> Canvas {
        Canvas::new(Size { cols: 30, rows: 8 })
    }

    #[test]
    fn f_comment_04_border_dim_and_focus() {
        let t = theme();
        let lines = vec![vec![span("╭──╮", Tone::Border)], vec![span(" sent  hi", Tone::Accent)]];
        for focused in [false, true] {
            let b = one_box("q1", focused, lines.clone());
            let mut c = cv();
            let n = draw_box_at(&mut c, &t, ThemeId::Solarized, None, 0, &b, 0, 1, 8);
            assert_eq!(n, 2);
            let s = c.into_screen();
            assert_eq!(&s.row_text(1)[..12], "╭──╮");
            let cell = s.rows[1][0];
            if focused {
                assert_eq!(cell.style.fg, Some(t.accent));
                assert!(cell.style.bold);
                assert!(s.rows[2][1].style.bold, "focused head bold");
            } else {
                assert_eq!(cell.style.fg, Some(t.dim));
                assert!(!cell.style.bold);
                assert!(!s.rows[2][1].style.bold);
            }
            assert_eq!(s.rows[2][1].style.fg, Some(t.accent));
        }
    }

    #[test]
    fn clipped_at_max_y() {
        let t = theme();
        let b = one_box("q1", false, vec![vec![span("a", Tone::Normal)]; 5]);
        let mut c = cv();
        assert_eq!(draw_box_at(&mut c, &t, ThemeId::Solarized, None, 0, &b, 0, 2, 4), 2);
        assert_eq!(draw_box_at(&mut c, &t, ThemeId::Solarized, None, 0, &b, 0, 6, 4), 0);
    }

    #[test]
    fn f_ask_05_spinner_frame_from_state() {
        let t = theme();
        let b = one_box(
            "q1",
            false,
            vec![vec![
                span(" ", Tone::Normal),
                span("⠋", Tone::Accent),
                span(" agent working…", Tone::Normal),
            ]],
        );
        for i in [0usize, 3, 13] {
            let mut c = cv();
            draw_box_at(&mut c, &t, ThemeId::Solarized, None, i, &b, 0, 0, 8);
            let s = c.into_screen();
            assert_eq!(s.rows[0][1].ch, SPIN_FRAMES[i % 10]);
            assert_eq!(s.rows[0][1].style.fg, Some(t.accent));
        }
        // the static waiting glyph stays
        let b = one_box("q1", false, vec![vec![span("⠿", Tone::Accent)]]);
        let mut c = cv();
        draw_box_at(&mut c, &t, ThemeId::Solarized, None, 4, &b, 0, 0, 8);
        assert_eq!(c.into_screen().rows[0][0].ch, '⠿');
    }

    #[test]
    fn f_ask_06_code_span_highlighted_text_unchanged() {
        let t = theme();
        let mut sp = span("fn main() { let x = 1; }", Tone::Code);
        sp.lang = Some("rust".into());
        let b = one_box("q1", false, vec![vec![span(" ", Tone::Normal), span("│ ", Tone::Dim), sp]]);
        let mut c = cv();
        draw_box_at(&mut c, &t, ThemeId::Solarized, None, 0, &b, 0, 0, 8);
        let s = c.into_screen();
        assert_eq!(s.row_text(0).trim_end(), " │ fn main() { let x = 1; }");
        assert_eq!(s.rows[0][1].style.fg, Some(t.dim));
        assert!(s.rows[0][3..27].iter().any(|c| c.style.fg.is_some()));
    }

    #[test]
    fn f_ask_08_buttons() {
        let t = theme();
        let b = one_box(
            "q1",
            true,
            vec![vec![
                span(" ", Tone::Normal),
                span("[ copy ]", Tone::Button),
                span(" ", Tone::Normal),
                span("[ copy ]", Tone::ButtonSelected),
            ]],
        );
        let mut c = cv();
        draw_box_at(&mut c, &t, ThemeId::Solarized, None, 0, &b, 0, 0, 8);
        let s = c.into_screen();
        assert_eq!(s.rows[0][2].style.fg, Some(t.accent));
        assert!(!s.rows[0][2].style.reverse);
        assert!(s.rows[0][11].style.reverse && s.rows[0][11].style.bold);
    }

    #[test]
    fn tones_map_to_theme() {
        let t = theme();
        let b = one_box(
            "q1",
            false,
            vec![vec![
                span("d", Tone::Dim),
                span("e", Tone::Error),
                span("b", Tone::Bold),
                span("c", Tone::Caret),
            ]],
        );
        let mut c = cv();
        draw_box_at(&mut c, &t, ThemeId::Solarized, None, 0, &b, 0, 0, 8);
        let s = c.into_screen();
        assert_eq!(s.rows[0][0].style.fg, Some(t.dim));
        assert_eq!(s.rows[0][1].style.fg, Some(t.dels));
        assert!(s.rows[0][2].style.bold);
        assert!(s.rows[0][3].style.reverse);
    }

    #[test]
    fn editor_box_uses_modal_palette() {
        let t = theme();
        let b = one_box("", true, vec![vec![span("╭─╮", Tone::Border)], vec![span(" hi", Tone::Normal)]]);
        let mut c = cv();
        draw_box_at(&mut c, &t, ThemeId::Solarized, None, 0, &b, 2, 0, 8);
        let s = c.into_screen();
        assert_eq!(s.rows[0][2].style.fg, Some(t.modal_border));
        assert_eq!(s.rows[0][2].style.bg, Some(t.modal_bg));
        assert_eq!(s.rows[1][3].style.bg, Some(t.modal_bg));
        assert_eq!(s.rows[1][3].style.fg, Some(t.modal_fg));
        assert_eq!(s.rows[0][0].style.bg, None);
    }
}
