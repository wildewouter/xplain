//! Modal boxes: file picker, file search, config, MCP, delete-comment, quit.
//!
//! Spec: F-LAYOUT-06/08 (placement, narrow), F-FILES-01, F-SEARCH-01 (hit highlights), F-CFGUI-01, F-MCPUI-01
//! (rows, status, clients, notes, preview, masking), F-COMMENT-08 (delete dialog text), F-QUIT-01 (quit dialog
//! text), F-THEME-02 (modal colors). Oracle: `src/components/{FileModal,SearchModal,ConfigModal,McpModal,
//! DeleteModal,QuitModal}.tsx`. Owner: component `viewframe` (F1). Reads data from `picker::entries`,
//! `search::hits`, `config_ui::rows`, `state.mcp`, `state.integration_state`. Must not mutate state.

use crate::canvas::{Canvas, Rect, center};
use crate::config_ui::{self, ConfigRow};
use crate::diff::Status;
use crate::integration::RegStatus;
use crate::picker::{self, PickerEntry};
use crate::screen::{Size, Style};
use crate::search;
use crate::state::{ConfigModal, HelpLevel, McpModal, Overlay, State};
use crate::theme::Theme;
use crate::view::{bold, fg, frame_height};

/// Styled text run.
type Seg = (String, Style);

fn seg(t: impl Into<String>, s: Style) -> Seg {
    (t.into(), s)
}

/// Styles every modal draws with.
struct Look {
    border: Style,
    fill: Style,
    dim: Style,
    accent: Style,
    sel: Style,
    err: Style,
    theme: Theme,
}

impl Look {
    fn new(theme: &Theme) -> Look {
        let fill = Style { fg: Some(theme.modal_fg), bg: Some(theme.modal_bg), ..Style::default() };
        Look {
            border: fg(theme.modal_border),
            fill,
            dim: Style { fg: Some(theme.dim), ..fill },
            accent: Style { fg: Some(theme.accent), ..fill },
            sel: Style { fg: Some(theme.sel_fg), bg: Some(theme.sel_bg), ..Style::default() },
            err: Style { fg: Some(theme.dels), ..fill },
            theme: *theme,
        }
    }

    /// Row base style: selected or plain.
    fn row(&self, on: bool) -> Style {
        if on { self.sel } else { self.fill }
    }
}

/// F-LAYOUT-06: box origin. Centered; with the help panel open the top sits at screen row 3.
fn place(size: Size, w: u16, h: u16, help_open: bool) -> (u16, u16) {
    let (x, y) = center(Size { cols: size.cols, rows: frame_height(size) }, w, h);
    (x, if help_open { 2 } else { y })
}

/// Box with border, returns inner rect.
fn open_box(c: &mut Canvas, lk: &Look, x: u16, y: u16, w: u16, h: u16) -> Rect {
    c.draw_box(Rect { x, y, w, h }, lk.border, lk.fill)
}

/// One inner row of styled segments, truncated with `…` at the inner width.
fn line(c: &mut Canvas, inner: Rect, k: u16, segs: &[Seg]) {
    if k >= inner.h {
        return;
    }
    let refs: Vec<(&str, Style)> = segs.iter().map(|(t, s)| (t.as_str(), *s)).collect();
    c.put_segs_trunc(inner.x, inner.y + k, inner.w, &refs);
}

/// Draw the open modal (if any) centered over the frame.
pub fn draw(c: &mut Canvas, state: &State, theme: &Theme) {
    let lk = Look::new(theme);
    let size = c.size();
    let help_open = state.help != HelpLevel::Closed;
    match &state.overlay {
        Overlay::Picker { sel } => {
            let entries = picker::entries(state);
            let (x, y, w, h) = picker_geometry(size, entries.len(), help_open);
            draw_picker(c, &lk, &entries, *sel, state.nav.file_index, Rect { x, y, w, h });
        }
        Overlay::Search(s) => {
            let hits = search::hits(state);
            let rows_t = frame_height(size);
            let w = list_width(size.cols);
            let h = rows_t.min(8.max(rows_t * 7 / 10));
            let (x, y) = place(size, w, h, help_open);
            let hit_list: Vec<(String, Vec<usize>)> = hits.into_iter().map(|h| (h.path, h.idx)).collect();
            draw_search(c, &lk, &s.query, &hit_list, s.sel, Rect { x, y, w, h });
        }
        Overlay::Config(m) => {
            let rows = config_ui::rows(state);
            let w = size.cols.min(56);
            let (x, y) = place(size, w, CONFIG_H, help_open);
            let committed = committed_values(state, m);
            draw_config(c, &lk, &rows, &committed, m, Rect { x, y, w, h: CONFIG_H });
        }
        Overlay::Mcp(m) => {
            let lines = mcp_lines(state, m, &lk);
            let w = size.cols.min(64);
            let h = lines.len() as u16 + 2;
            let (x, y) = place(size, w, h, help_open);
            let inner = open_box(c, &lk, x, y, w, h);
            for (k, l) in lines.iter().enumerate() {
                line(c, inner, k as u16, l);
            }
        }
        Overlay::DeleteComment { .. } => dialog(c, &lk, size, help_open, "Delete comment? (y/n)"),
        Overlay::Quit => dialog(c, &lk, size, help_open, "Quit xplain? (y/n)"),
        Overlay::None | Overlay::Find { .. } | Overlay::Goto { .. } | Overlay::Editor(_) => {}
    }
}

