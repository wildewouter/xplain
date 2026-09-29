//! Header line, rule and footer line.
//!
//! Spec: F-HEADER-01 (diff header chips + colors), F-HEADER-02 (browse header), F-HEADER-03 (cursor tag via
//! `nav::visual::tag_text`), F-LAYOUT-01/02 (footer composition: find/goto input, term, note, `too narrow for
//! split`, `(a-b/n)`, hints from `help::footer_hints`), F-LAYOUT-07 (truncation). Oracle: header/footer parts
//! of `src/app.tsx` and `src/components/DiffView.tsx`. Owner: component `viewframe` (F1).

use crate::canvas::Canvas;
use crate::help;
use crate::nav::{viewport, visual};
use crate::screen::Style;
use crate::state::{Overlay, State};
use crate::theme::Theme;
use crate::view::{bold, fg, frame_height};

const NARROW_SPLIT: u16 = 100;

/// Styled text run of the header.
type Seg = (String, Style);

fn seg(text: impl Into<String>, style: Style) -> Seg {
    (text.into(), style)
}

/// `[mcp: on] ` / `[mcp: off] ` chip (view color). On only while the server runs (F-HEADER-01).
fn mcp_chip(state: &State, theme: &Theme) -> Seg {
    seg(format!("[mcp: {}] ", if state.mcp.is_running() { "on" } else { "off" }), fg(theme.view))
}

/// Header runs. `tag` is the cursor tag text from `nav::visual::tag_text` (without brackets), only used in
/// diff and browse headers.
pub fn header_segs(state: &State, theme: &Theme, tag: &str) -> Vec<Seg> {
    let mode = state.settings.mode.as_str();
    let tag_seg = seg(format!("[{tag}] "), Style { bold: true, ..fg(theme.accent) });
    if let Some(b) = &state.browse {
        return vec![
            seg("[browse] ", fg(theme.mode)),
            seg(format!("[{}] ", state.settings.theme.as_str()), fg(theme.view)),
            mcp_chip(state, theme),
            tag_seg,
            seg(b.path.clone(), fg(theme.file)),
        ];
    }
    let Some(file) = state.files.get(state.nav.file_index) else {
        return vec![
            seg(format!("[{mode}] "), fg(theme.mode)),
            mcp_chip(state, theme),
            seg("No changes (m cycles mode, F search, q quits)", Style::default()),
        ];
    };
    vec![
        seg(format!("[{mode}] "), fg(theme.mode)),
        seg(format!("[{}] ", if state.settings.full { "full" } else { "changes" }), fg(theme.mode)),
        seg(format!("[{}] ", if state.settings.split { "split" } else { "unified" }), fg(theme.view)),
        seg(format!("[{}] ", state.settings.theme.as_str()), fg(theme.view)),
        mcp_chip(state, theme),
        seg(format!("[{}/{}] ", state.nav.file_index + 1, state.files.len()), bold(Style::default())),
        tag_seg,
        seg(file.display_path(), fg(theme.file)),
        seg(format!(" +{}", file.adds), fg(theme.adds)),
        seg(format!(" -{}", file.dels), fg(theme.dels)),
    ]
}

fn shows_cursor(state: &State) -> bool {
    state.browse.is_some() || state.files.get(state.nav.file_index).is_some()
}

pub fn draw_header(c: &mut Canvas, state: &State, theme: &Theme) {
    let tag = if shows_cursor(state) { visual::tag_text(state) } else { String::new() };
    draw_header_with_tag(c, state, theme, &tag);
}

/// [`draw_header`] with the cursor tag supplied.
pub fn draw_header_with_tag(c: &mut Canvas, state: &State, theme: &Theme, tag: &str) {
    let segs = header_segs(state, theme, tag);
    let refs: Vec<(&str, Style)> = segs.iter().map(|(t, s)| (t.as_str(), *s)).collect();
    c.put_segs_trunc(0, 0, c.size().cols, &refs);
}

/// Rule row (row 2) and the footer (last row).
pub fn draw_footer(c: &mut Canvas, state: &State, theme: &Theme) {
    let cols = c.size().cols;
    c.hline(0, 1, cols.saturating_sub(1).max(1), fg(theme.dim));
    c.put_trunc(0, frame_height(c.size()) - 1, cols, &footer_text(state), fg(theme.dim));
}

