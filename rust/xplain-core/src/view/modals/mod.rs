//! Modal boxes: file picker, file search, config, MCP, delete-comment, quit.
//!
//! Spec: F-LAYOUT-06/08 (placement, narrow), F-FILES-01, F-SEARCH-01 (hit highlights), F-CFGUI-01, F-MCPUI-01
//! (rows, status, clients, notes, preview, masking), F-COMMENT-08 (delete dialog text), F-QUIT-01 (quit dialog
//! text), F-THEME-02 (modal colors). Oracle: `src/components/{FileModal,SearchModal,ConfigModal,McpModal,
//! DeleteModal,QuitModal}.tsx`. Owner: component `viewframe` (F1). Reads data from `picker::entries`,
//! `search::hits`, `config_ui::rows`, `state.mcp`, `state.integration_state`. Must not mutate state.

use crate::canvas::{Canvas, Rect, center};
use crate::config_ui;
use crate::screen::{Seg, Size, Style, bold, fg, seg};
use crate::search;
use crate::state::{HelpLevel, Overlay, State};
use crate::theme::Theme;

mod config;
mod dialog;
mod mcp;
mod picker;
use crate::view::frame_height;
use crate::view::layout::{CONFIG_H, CONFIG_W, MCP_W, list_width};

/// Styles every modal draws with.
pub(super) struct Look {
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

/// Draw the open modal (if any) centered over the frame.
pub fn draw(c: &mut Canvas, state: &State, theme: &Theme) {
    let lk = Look::new(theme);
    let size = c.size();
    let help_open = state.help != HelpLevel::Closed;
    match &state.overlay {
        Overlay::Picker { sel } => {
            let entries = crate::picker::entries(state);
            let (x, y, w, h) = picker::picker_geometry(size, entries.len(), help_open);
            picker::draw_picker(c, &lk, &entries, *sel, state.nav.file_index, Rect { x, y, w, h });
        }
        Overlay::Search(s) => {
            let hits = search::hits(state);
            let rows_t = frame_height(size);
            let w = list_width(size.cols);
            let h = rows_t.min(8.max(rows_t * 7 / 10));
            let (x, y) = place(size, w, h, help_open);
            let hit_list: Vec<(String, Vec<usize>)> = hits.into_iter().map(|h| (h.path, h.idx)).collect();
            picker::draw_search(c, &lk, &s.query, &hit_list, s.sel, Rect { x, y, w, h });
        }
        Overlay::Config(m) => {
            let rows = config_ui::rows(state);
            let w = size.cols.min(CONFIG_W);
            let (x, y) = place(size, w, CONFIG_H, help_open);
            let committed = config::committed_values(state, m);
            config::draw_config(c, &lk, &rows, &committed, m, Rect { x, y, w, h: CONFIG_H });
        }
        Overlay::Mcp(m) => {
            let lines = mcp::mcp_lines(state, m, &lk);
            let w = size.cols.min(MCP_W);
            let h = lines.len() as u16 + 2;
            let (x, y) = place(size, w, h, help_open);
            let inner = open_box(c, &lk, x, y, w, h);
            for (k, l) in lines.iter().enumerate() {
                line(c, inner, k as u16, l);
            }
        }
        Overlay::DeleteComment { .. } => dialog::dialog(c, &lk, size, help_open, "Delete comment? (y/n)"),
        Overlay::Quit => dialog::dialog(c, &lk, size, help_open, "Quit xplain? (y/n)"),
        Overlay::None | Overlay::Find { .. } | Overlay::Goto { .. } | Overlay::Editor(_) => {}
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

#[cfg(test)]
pub(super) mod fixtures {
    use super::*;
    use crate::screen::Screen;
    use crate::view::testutil::theme;

    pub fn look() -> Look {
        Look::new(&theme())
    }

    pub fn canvas(cols: u16, rows: u16) -> Canvas {
        Canvas::new(Size { cols, rows })
    }

    pub fn cell_x(s: &Screen, row: usize, text: &str) -> usize {
        let t = s.row_text(row);
        t.find(text).map(|b| t[..b].chars().count()).unwrap_or(usize::MAX)
    }

    pub fn cols_text(s: &Screen, row: usize, from: usize, to: usize) -> String {
        s.row_text(row).chars().skip(from).take(to - from + 1).collect()
    }
}