/// Picker and search width: min(cols, max(20, floor(cols*0.7))).
fn list_width(cols: u16) -> u16 {
    cols.min(20.max((u32::from(cols) * 7 / 10) as u16))
}

/// Picker box (x, y, w, h) for `n` files (F-FILES-01).
fn picker_geometry(size: Size, n: usize, help_open: bool) -> (u16, u16, u16, u16) {
    let rows_t = frame_height(size);
    let w = list_width(size.cols);
    let h = rows_t.min(5.max((n.min(usize::from(u16::MAX)) as u16).saturating_add(4).min(rows_t * 6 / 10)));
    let (x, y) = place(size, w, h, help_open);
    (x, y, w, h)
}

/// List window start: selection centered, clamped to the ends (F-FILES-01).
fn window_start(sel: usize, total: usize, vis: usize) -> usize {
    sel.saturating_sub(vis / 2).min(total.saturating_sub(vis))
}

/// F-FILES-01.
fn draw_picker(c: &mut Canvas, lk: &Look, entries: &[PickerEntry], sel: usize, current: usize, r: Rect) {
    let inner = open_box(c, lk, r.x, r.y, r.w, r.h);
    let vis = usize::from(r.h.saturating_sub(4)).max(1);
    line(c, inner, 0, &[seg(format!(" Files ({}/{})", sel + 1, entries.len()), bold(lk.fill))]);
    let start = window_start(sel, entries.len(), vis);
    for (k, e) in entries.iter().enumerate().skip(start).take(vis) {
        let on = k == sel;
        let base = lk.row(on);
        let (letter, adds, dels) = if on {
            (base, base, base)
        } else {
            let letter = match e.status {
                Status::Added => lk.theme.adds,
                Status::Deleted => lk.theme.dels,
                Status::Renamed => lk.theme.mode,
                Status::Modified => lk.theme.accent,
            };
            (
                Style { fg: Some(letter), ..base },
                Style { fg: Some(lk.theme.adds), ..base },
                Style { fg: Some(lk.theme.dels), ..base },
            )
        };
        let mut segs = vec![
            seg(if on { ">" } else { " " }, base),
            seg(e.status.letter().to_string(), letter),
            seg(" ", base),
            seg(e.label.clone(), base),
            seg(format!(" +{}", e.adds), adds),
            seg(format!(" -{}", e.dels), dels),
        ];
        if k == current {
            segs.push(seg(" *", base));
        }
        line(c, inner, 1 + (k - start) as u16, &segs);
    }
    line(
        c,
        inner,
        1 + vis as u16,
        &[seg(" j/k/\u{2191}\u{2193} move d/u half page enter open esc/q close", lk.dim)],
    );
}

/// F-SEARCH-01. `hits`: (path, matched char positions).
fn draw_search(c: &mut Canvas, lk: &Look, query: &str, hits: &[(String, Vec<usize>)], sel: usize, r: Rect) {
    let inner = open_box(c, lk, r.x, r.y, r.w, r.h);
    let vis = usize::from(r.h.saturating_sub(5)).max(1);
    let shown = if hits.is_empty() { 0 } else { sel + 1 };
    line(c, inner, 0, &[seg(format!(" Search ({shown}/{})", hits.len()), bold(lk.fill))]);
    line(
        c,
        inner,
        1,
        &[
            seg(" ", lk.fill),
            seg("> ", lk.accent),
            seg(query, lk.fill),
            seg(" ", Style { reverse: true, ..lk.fill }),
        ],
    );
    let start = window_start(sel, hits.len(), vis);
    for (k, (path, idx)) in hits.iter().enumerate().skip(start).take(vis) {
        let on = k == sel;
        let base = lk.row(on);
        let mut segs = vec![seg(if on { "> " } else { "  " }, base)];
        for (j, ch) in path.chars().enumerate() {
            let hit = idx.contains(&j);
            let st = match (on, hit) {
                (true, true) => bold(base),
                (true, false) => base,
                (false, true) => bold(lk.accent),
                (false, false) => lk.fill,
            };
            segs.push(seg(ch.to_string(), st));
        }
        line(c, inner, 2 + (k - start) as u16, &segs);
    }
    line(c, inner, 2 + vis as u16, &[seg(" \u{2191}\u{2193}/^n^p move enter open esc close", lk.dim)]);
}

const CONFIG_H: u16 = 10;
const CONFIG_LABEL_W: usize = 14;

/// Committed value of each of the 6 config rows.
fn committed_values(state: &State, m: &ConfigModal) -> [String; 6] {
    let on = |b: bool| if b { "on" } else { "off" }.to_string();
    let s = &state.settings;
    [
        m.committed_theme.as_str().to_string(),
        s.mode.as_str().to_string(),
        on(s.split),
        if s.full { "full" } else { "changes" }.to_string(),
        on(s.confirm_quit),
        on(s.mcp_autostart),
    ]
}