/// Inputs of the footer line, in composition order (F-LAYOUT-02).
pub struct FooterParts<'a> {
    /// `/` or `:` and the typed text while find/goto input is open.
    pub input: Option<(char, &'a str)>,
    pub term: Option<&'a str>,
    pub note: Option<&'a str>,
    pub narrow_split: bool,
    /// `(a, b, n)` scroll window.
    pub window: (usize, usize, usize),
    pub hints: &'a str,
}

/// Pure footer composition (F-LAYOUT-02).
pub fn compose_footer(p: &FooterParts) -> String {
    let mut out = String::new();
    if let Some((k, text)) = p.input {
        out.push(k);
        out.push_str(text);
        out.push('\u{2588}');
    } else {
        if let Some(t) = p.term.filter(|t| !t.is_empty()) {
            out.push_str(&format!("/{t} | "));
        }
        if let Some(n) = p.note.filter(|n| !n.is_empty()) {
            out.push_str(&format!("{n} | "));
        }
    }
    if p.narrow_split {
        out.push_str("too narrow for split | ");
    }
    let (a, b, n) = p.window;
    out.push_str(&format!("({a}-{b}/{n}) {}", p.hints));
    out
}

/// Footer text without style, for reuse in tests.
pub fn footer_text(state: &State) -> String {
    let input = match &state.overlay {
        Overlay::Find { text } => Some(('/', text.as_str())),
        Overlay::Goto { text } => Some((':', text.as_str())),
        _ => None,
    };
    let hints = help::footer_hints(state);
    compose_footer(&FooterParts {
        input,
        term: state.find.term.as_deref(),
        note: state.note.as_deref(),
        narrow_split: state.settings.split && state.size.cols < NARROW_SPLIT,
        window: viewport::window(state),
        hints: &hints,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::{FileDiff, Status};
    use crate::screen::Size;
    use crate::state::Browse;
    use crate::view::testutil::{rgb, state, theme};

    fn file(path: &str, old: Option<&str>, adds: u32, dels: u32) -> FileDiff {
        FileDiff {
            path: path.to_string(),
            old_path: old.map(str::to_string),
            status: Status::Modified,
            adds,
            dels,
            hunks: Vec::new(),
            note: None,
        }
    }

    fn header_row(st: &State, tag: &str) -> crate::screen::Screen {
        let mut c = Canvas::new(st.size);
        draw_header_with_tag(&mut c, st, &theme(), tag);
        c.into_screen()
    }

    fn diff_state(cols: u16) -> State {
        let mut st = state(cols, 24);
        st.files = vec![
            file("f.txt", None, 3, 1),
            file("g.txt", None, 0, 0),
            file("h", None, 0, 0),
            file("i", None, 0, 0),
        ];
        st
    }

    #[test]
    fn f_header_01_diff_text() {
        let st = diff_state(120);
        let s = header_row(&st, "cursor L2:C1");
        assert_eq!(
            s.row_text(0).trim_end(),
            "[all] [full] [unified] [solarized] [mcp: off] [1/4] [cursor L2:C1] f.txt +3 -1"
        );
    }

    #[test]
    fn f_header_01_chip_colors() {
        let st = diff_state(120);
        let s = header_row(&st, "cursor L2:C1");
        let th = theme();
        let at = |text: &str| {
            let row = s.row_text(0);
            let x = row.find(text).unwrap_or(0);
            s.rows[0][x].style
        };
        assert_eq!(at("[all]").fg, Some(th.mode));
        assert_eq!(at("[full]").fg, Some(th.mode));
        assert_eq!(at("[unified]").fg, Some(th.view));
        assert_eq!(at("[solarized]").fg, Some(th.view));
        assert_eq!(at("[mcp: off]").fg, Some(th.view));
        assert!(at("[1/4]").bold);
        assert_eq!(at("[1/4]").fg, None);
        assert_eq!(at("[cursor").fg, Some(th.accent));
        assert!(at("[cursor").bold);
        assert_eq!(at("f.txt").fg, Some(th.file));
        assert_eq!(at("+3").fg, Some(th.adds));
        assert_eq!(at("-1").fg, Some(th.dels));
        assert_eq!(th.accent, rgb(0xcb4b16));
    }

    #[test]
    fn f_header_01_chips_follow_settings_and_mcp() {
        let mut st = diff_state(120);
        st.settings.full = false;
        st.settings.split = true;
        st.settings.mode = crate::options::DiffMode::Staged;
        st.mcp.set_running(true);
        let row = header_row(&st, "cursor L1:C1").row_text(0);
        assert!(row.starts_with("[staged] [changes] [split] [solarized] [mcp: on] [1/4] "), "{row}");
    }

    #[test]
    fn f_header_01_starting_is_off() {
        let mut st = diff_state(120);
        st.mcp.server = crate::mcp::ServerState::Starting(crate::event::ReqId(1));
        assert!(header_row(&st, "t").row_text(0).contains("[mcp: off]"));
    }

    #[test]
    fn f_header_01_rename_path() {
        let mut st = diff_state(120);
        st.files[0] = file("new.txt", Some("old.txt"), 1, 1);
        let row = header_row(&st, "cursor L1:C1").row_text(0);
        assert!(row.contains("[cursor L1:C1] old.txt -> new.txt +1 -1"), "{row}");
    }

    #[test]
    fn f_header_02_browse_text() {
        let mut st = state(120, 24);
        st.browse = Some(Browse { path: "src/a.ts".into(), lines: vec![] });
        let s = header_row(&st, "cursor L1:C1");
        assert_eq!(s.row_text(0).trim_end(), "[browse] [solarized] [mcp: off] [cursor L1:C1] src/a.ts");
        assert_eq!(s.rows[0][0].style.fg, Some(theme().mode));
        let x = s.row_text(0).find("[solarized]").unwrap_or(0);
        assert_eq!(s.rows[0][x].style.fg, Some(theme().view));
    }

    #[test]
    fn f_mode_05_no_changes_header() {
        let mut st = state(120, 24);
        st.settings.mode = crate::options::DiffMode::Unstaged;
        let s = header_row(&st, "");
        assert_eq!(
            s.row_text(0).trim_end(),
            "[unstaged] [mcp: off] No changes (m cycles mode, F search, q quits)"
        );
        assert!(!shows_cursor(&st));
    }

    #[test]
    fn f_layout_07_header_truncated_with_ellipsis() {
        let st = diff_state(60);
        let s = header_row(&st, "cursor new L12:C3");
        let row = s.row_text(0);
        assert_eq!(row.chars().count(), 60);
        assert!(row.ends_with("\u{2026}"), "{row}");
        assert!(
            row.starts_with("[all] [full] [unified] [solarized] [mcp: off] [1/4] [cursor\u{2026}"),
            "{row}"
        );
    }

    fn parts<'a>() -> FooterParts<'a> {
        FooterParts {
            input: None,
            term: None,
            note: None,
            narrow_split: false,
            window: (1, 20, 40),
            hints: "hjkl move  enter ask  J/K comments  ? help",
        }
    }

    #[test]
    fn f_layout_02_plain() {
        assert_eq!(compose_footer(&parts()), "(1-20/40) hjkl move  enter ask  J/K comments  ? help");
    }

    #[test]
    fn f_layout_02_order() {
        let p = FooterParts { term: Some("foo"), note: Some("reloaded"), narrow_split: true, ..parts() };
        assert_eq!(
            compose_footer(&p),
            "/foo | reloaded | too narrow for split | (1-20/40) hjkl move  enter ask  J/K comments  ? help"
        );
    }

    #[test]
    fn f_layout_02_find_open_no_separator() {
        let p = FooterParts { input: Some(('/', "foo")), term: Some("old"), note: Some("n"), ..parts() };
        assert_eq!(compose_footer(&p), "/foo\u{2588}(1-20/40) hjkl move  enter ask  J/K comments  ? help");
        let g = FooterParts { input: Some((':', "12")), narrow_split: true, ..parts() };
        assert!(compose_footer(&g).starts_with(":12\u{2588}too narrow for split | (1-20/40) "));
    }

    #[test]
    fn f_layout_02_no_changes_window() {
        let p = FooterParts { window: (0, 0, 0), ..parts() };
        assert!(compose_footer(&p).starts_with("(0-0/0) hjkl"));
    }

    #[test]
    fn footer_and_rule_rows() {
        let st = state(20, 8);
        let mut c = Canvas::new(Size { cols: 20, rows: 8 });
        let th = theme();
        c.hline(0, 1, 19, fg(th.dim));
        let s = c.into_screen();
        assert_eq!(s.row_text(1).chars().filter(|ch| *ch == '\u{2500}').count(), 19);
        assert_eq!(crate::view::frame_height(st.size), 8);
    }
}
