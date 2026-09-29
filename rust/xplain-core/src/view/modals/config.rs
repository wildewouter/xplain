//! Config modal (F-CFGUI-01).

use super::*;
use crate::config_ui::ConfigRow;
use crate::state::ConfigModal;

pub(super) const CONFIG_LABEL_W: usize = 14;

/// Committed value of each of the 6 config rows.
pub(super) fn committed_values(state: &State, m: &ConfigModal) -> [String; 6] {
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
pub(super) fn window_choices(choices: &[String], at: usize, avail: usize) -> (usize, usize) {
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
pub(super) fn draw_config(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::modals::fixtures::*;
    use crate::view::testutil::{state, theme};

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
}