/// Choice slice `[lo, hi]` fitting `avail` cols that contains `at`: grow right first, then left, alternately.
fn window_choices(choices: &[String], at: usize, avail: usize) -> (usize, usize) {
    let w: Vec<usize> = choices.iter().map(|c| c.chars().count() + 2).collect();
    let (mut lo, mut hi) = (at, at);
    let mut used = w.get(at).copied().unwrap_or(0);
    let mut moved = true;
    while moved {
        moved = false;
        if hi + 1 < w.len() && used + 1 + w[hi + 1] <= avail {
            hi += 1;
            used += 1 + w[hi];
            moved = true;
        }
        if lo > 0 && used + 1 + w[lo - 1] <= avail {
            lo -= 1;
            used += 1 + w[lo];
            moved = true;
        }
    }
    (lo, hi)
}

/// F-CFGUI-01.
fn draw_config(
    c: &mut Canvas,
    lk: &Look,
    rows: &[ConfigRow],
    committed: &[String; 6],
    m: &ConfigModal,
    r: Rect,
) {
    let inner = open_box(c, lk, r.x, r.y, r.w, r.h);
    line(c, inner, 0, &[seg(" Config", bold(lk.fill))]);
    let prefix = 2 + CONFIG_LABEL_W;
    for (i, row) in rows.iter().enumerate() {
        let on = i == m.row;
        let base = lk.row(on);
        let value = committed.get(i).map(String::as_str).unwrap_or("");
        let fallback = row.choices.iter().position(|c| c == value).unwrap_or(0);
        let at = m.cursors.get(i).copied().unwrap_or(fallback).min(row.choices.len().saturating_sub(1));
        let avail = usize::from(r.w).saturating_sub(2 + prefix + 4);
        let (lo, hi) = window_choices(&row.choices, at, avail);
        let mut segs = vec![
            seg(format!("{} {:<w$}", if on { ">" } else { " " }, row.label, w = CONFIG_LABEL_W), base),
            seg(if lo > 0 { "\u{2039} " } else { "  " }, base),
        ];
        for (k, ch) in row.choices.iter().enumerate().skip(lo).take(hi + 1 - lo) {
            let st = if on && k == at { Style { reverse: true, ..base } } else { base };
            let cell = if ch == value { format!("[{ch}]") } else { format!(" {ch} ") };
            segs.push(seg(if k > lo { format!(" {cell}") } else { cell }, st));
        }
        if hi + 1 < row.choices.len() {
            segs.push(seg(" \u{203a}", base));
        }
        line(c, inner, 1 + i as u16, &segs);
    }
    line(c, inner, 1 + rows.len() as u16, &[seg(" j/k row h/l browse enter select esc close", lk.dim)]);
}

/// F-ASK-05 wrap: break at the last space within `width`, else hard cut; blank lines kept, trailing blank
/// lines dropped; tabs as 2 spaces, CR removed; trailing spaces of a line and leading spaces of the
/// continuation trimmed.
fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let w = width.max(1);
    let mut out: Vec<String> = Vec::new();
    for raw in text.replace('\r', "").replace('\t', "  ").split('\n') {
        let mut l: Vec<char> = raw.chars().collect();
        if l.is_empty() {
            out.push(String::new());
            continue;
        }
        while l.len() > w {
            let k = match l[..=w].iter().rposition(|c| *c == ' ') {
                Some(k) if k > 0 => k,
                _ => w,
            };
            out.push(l[..k].iter().collect::<String>().trim_end().to_string());
            let rest: String = l[k..].iter().collect();
            l = rest.trim_start().chars().collect();
        }
        out.push(l.into_iter().collect());
    }
    while out.len() > 1 && out.last().is_some_and(String::is_empty) {
        out.pop();
    }
    out
}

fn host_of(url: &str) -> String {
    let rest = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://")).unwrap_or(url);
    rest.split('/').next().unwrap_or("").to_string()
}

const MCP_CLIENTS_MAX: usize = 2;
const MCP_PREVIEW_MAX: usize = 3;

/// All inner rows of the MCP modal (F-MCPUI-01), each a run of styled segments.
fn mcp_lines(state: &State, m: &McpModal, lk: &Look) -> Vec<Vec<Seg>> {
    let mcp = &state.mcp;
    let inner_w = 10.max(usize::from(state.size.cols.min(64)).saturating_sub(4));
    let mut out: Vec<Vec<Seg>> = vec![vec![seg(" MCP", bold(lk.fill))]];
    let base = lk.row(m.row == 0);
    let mut power = format!(
        "{} {}",
        if m.row == 0 { ">" } else { " " },
        if mcp.starting().is_some() {
            "\u{2026} starting"
        } else if mcp.is_running() {
            "\u{25cf} on "
        } else {
            "\u{25cb} off"
        }
    );
    if mcp.is_running() {
        let host = mcp.endpoint().map(|e| host_of(&e.url)).unwrap_or_default();
        power.push_str(&format!(" {host}"));
    }
    out.push(vec![seg(power, base)]);
    if let Some(e) = mcp.start_error() {
        out.push(vec![seg(format!(" {e}"), lk.err)]);
    }
    out.push(vec![seg(
        format!(" clients {} pending {} delivered {}", mcp.clients.len(), mcp.queue.len(), mcp.delivered),
        lk.dim,
    )]);
    for cl in mcp.clients.iter().take(MCP_CLIENTS_MAX) {
        let version = if cl.version.is_empty() { String::new() } else { format!(" {}", cl.version) };
        let poll = if cl.polling { " \u{27f3}" } else { "" };
        out.push(vec![seg(format!("   {}{version}{poll}", cl.name), lk.fill)]);
    }
    if mcp.clients.len() > MCP_CLIENTS_MAX {
        out.push(vec![seg(format!("   \u{2026} +{} more", mcp.clients.len() - MCP_CLIENTS_MAX), lk.dim)]);
    }
    out.push(vec![seg(" INTEGRATIONS", lk.accent)]);
    for (i, integ) in state.integrations.iter().enumerate() {
        let ui = state.integration_state.get(i).cloned().unwrap_or_default();
        let on = m.row == i + 1;
        let status = if !integ.can_register() {
            "copy-paste only"
        } else {
            match ui.status {
                RegStatus::Registered => "registered",
                RegStatus::Stale => "stale",
                RegStatus::NotRegistered => "not registered",
            }
        };
        let restart = if integ.can_register() && integ.needs_restart() { "  restart needed" } else { "" };
        out.push(vec![seg(
            format!(
                "{} {} {}{status}{restart}",
                if on { ">" } else { " " },
                integ.label(),
                if ui.busy { "\u{2026} " } else { "" }
            ),
            lk.row(on),
        )]);
    }
    let confirming = m.confirm.is_some();
    if let Some((idx, register)) = m.confirm {
        if let Some(integ) = state.integrations.get(idx) {
            let label = integ.label();
            if register {
                out.push(vec![seg(format!(" Register {label} MCP server? (y/n)"), lk.accent)]);
                out.push(vec![seg(format!(" adds the xplain MCP server to {label} config"), lk.dim)]);
            } else {
                out.push(vec![seg(format!(" Remove {label} registration? (y/n)"), lk.accent)]);
                out.push(vec![seg(format!(" removes the xplain MCP server from {label} config"), lk.dim)]);
            }
        }
    }
    if !confirming {
        let selected_msg =
            m.row.checked_sub(1).and_then(|i| state.integration_state.get(i)).and_then(|u| u.message.clone());
        if let Some(note) = m.note.clone().or(selected_msg) {
            for l in wrap_text(&note, inner_w).into_iter().take(2) {
                out.push(vec![seg(format!(" {l}"), lk.accent)]);
            }
        }
        if let Some((what, text)) = &m.preview {
            out.push(vec![seg(format!(" {what}:"), lk.dim)]);
            let pv = wrap_text(text, inner_w);
            let mut shown: Vec<String> = pv.iter().take(MCP_PREVIEW_MAX).cloned().collect();
            if pv.len() > MCP_PREVIEW_MAX {
                if let Some(last) = shown.last_mut() {
                    *last = format!("{}\u{2026}", last.chars().take(inner_w - 1).collect::<String>());
                }
            }
            for l in shown {
                out.push(vec![seg(format!(" {l}"), lk.fill)]);
            }
        }
    }
    let hint = if confirming {
        " y confirm  n/esc cancel"
    } else {
        " j/k  enter register  d remove  c/w copy  R refresh  esc"
    };
    out.push(vec![seg(hint, lk.dim)]);
    out
}

/// Delete / quit dialog: one text row, one cell padding each side (F-COMMENT-08, F-QUIT-01).
fn dialog(c: &mut Canvas, lk: &Look, size: Size, help_open: bool, text: &str) {
    let w = text.chars().count() as u16 + 4;
    let (x, y) = place(size, w, 3, help_open);
    let inner = open_box(c, lk, x, y, w, 3);
    c.put(inner.x + 1, inner.y, text, lk.fill);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screen::{Color, Screen};
    use crate::view::testutil::{rgb, state, theme};

    fn look() -> Look {
        Look::new(&theme())
    }

    fn entry(status: Status, label: &str, adds: u32, dels: u32) -> PickerEntry {
        PickerEntry { status, label: label.to_string(), adds, dels }
    }

    fn canvas(cols: u16, rows: u16) -> Canvas {
        Canvas::new(Size { cols, rows })
    }

    fn cell_x(s: &Screen, row: usize, text: &str) -> usize {
        let t = s.row_text(row);
        t.find(text).map(|b| t[..b].chars().count()).unwrap_or(usize::MAX)
    }

    fn cols_text(s: &Screen, row: usize, from: usize, to: usize) -> String {
        s.row_text(row).chars().skip(from).take(to - from + 1).collect()
    }

    #[test]
    fn f_layout_06_delete_modal_25x3_at_80x24() {
        let mut st = state(80, 24);
        st.overlay = Overlay::DeleteComment { id: "c1".into() };
        let mut c = canvas(80, 24);
        draw(&mut c, &st, &theme());
        let s = c.into_screen();
        assert_eq!(
            cols_text(&s, 11, 28, 52),
            "\u{256d}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{256e}"
        );
        assert_eq!(cols_text(&s, 12, 28, 52), "\u{2502} Delete comment? (y/n) \u{2502}");
        assert_eq!(s.row_text(11).chars().take(28).collect::<String>().trim(), "");
        assert_eq!(s.row_text(14).trim(), "");
        assert!(s.row_text(13).trim().starts_with('\u{2570}'));
    }

    #[test]
    fn f_quit_01_dialog_text_and_size() {
        let mut st = state(80, 24);
        st.overlay = Overlay::Quit;
        let mut c = canvas(80, 24);
        draw(&mut c, &st, &theme());
        let s = c.into_screen();
        // width 22: left col ceil(58/2) = 29
        assert_eq!(cols_text(&s, 12, 29, 50), "\u{2502} Quit xplain? (y/n) \u{2502}");
        let th = theme();
        let x = cell_x(&s, 12, "Quit");
        assert_eq!(s.rows[12][x].style.fg, Some(th.modal_fg));
        assert_eq!(s.rows[12][x].style.bg, Some(th.modal_bg));
        assert_eq!(s.rows[12][29].style.fg, Some(th.modal_border));
    }

    #[test]
    fn f_layout_06_help_open_moves_modal_to_row_three() {
        let mut st = state(80, 24);
        st.overlay = Overlay::Quit;
        st.help = HelpLevel::L1;
        let (x, y) = place(Size { cols: 80, rows: 24 }, 22, 3, true);
        assert_eq!((x, y), (29, 2));
        let _ = &mut st;
    }

    #[test]
    fn f_files_01_geometry_80x24() {
        // width min(80, max(20, 56)) = 56; height min(24, max(5, min(3+4, 14))) = 7
        let (x, y, w, h) = picker_geometry(Size { cols: 80, rows: 24 }, 3, false);
        assert_eq!((x, y, w, h), (12, 9, 56, 7));
        let (_, _, _, h) = picker_geometry(Size { cols: 80, rows: 24 }, 0, false);
        assert_eq!(h, 5);
        let (_, y, _, h) = picker_geometry(Size { cols: 80, rows: 24 }, 40, false);
        assert_eq!((y, h), (5, 14));
    }

    #[test]
    fn f_files_01_rows_title_hint_colors() {
        let entries = vec![
            entry(Status::Modified, "a.txt", 3, 1),
            entry(Status::Renamed, "old.txt -> b.txt", 0, 0),
            entry(Status::Added, "c.txt", 5, 0),
        ];
        let lk = look();
        let mut c = canvas(80, 24);
        draw_picker(&mut c, &lk, &entries, 0, 1, Rect { x: 12, y: 8, w: 56, h: 7 });
        let s = c.into_screen();
        assert!(cols_text(&s, 9, 12, 67).starts_with("\u{2502} Files (1/3)  "));
        assert!(cols_text(&s, 10, 12, 67).starts_with("\u{2502}>M a.txt +3 -1"));
        assert!(cols_text(&s, 11, 12, 67).starts_with("\u{2502} R old.txt -> b.txt +0 -0 *"));
        assert!(cols_text(&s, 12, 12, 67).starts_with("\u{2502} A c.txt +5 -0"));
        assert!(
            cols_text(&s, 13, 12, 67)
                .contains(" j/k/\u{2191}\u{2193} move d/u half page enter open esc/q close")
        );
        let th = theme();
        // selected row: all cells selBg/selFg (status letter, +N, -N too)
        for t in [">M", "a.txt", "+3", "-1"] {
            let x = cell_x(&s, 10, t);
            assert_eq!(s.rows[10][x].style.bg, Some(th.sel_bg), "{t}");
            assert_eq!(s.rows[10][x].style.fg, Some(th.sel_fg), "{t}");
        }
        // others: own colors
        assert_eq!(s.rows[11][cell_x(&s, 11, "R old")].style.fg, Some(th.mode));
        assert_eq!(s.rows[12][cell_x(&s, 12, "A c")].style.fg, Some(th.adds));
        assert_eq!(s.rows[12][cell_x(&s, 12, "+5")].style.fg, Some(th.adds));
        assert_eq!(s.rows[12][cell_x(&s, 12, "-0")].style.fg, Some(th.dels));
        assert_eq!(s.rows[10][cell_x(&s, 10, "M a")].style.fg, Some(th.sel_fg));
        assert_eq!(s.rows[11][cell_x(&s, 11, "old.txt")].style.bg, Some(th.modal_bg));
        assert!(
            s.rows[13][cell_x(&s, 13, "j/k")].style.dim
                || s.rows[13][cell_x(&s, 13, "j/k")].style.fg == Some(th.dim)
        );
    }

    #[test]
    fn f_files_01_empty_list_title() {
        let mut c = canvas(80, 24);
        let (x, y, w, h) = picker_geometry(Size { cols: 80, rows: 24 }, 0, false);
        draw_picker(&mut c, &look(), &[], 0, 0, Rect { x, y, w, h });
        assert!(c.into_screen().row_text(usize::from(y) + 1).contains(" Files (1/0)"));
    }

    #[test]
    fn f_files_01_window_centered_and_clamped() {
        assert_eq!(window_start(0, 20, 5), 0);
        assert_eq!(window_start(10, 20, 5), 8);
        assert_eq!(window_start(19, 20, 5), 15);
        assert_eq!(window_start(2, 3, 5), 0);
    }

    fn hit(path: &str, idx: &[usize]) -> (String, Vec<usize>) {
        (path.to_string(), idx.to_vec())
    }

    #[test]
    fn f_search_01_layout_and_highlights() {
        let hits = vec![hit("README.md", &[0, 1]), hit("readme.md", &[0, 1])];
        let mut c = canvas(80, 24);
        draw_search(&mut c, &look(), "rm", &hits, 0, Rect { x: 12, y: 4, w: 56, h: 16 });
        let s = c.into_screen();
        let th = theme();
        assert!(cols_text(&s, 5, 12, 67).starts_with("\u{2502} Search (1/2)  "));
        assert!(cols_text(&s, 6, 12, 67).starts_with("\u{2502} > rm "));
        assert!(cols_text(&s, 7, 12, 67).starts_with("\u{2502}> README.md "));
        assert!(cols_text(&s, 8, 12, 67).starts_with("\u{2502}  readme.md "));
        assert!(
            cols_text(&s, 18, 12, 67)
                .starts_with("\u{2502} \u{2191}\u{2193}/^n^p move enter open esc close ")
        );
        assert_eq!(cols_text(&s, 19, 12, 67), format!("\u{2570}{}\u{256f}", "\u{2500}".repeat(54)));
        // query row: accent `> `, inverse caret
        let x = cell_x(&s, 6, "> rm");
        assert_eq!(s.rows[6][x].style.fg, Some(th.accent));
        assert!(s.rows[6][x + 4].style.reverse);
        // selected row: sel colors, matched chars bold
        let r = cell_x(&s, 7, "README");
        assert_eq!(s.rows[7][r].style.bg, Some(th.sel_bg));
        assert_eq!(s.rows[7][r].style.fg, Some(th.sel_fg));
        assert!(s.rows[7][r].style.bold);
        assert!(!s.rows[7][r + 2].style.bold);
        // unselected: matched accent bold, others modalFg
        let u = cell_x(&s, 8, "readme");
        assert_eq!(s.rows[8][u].style.fg, Some(th.accent));
        assert!(s.rows[8][u + 1].style.bold);
        assert_eq!(s.rows[8][u + 2].style.fg, Some(th.modal_fg));
        assert_eq!(s.rows[8][u + 2].style.bg, Some(th.modal_bg));
        assert!(!s.rows[8][u + 2].style.bold);
    }

    #[test]
    fn f_search_01_no_hits_title() {
        let mut c = canvas(80, 24);
        draw_search(&mut c, &look(), "zzz", &[], 0, Rect { x: 12, y: 4, w: 56, h: 16 });
        assert!(c.into_screen().row_text(5).contains(" Search (0/0)"));
    }

    #[test]
    fn f_search_01_window_height_minus_five() {
        let hits: Vec<_> = (0..30).map(|i| hit(&format!("f{i:02}"), &[])).collect();
        let mut c = canvas(80, 24);
        draw_search(&mut c, &look(), "", &hits, 15, Rect { x: 12, y: 4, w: 56, h: 16 });
        let s = c.into_screen();
        // vis 11, start = 15-5 = 10 -> f10..f20 on rows 7..17
        assert!(s.row_text(7).contains("f10"));
        assert!(s.row_text(12).contains("> f15"));
        assert!(s.row_text(17).contains("f20"));
    }

    fn cfg_rows() -> Vec<ConfigRow> {
        let v = |xs: &[&str]| xs.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        vec![
            ConfigRow {
                label: "theme",
                choices: v(&["solarized", "vibrant", "dull", "contrast", "colorblind", "light"]),
            },
            ConfigRow { label: "mode", choices: v(&["all", "staged", "unstaged"]) },
            ConfigRow { label: "split", choices: v(&["off", "on"]) },
            ConfigRow { label: "view", choices: v(&["full", "changes"]) },
            ConfigRow { label: "confirm quit", choices: v(&["off", "on"]) },
            ConfigRow { label: "mcp on startup", choices: v(&["off", "on"]) },
        ]
    }

    fn cfg_modal(row: usize, cursors: [usize; 6]) -> ConfigModal {
        ConfigModal { row, cursors, committed_theme: crate::theme::ThemeId::Solarized }
    }

    fn cfg_committed() -> [String; 6] {
        ["solarized", "all", "off", "full", "off", "off"].map(str::to_string)
    }

    #[test]
    fn f_cfgui_01_rows_spec_example() {
        let mut c = canvas(80, 24);
        draw_config(
            &mut c,
            &look(),
            &cfg_rows(),
            &cfg_committed(),
            &cfg_modal(0, [0; 6]),
            Rect { x: 12, y: 7, w: 56, h: 10 },
        );
        let s = c.into_screen();
        assert!(cols_text(&s, 8, 13, 66).starts_with(" Config "));
        assert!(
            cols_text(&s, 9, 13, 66).starts_with("> theme           [solarized]  vibrant   dull  \u{203a}")
        );
        assert!(cols_text(&s, 10, 13, 66).starts_with("  mode            [all]  staged   unstaged"));
        assert!(cols_text(&s, 11, 13, 66).starts_with("  split           [off]  on "));
        assert_eq!(cols_text(&s, 15, 13, 66).trim_end(), " j/k row h/l browse enter select esc close");
    }

    #[test]
    fn f_cfgui_01_selected_row_and_cursor_reverse() {
        let mut c = canvas(80, 24);
        draw_config(
            &mut c,
            &look(),
            &cfg_rows(),
            &cfg_committed(),
            &cfg_modal(1, [0, 1, 0, 0, 0, 0]),
            Rect { x: 12, y: 7, w: 56, h: 10 },
        );
        let s = c.into_screen();
        let th = theme();
        assert_eq!(s.rows[10][cell_x(&s, 10, "> mode")].style.bg, Some(th.sel_bg));
        assert_eq!(s.rows[9][cell_x(&s, 9, "  theme")].style.bg, Some(th.modal_bg));
        // cursor on `staged`: reverse covers the joining space and ` staged `
        let x = cell_x(&s, 10, "[all]") + 5;
        assert!(s.rows[10][x].style.reverse, "joining space");
        assert!(s.rows[10][x + 1].style.reverse);
        assert!(s.rows[10][x + 8].style.reverse);
        assert!(!s.rows[10][x + 9].style.reverse);
        assert!(!s.rows[10][x - 1].style.reverse);
        assert_eq!(s.rows[10][x + 1].style.fg, Some(th.sel_fg));
    }

    #[test]
    fn f_cfgui_01_choice_window_arrows() {
        let ch: Vec<String> =
            ["solarized", "vibrant", "dull", "contrast", "colorblind", "light"].map(str::to_string).into();
        // width 56 -> avail 34
        assert_eq!(window_choices(&ch, 0, 34), (0, 2));
        assert_eq!(window_choices(&ch, 5, 34), (3, 5));
        assert_eq!(window_choices(&ch, 2, 34), (1, 3));
        let mut c = canvas(80, 24);
        draw_config(
            &mut c,
            &look(),
            &cfg_rows(),
            &cfg_committed(),
            &cfg_modal(0, [5, 0, 0, 0, 0, 0]),
            Rect { x: 12, y: 7, w: 56, h: 10 },
        );
        let s = c.into_screen();
        assert!(s.row_text(9).contains("\u{2039} "));
        assert!(!s.row_text(9).contains('\u{203a}'));
    }

    #[test]
    fn f_cfgui_01_committed_values() {
        let mut st = state(80, 24);
        st.settings.mode = crate::options::DiffMode::Unstaged;
        st.settings.split = true;
        st.settings.full = false;
        st.settings.confirm_quit = true;
        st.settings.mcp_autostart = true;
        let m = ConfigModal { row: 0, cursors: [0; 6], committed_theme: crate::theme::ThemeId::Light };
        assert_eq!(
            committed_values(&st, &m),
            ["light", "unstaged", "on", "changes", "on", "on"].map(str::to_string)
        );
    }

    fn mcp_state(rows: usize) -> State {
        let mut st = state(80, 24);
        st.mcp.server = crate::mcp::ServerState::Running(crate::mcp::McpEndpoint {
            url: "http://127.0.0.1:47615/mcp".into(),
            token: "t".into(),
            port: 47615,
        });
        st.integration_state.resize(rows, Default::default());
        st
    }

    fn texts(lines: &[Vec<Seg>]) -> Vec<String> {
        lines.iter().map(|l| l.iter().map(|s| s.0.as_str()).collect::<String>()).collect()
    }

    #[test]
    fn f_mcpui_01_spec_example() {
        let st = mcp_state(st_ints());
        let lines = texts(&mcp_lines(&st, &McpModal::default(), &look()));
        assert_eq!(lines[0], " MCP");
        assert_eq!(lines[1], "> \u{25cf} on  127.0.0.1:47615");
        assert_eq!(lines[2], " clients 0 pending 0 delivered 0");
        assert_eq!(lines[3], " INTEGRATIONS");
        assert_eq!(lines[4], "  Alpha not registered  restart needed");
        assert_eq!(lines[5], "  Beta copy-paste only");
        assert_eq!(lines[6], " j/k  enter register  d remove  c/w copy  R refresh  esc");
        assert_eq!(lines.len(), 7);
    }

    fn st_ints() -> usize {
        state(80, 24).integrations.len()
    }

    #[test]
    fn f_mcpui_01_power_row_variants() {
        let mut st = mcp_state(2);
        st.mcp.set_running(false);
        let l = texts(&mcp_lines(&st, &McpModal::default(), &look()));
        assert_eq!(l[1], "> \u{25cb} off");
        st.mcp.server = crate::mcp::ServerState::Starting(crate::event::ReqId(1));
        let l = texts(&mcp_lines(&st, &McpModal { row: 1, ..Default::default() }, &look()));
        assert_eq!(l[1], "  \u{2026} starting");
        st.mcp.server = crate::mcp::ServerState::Stopped { last_error: Some("port busy".into()) };
        let l = texts(&mcp_lines(&st, &McpModal::default(), &look()));
        assert_eq!(l[2], " port busy");
    }

    #[test]
    fn f_mcpui_01_clients_list() {
        let mut st = mcp_state(2);
        let cl = |n: &str, v: &str, p: bool| crate::mcp::ClientInfo {
            id: n.into(),
            name: n.into(),
            version: v.into(),
            polling: p,
        };
        st.mcp.clients = vec![cl("one", "1.2", true), cl("two", "", false), cl("three", "3", false)];
        st.mcp.delivered = 4;
        let l = texts(&mcp_lines(&st, &McpModal::default(), &look()));
        assert_eq!(l[2], " clients 3 pending 0 delivered 4");
        assert_eq!(l[3], "   one 1.2 \u{27f3}");
        assert_eq!(l[4], "   two");
        assert_eq!(l[5], "   \u{2026} +1 more");
        assert_eq!(l[6], " INTEGRATIONS");
    }

    #[test]
    fn f_mcpui_01_busy_status_and_selection() {
        let mut st = mcp_state(2);
        st.integration_state[0].busy = true;
        st.integration_state[0].status = RegStatus::Stale;
        let m = McpModal { row: 1, ..Default::default() };
        let lk = look();
        let lines = mcp_lines(&st, &m, &lk);
        let t = texts(&lines);
        assert_eq!(t[4], "> Alpha \u{2026} stale  restart needed");
        assert_eq!(lines[4][0].1, lk.sel);
        assert_eq!(lines[1][0].1, lk.fill);
    }

    #[test]
    fn f_mcpui_01_confirm_hides_note_and_preview() {
        let st = mcp_state(2);
        let m = McpModal {
            row: 1,
            confirm: Some((0, true)),
            note: Some("hidden".into()),
            preview: Some(("watch prompt (copied)".into(), "x".into())),
        };
        let t = texts(&mcp_lines(&st, &m, &look()));
        assert_eq!(t[6], " Register Alpha MCP server? (y/n)");
        assert_eq!(t[7], " adds the xplain MCP server to Alpha config");
        assert_eq!(t[8], " y confirm  n/esc cancel");
        assert_eq!(t.len(), 9);
        let m = McpModal { confirm: Some((0, false)), ..m };
        let t = texts(&mcp_lines(&st, &m, &look()));
        assert_eq!(t[6], " Remove Alpha registration? (y/n)");
        assert_eq!(t[7], " removes the xplain MCP server from Alpha config");
    }

    #[test]
    fn f_mcpui_01_note_wrapped_max_two_rows_and_selected_message() {
        let mut st = mcp_state(2);
        st.integration_state[0].message = Some("last result".into());
        let m = McpModal { row: 1, ..Default::default() };
        let t = texts(&mcp_lines(&st, &m, &look()));
        assert_eq!(t[6], " last result");
        let m = McpModal { row: 1, note: Some("own note".into()), ..Default::default() };
        assert_eq!(texts(&mcp_lines(&st, &m, &look()))[6], " own note");
        let long = "word ".repeat(40);
        let m = McpModal { row: 1, note: Some(long), ..Default::default() };
        let l = mcp_lines(&st, &m, &look());
        let t = texts(&l);
        assert_eq!(t.len(), 9);
        assert_eq!(l[6][0].1, look().accent);
    }

    #[test]
    fn f_mcpui_01_preview_lines_and_ellipsis() {
        let st = mcp_state(2);
        let m = McpModal {
            preview: Some(("register command (copied)".into(), "short".into())),
            ..Default::default()
        };
        let t = texts(&mcp_lines(&st, &m, &look()));
        assert_eq!(t[6], " register command (copied):");
        assert_eq!(t[7], " short");
        assert_eq!(t[8], " j/k  enter register  d remove  c/w copy  R refresh  esc");
        // inner width 60: four 60-char lines -> 3 rows, third cut to 59 + ellipsis
        let text = ["a".repeat(60), "b".repeat(60), "c".repeat(60), "d".repeat(60)].join("\n");
        let m = McpModal { preview: Some(("p".into(), text)), ..Default::default() };
        let t = texts(&mcp_lines(&st, &m, &look()));
        assert_eq!(t[7], format!(" {}", "a".repeat(60)));
        assert_eq!(t[9], format!(" {}\u{2026}", "c".repeat(59)));
        assert_eq!(t.len(), 11);
        // blank third line gives ` …`
        let m = McpModal { preview: Some(("p".into(), "a\nb\n\nd".into())), ..Default::default() };
        let t = texts(&mcp_lines(&st, &m, &look()));
        assert_eq!(t[9], " \u{2026}");
    }

    #[test]
    fn f_mcpui_01_modal_drawn_centered_width_64() {
        let mut st = mcp_state(2);
        st.overlay = Overlay::Mcp(McpModal::default());
        let mut c = canvas(80, 24);
        // this overlay needs no other component
        draw(&mut c, &st, &theme());
        let s = c.into_screen();
        // 7 content rows + 2 -> h 9 at y = ceil(15/2) = 8; x = 8
        assert!(s.row_text(8).chars().skip(8).take(64).collect::<String>().starts_with('\u{256d}'));
        assert!(s.row_text(9).contains("\u{2502} MCP"));
        assert!(s.row_text(16).chars().skip(8).take(64).collect::<String>().ends_with('\u{256f}'));
        let x = cell_x(&s, 10, "> ");
        assert_eq!(s.rows[10][x].style.bg, Some(theme().sel_bg));
        assert_eq!(theme().sel_bg, rgb(0x073642));
        assert_eq!(s.rows[10][0].style.fg, None::<Color>);
    }

    #[test]
    fn wrap_text_rules() {
        assert_eq!(wrap_text("aa bb cc", 5), vec!["aa bb", "cc"]);
        assert_eq!(wrap_text("abcdefgh", 3), vec!["abc", "def", "gh"]);
        assert_eq!(wrap_text("a\n\nb\n\n", 5), vec!["a", "", "b"]);
        assert_eq!(wrap_text("a\tb", 10), vec!["a  b"]);
        assert_eq!(wrap_text("aaa   bbb", 5), vec!["aaa", "bbb"]);
    }
}
