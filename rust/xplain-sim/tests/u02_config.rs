//! Ported from `e2e/scenarios/u02-config`.
//!
//! Warnings that the e2e suite read off the main screen after exit are checked on `Sim::stderr()` instead.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use regex::Regex;
use serde_json::{Value, json};
use xplain_sim::Sim;

// ---- helpers ---------------------------------------------------------------------------------------------------

/// Cell `offset` columns after the first `text` in `row`.
#[track_caller]
fn cell_in(s: &Sim, row: isize, text: &str, offset: usize) -> xplain_sim::CellView {
    let p = s.find_in_row(row, text).unwrap_or_else(|| panic!("{text:?} not in row {row}\n{}", s.dump()));
    s.cell(p.x + offset, p.y as isize)
}

#[track_caller]
fn assert_reverse(s: &Sim, row: isize, text: &str, offset: usize, want: bool) {
    let c = cell_in(s, row, text, offset);
    assert!(c.reverse == want, "{text:?} row {row}: reverse want {want}, got {}\n{}", c.reverse, s.dump());
}

#[track_caller]
fn assert_matches_all(s: &Sim, pats: &[&str]) {
    for p in pats {
        s.assert_matches(p);
    }
}

/// JSON subset match like the e2e `json:` matcher: objects need only the listed keys, arrays match element-wise,
/// `{"$exists": bool}` checks presence.
#[track_caller]
fn subset(actual: Option<&Value>, want: &Value, at: &str) {
    if let Some(m) = want.as_object() {
        if let Some(e) = m.get("$exists") {
            assert_eq!(actual.is_some(), e.as_bool().unwrap(), "{at}: $exists");
            return;
        }
        let a = actual.unwrap_or_else(|| panic!("{at}: missing"));
        for (k, v) in m {
            subset(a.get(k), v, &format!("{at}.{k}"));
        }
        return;
    }
    if let Some(w) = want.as_array() {
        let a = actual.and_then(Value::as_array).unwrap_or_else(|| panic!("{at}: not an array"));
        assert_eq!(a.len(), w.len(), "{at}: length");
        for (i, (x, y)) in a.iter().zip(w).enumerate() {
            subset(Some(x), y, &format!("{at}[{i}]"));
        }
        return;
    }
    assert_eq!(actual, Some(want), "{at}");
}

#[track_caller]
fn assert_json_file(s: &Sim, path: &str, want: Value) {
    let text = s.file(path);
    let v: Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path}: not JSON ({e}): {text}"));
    subset(Some(&v), &want, path);
}

#[track_caller]
fn assert_file_eq(s: &Sim, path: &str, want: &str) {
    assert_eq!(s.file(path), want, "{path}");
}

const CFG: &str = "${CONFIG}/xplain/config.json";

fn cfg(v: Value) -> Sim {
    Sim::builder().config_json(v).build()
}

// ---- F-CFGUI-01 ------------------------------------------------------------------------------------------------

#[test]
fn f_cfgui_01_choice_window() {
    let mut s = Sim::builder().build();
    s.keys("C");
    s.assert_row_matches(17, r"│> theme {11}\[solarized\] +vibrant +dull +› +│$");
    s.keys("l");
    s.assert_row_matches(17, r"│> theme {11}\[solarized\] +vibrant +dull +› +│$");
    assert_reverse(&s, 17, "vibrant", 0, true);
    s.keys("l");
    s.assert_row_matches(17, r"│> theme {9}‹ +vibrant +dull +contrast +› +│$");
    assert_reverse(&s, 17, "dull", 0, true);
    s.keys("l");
    s.assert_row_matches(17, r"│> theme {9}‹ +dull +contrast +colorblind +› +│$");
    s.keys("l");
    s.assert_row_matches(17, r"│> theme {9}‹ +contrast +colorblind +light +│$");
    s.keys("l");
    s.assert_row_matches(17, r"│> theme {9}‹ +contrast +colorblind +light +│$");
    assert_reverse(&s, 17, "light", 0, true);
    // clamped at the last choice
    s.keys("l");
    s.assert_row_matches(17, r"│> theme {9}‹ +contrast +colorblind +light +│$");
    assert_reverse(&s, 17, "light", 0, true);
    // back left to the committed value
    s.keys("hhhhh");
    s.assert_row_matches(17, r"│> theme {11}\[solarized\] +vibrant +dull +› +│$");
    assert_reverse(&s, 17, "[solarized]", 1, true);
}

#[test]
fn f_cfgui_01_current_state() {
    let mut s = Sim::builder()
        .args(["--unstaged"])
        .config_json(json!({"theme": "solarized", "app": {"confirmQuit": false}, "mcp": {"autostart": true}}))
        .build();
    s.assert_row_matches(0, r"^\[unstaged\] \[full\] \[unified\] \[solarized\] \[mcp: on\] ");
    // t cycles theme to vibrant, s split on, c changes scope
    s.keys("tsc");
    s.assert_row_matches(0, r"^\[unstaged\] \[changes\] \[split\] \[vibrant\] ");
    s.keys("C");
    assert_matches_all(
        &s,
        &[
            r"│> theme +solarized +\[vibrant\] +dull +› +│",
            r"│  mode +all +staged +\[unstaged\] +│",
            r"│  split +off +\[on\] +│",
            r"│  view +full +\[changes\] +│",
            r"│  confirm quit {4}\[off\] +on +│",
            r"│  mcp on startup +off +\[on\] +│",
        ],
    );
    // choice cursor on the current theme
    assert_reverse(&s, 17, "[vibrant]", 1, true);
    assert_reverse(&s, 17, "solarized", 0, false);
    // moving to the mode row puts its choice cursor on the current mode
    s.keys("j");
    assert_reverse(&s, 18, "[unstaged]", 1, true);
    assert_reverse(&s, 17, "[vibrant]", 1, false);
}

#[test]
fn f_cfgui_01_layout() {
    let mut s = Sim::builder().build();
    s.assert_not_contains("Config");
    s.keys("C");
    s.assert_row_matches(15, r"^ {32}\S{56}$");
    s.assert_row_matches(16, r"^ {32}│ Config {47}│$");
    s.assert_row_matches(17, r"^ {32}│> theme {11}\[solarized\] +vibrant +dull +› +│$");
    s.assert_row_matches(18, r"^ {32}│  mode {12}\[all\] +staged +unstaged +│$");
    s.assert_row_matches(19, r"^ {32}│  split {11}\[off\] +on +│$");
    s.assert_row_matches(20, r"^ {32}│  view {12}\[full\] +changes +│$");
    s.assert_row_matches(21, r"^ {32}│  confirm quit {4,}off +\[on\] +│$");
    s.assert_row_matches(22, r"^ {32}│  mcp on startup  \[off\] +on +│$");
    s.assert_row_matches(23, r"^ {32}│ j/k row h/l browse enter select esc close +│$");
    s.assert_row_matches(24, r"^ {32}\S{56}$");
    s.assert_row(25, "");
    s.assert_row(14, "");
    s.assert_not_contains("‹");
    s.assert_not_contains("contrast");
    // title bold
    assert!(cell_in(&s, 16, "Config", 0).bold);
    // selected row bg selBg, fg selFg (solarized)
    let c = cell_in(&s, 17, "> theme", 2);
    assert!(c.bg_is("#073642") && c.fg_is("#93a1a1"), "{c:?}");
    // other rows are not selected (modal bg)
    assert!(cell_in(&s, 18, "  mode", 2).bg_is("#002b36"));
    // choice cursor on current value, reverse, on selected row only
    assert_reverse(&s, 17, "[solarized]", 1, true);
    assert_reverse(&s, 17, "vibrant", 0, false);
    assert_reverse(&s, 18, "[all]", 1, false);
}

#[test]
fn f_cfgui_01_pre() {
    let mut s = Sim::builder().build();
    s.keys("q");
    s.assert_contains("Quit xplain? (y/n)");
    s.keys("C");
    s.assert_contains("Quit xplain? (y/n)");
    s.assert_not_contains(" Config");
    s.keys("n");
    s.assert_not_contains("Quit xplain?");
    s.assert_not_contains(" Config");
    s.keys("/");
    s.keys("C");
    // C typed into the find input
    s.assert_not_contains(" Config");
    s.assert_row_matches(-1, "^/C█");
    s.keys("<Esc>");
    s.keys("C");
    s.assert_contains(" Config");
}

// ---- F-CFGUI-02 ------------------------------------------------------------------------------------------------

#[test]
fn f_cfgui_02_choice_clamp() {
    let mut s = Sim::builder().build();
    s.keys("Cjj");
    assert_reverse(&s, 19, "[off]", 1, true);
    s.keys("h<Left>");
    assert_reverse(&s, 19, "[off]", 1, true);
    s.keys("<Right>");
    assert_reverse(&s, 19, "[off]", 1, false);
    assert_reverse(&s, 19, "on", 0, true);
    s.keys("ll");
    assert_reverse(&s, 19, "on", 0, true);
    // browsing only; split not applied, nothing saved
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] ");
    s.assert_row_matches(19, r"│> split +\[off\] +on +│");
    assert!(!s.file_exists(CFG));
}

#[test]
fn f_cfgui_02_confirm_quit() {
    let mut s = Sim::builder().build();
    s.keys("q");
    s.assert_contains("Quit xplain? (y/n)");
    s.keys("n");
    s.keys("Cjjjjh<Enter>");
    s.assert_row_matches(21, r"│> confirm quit {4}\[off\] +on +│");
    assert_json_file(&s, CFG, json!({"app": {"confirmQuit": false}}));
    s.keys("<Esc>");
    s.assert_not_contains(" Config");
    s.keys("q");
    assert_eq!(s.exit_code(), Some(0));
}

/// F-NAV-10: selecting the current view / split value keeps the cursor, resets top then follows.
#[test]
fn f_cfgui_02_current_value_viewport() {
    let mut s = Sim::builder().size(120, 12).build();
    s.keys("<Tab>G");
    s.assert_row_matches(0, r"\[cursor L60:C1\] src/big\.ts ");
    s.assert_row_matches(-1, r"^\(54-62/62\) ");
    s.keys("10k");
    s.assert_row_matches(0, r"\[cursor L50:C1\] src/big\.ts ");
    s.assert_row_matches(-1, r"^\(50-58/62\) ");
    // view row, Enter on current value full
    s.keys("Cjjj<Enter>");
    s.assert_row_matches(0, r"\[cursor L50:C1\] src/big\.ts ");
    s.assert_row_matches(-1, r"^\(46-54/62\) ");
    assert_json_file(&s, CFG, json!({"view": {"full": true}}));
    s.keys("<Esc>G10k");
    s.assert_row_matches(0, r"\[cursor L50:C1\] ");
    s.assert_row_matches(-1, r"^\(50-58/62\) ");
    // split row (k x5 clamps to theme first), Enter on current value off
    s.keys("Ckkkkkjj<Enter>");
    s.assert_row_matches(0, r"\[unified\] .*\[cursor L50:C1\] src/big\.ts ");
    s.assert_row_matches(-1, r"^\(46-54/62\) ");
    assert_json_file(&s, CFG, json!({"view": {"split": false, "full": true}}));
}

#[test]
fn f_cfgui_02_mcp_startup_off() {
    let mut s = cfg(json!({"mcp": {"autostart": true}}));
    s.assert_row_matches(0, r"\[mcp: on\] ");
    s.keys("Cjjjjjh<Enter>");
    s.assert_row_matches(0, r"\[mcp: on\] ");
    s.assert_row_matches(22, r"│> mcp on startup  \[off\] +on +│");
    assert_json_file(&s, CFG, json!({"mcp": {"autostart": false}}));
    assert_eq!(s.mcp_rpc("ping", json!({})).status, 200);
}

#[test]
fn f_cfgui_02_mcp_startup() {
    let mut s = Sim::builder().build();
    s.assert_row_matches(0, r"\[mcp: off\] ");
    s.keys("Cjjjjjl<Enter>");
    s.assert_row_matches(0, r"\[mcp: off\] ");
    s.assert_row_matches(22, r"│> mcp on startup +off +\[on\] +│");
    assert_json_file(&s, CFG, json!({"mcp": {"autostart": true}}));
    // no MCP token file, server never started
    assert!(!s.file_exists("${STATE}/xplain/mcp.json"));
}

#[test]
fn f_cfgui_02_mode() {
    let mut s = Sim::builder().build();
    s.keys("<Tab>");
    s.assert_row_matches(0, r"^\[all\] .* \[2/4\] \[cursor [^\]]+\] src/big\.ts ");
    s.keys("Cjll<Enter>");
    s.assert_contains(" Config");
    s.assert_row_matches(
        0,
        r"^\[unstaged\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/3\] \[cursor [^\]]+\] README\.md ",
    );
    s.assert_row_matches(18, r"│> mode +all +staged +\[unstaged\] +│");
    assert_json_file(&s, CFG, json!({"view": {"mode": "unstaged"}}));
    s.keys("<Esc><Tab>");
    s.assert_row_matches(0, r"^\[unstaged\] .* \[2/3\] \[cursor [^\]]+\] src/big\.ts ");
    // select the current value (unstaged)
    s.keys("Ckj<Enter>");
    s.assert_row_matches(0, r"^\[unstaged\] .* \[1/3\] \[cursor [^\]]+\] README\.md ");
    assert_json_file(&s, CFG, json!({"view": {"mode": "unstaged"}}));
    // staged has nothing in the fixture -> no-changes header
    s.keys("h<Enter>");
    s.assert_row_matches(0, r"^\[staged\] \[mcp: off\] No changes");
}

#[test]
fn f_cfgui_02_rows() {
    let mut s = Sim::builder().build();
    s.keys("C");
    s.assert_row_matches(17, "│> theme ");
    s.keys("k");
    s.assert_row_matches(17, "│> theme ");
    s.assert_row_matches(18, "│  mode ");
    s.keys("<Up>");
    s.assert_row_matches(17, "│> theme ");
    s.keys("j");
    s.assert_row_matches(17, "│  theme ");
    s.assert_row_matches(18, "│> mode ");
    s.keys("<Down>");
    s.assert_row_matches(18, "│  mode ");
    s.assert_row_matches(19, "│> split ");
    s.keys("jjj");
    s.assert_row_matches(21, "│  confirm quit ");
    s.assert_row_matches(22, "│> mcp on startup ");
    s.keys("j<Down>");
    s.assert_row_matches(22, "│> mcp on startup ");
    // selection bg follows the row
    assert!(cell_in(&s, 22, "> mcp on startup", 2).bg_is("#073642"));
    assert!(cell_in(&s, 17, "  theme", 2).bg_is("#002b36"));
    s.keys("<Up>k");
    s.assert_row_matches(20, "│> view ");
    s.assert_row_matches(22, "│  mcp on startup ");
}

#[test]
fn f_cfgui_02_split() {
    let mut s = Sim::builder().build();
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] ");
    assert!(!s.row(-1).contains("p pane"));
    s.keys("Cjjl<Enter>");
    s.assert_contains(" Config");
    s.assert_row_matches(0, r"^\[all\] \[full\] \[split\] ");
    s.assert_row_matches(3, "│");
    s.assert_row_matches(19, r"│> split +off +\[on\] +│");
    s.assert_row_matches(-1, r"hjkl move  enter ask  J/K comments  p pane  \? help$");
    assert_json_file(&s, CFG, json!({"view": {"split": true}}));
    s.keys("h<Enter>");
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] ");
    s.assert_row_matches(-1, r"\(1-5/5\) hjkl move  enter ask  J/K comments  \? help$");
    assert_json_file(&s, CFG, json!({"view": {"split": false}}));
}

#[test]
fn f_cfgui_02_theme_preview() {
    let mut s = Sim::builder().build();
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[solarized\] ");
    assert!(cell_in(&s, 0, "[solarized]", 1).fg_is("#6c71c4"));
    s.keys("Cl");
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[vibrant\] ");
    s.assert_row_matches(17, r"│> theme +\[solarized\] +vibrant ");
    // preview really switches the theme (view chip color magenta in vibrant)
    assert!(cell_in(&s, 0, "[vibrant]", 1).fg_is("5"));
    s.keys("<Right>");
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[dull\] ");
    s.keys("<Esc>");
    s.assert_not_contains(" Config");
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[solarized\] ");
    assert!(cell_in(&s, 0, "[solarized]", 1).fg_is("#6c71c4"));
    s.keys("Clll");
    s.assert_row_matches(0, r"\[contrast\] ");
    // q closes the modal (does not quit)
    s.keys("q");
    s.assert_not_contains(" Config");
    s.assert_not_contains("Quit xplain");
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[solarized\] ");
    s.keys("Ch");
    // h at first choice stays on solarized
    s.assert_row_matches(0, r"\[solarized\] ");
    s.keys("l<Left>l");
    s.assert_row_matches(0, r"\[vibrant\] ");
    s.keys("C");
    s.assert_not_contains(" Config");
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[solarized\] ");
    assert!(!s.file_exists(CFG));
}

#[test]
fn f_cfgui_02_theme_select() {
    let mut s = Sim::builder().build();
    s.keys("Cl<Enter>");
    s.assert_contains(" Config");
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[vibrant\] ");
    s.assert_row_matches(17, r"│> theme +solarized +\[vibrant\] +dull ");
    assert_json_file(&s, CFG, json!({"theme": "vibrant"}));
    s.keys("<Esc>");
    s.assert_not_contains(" Config");
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[vibrant\] ");
    s.keys("Cl<Space>");
    s.assert_contains(" Config");
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[dull\] ");
    s.assert_row_matches(17, r"│> theme .*vibrant +\[dull\] ");
    assert_json_file(&s, CFG, json!({"theme": "dull"}));
    // preview after commit reverts to the new committed theme
    s.keys("l<Esc>");
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[dull\] ");
}

#[test]
fn f_cfgui_02_view() {
    let mut s = Sim::builder().build();
    s.keys("<Tab>");
    s.assert_row_matches(0, r"^\[all\] \[full\] .* \[2/4\] \[cursor [^\]]+\] src/big\.ts ");
    s.assert_row_matches(-1, r"/62\) ");
    s.keys("Cjjjl<Enter>");
    s.assert_contains(" Config");
    s.assert_row_matches(0, r"^\[all\] \[changes\] .* \[2/4\] \[cursor [^\]]+\] src/big\.ts ");
    s.assert_row_matches(20, r"│> view +full +\[changes\] +│");
    assert!(!s.row(-1).contains("/62) "));
    assert_json_file(&s, CFG, json!({"view": {"full": false}}));
    s.keys("h<Enter>");
    s.assert_row_matches(0, r"^\[all\] \[full\] .* \[2/4\] \[cursor [^\]]+\] src/big\.ts ");
    s.assert_row_matches(-1, r"/62\) ");
    assert_json_file(&s, CFG, json!({"view": {"full": true}}));
}

// ---- F-CFGUI-03 ------------------------------------------------------------------------------------------------

#[test]
fn f_cfgui_03_broken_json() {
    let mut s = Sim::builder().size(300, 20).config("{ not json\n").build();
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[solarized\] ");
    s.assert_row_matches(-1, r"^\(1-5/5\) ");
    s.keys("Cl<Enter>");
    s.assert_contains(" Config");
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[vibrant\] ");
    s.assert_row(
        -1,
        &s.expand(
            "config unreadable, not saved (${CONFIG}/xplain/config.json) | (1-5/5) hjkl move  enter ask  J/K comments  ? help",
        ),
    );
    assert_file_eq(&s, CFG, "{ not json\n");
    // theme stays applied after closing
    s.keys("<Esc>");
    s.assert_not_contains(" Config");
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[vibrant\] ");
    // another setting, same note, applied live
    s.keys("Cjjjl<Enter>");
    s.assert_row_matches(0, r"^\[all\] \[changes\] \[unified\] \[vibrant\] ");
    s.assert_row_matches(-1, r"^config unreadable, not saved \(.*config\.json\) \| ");
    assert_file_eq(&s, CFG, "{ not json\n");
}

#[test]
fn f_cfgui_03_top_level_null() {
    let mut s = Sim::builder().size(300, 20).config("null\n").build();
    s.keys("Cjjjjh<Enter>");
    s.assert_row_matches(-1, r"^config unreadable, not saved \(.*config\.json\) \| ");
    s.assert_row_contains(-1, &s.expand("(${CONFIG}/xplain/config.json)"));
    assert_file_eq(&s, CFG, "null\n");
    s.keys("<Esc>q");
    assert_eq!(s.exit_code(), Some(0));
}

// ---- F-CONFIG-01 -----------------------------------------------------------------------------------------------

#[test]
fn f_config_01_cmd_env_empty() {
    let s = Sim::builder().env("XPLAIN_CONFIG", "").args(["config", "path"]).build();
    assert_eq!(s.exit_code(), Some(0));
    assert_eq!(s.stdout(), s.expand("${CONFIG}/xplain/config.json\n"));
}

#[test]
fn f_config_01_cmd_env() {
    let s = Sim::builder().env("XPLAIN_CONFIG", "${TMP}/env/cfg.json").args(["config", "path"]).build();
    assert_eq!(s.exit_code(), Some(0));
    assert_eq!(s.stdout(), s.expand("${TMP}/env/cfg.json\n"));
}

#[test]
fn f_config_01_cmd_flag_empty_eq() {
    let s = Sim::builder()
        .env("XPLAIN_CONFIG", "${TMP}/env/cfg.json")
        .args(["--config=", "config", "path"])
        .build();
    assert_eq!(s.exit_code(), Some(0));
    assert_eq!(s.stdout(), s.expand("${TMP}/env/cfg.json\n"));
}

#[test]
fn f_config_01_cmd_flag_empty_space() {
    let s = Sim::builder().args(["--config", "", "config", "path"]).build();
    assert_eq!(s.exit_code(), Some(0));
    assert_eq!(s.stdout(), s.expand("${CONFIG}/xplain/config.json\n"));
}

#[test]
fn f_config_01_cmd_flag_eq() {
    let s = Sim::builder()
        .env("XPLAIN_CONFIG", "${TMP}/env/cfg.json")
        .args(["--config=${TMP}/flag/cfg.json", "config", "path"])
        .build();
    assert_eq!(s.exit_code(), Some(0));
    assert_eq!(s.stdout(), s.expand("${TMP}/flag/cfg.json\n"));
}

#[test]
fn f_config_01_cmd_flag() {
    let s = Sim::builder()
        .env("XPLAIN_CONFIG", "${TMP}/env/cfg.json")
        .args(["--config", "${TMP}/flag/cfg.json", "config", "path"])
        .build();
    assert_eq!(s.exit_code(), Some(0));
    assert_eq!(s.stdout(), s.expand("${TMP}/flag/cfg.json\n"));
}

#[test]
fn f_config_01_cmd_home() {
    let s = Sim::builder().env_unset("XDG_CONFIG_HOME").args(["config", "path"]).build();
    assert_eq!(s.exit_code(), Some(0));
    assert_eq!(s.stdout(), s.expand("${HOME}/.config/xplain/config.json\n"));
}

#[test]
fn f_config_01_cmd_xdg_empty() {
    let s = Sim::builder().env("XDG_CONFIG_HOME", "").args(["config", "path"]).build();
    assert_eq!(s.exit_code(), Some(0));
    assert_eq!(s.stdout(), s.expand("${HOME}/.config/xplain/config.json\n"));
}

#[test]
fn f_config_01_cmd_xdg() {
    let s = Sim::builder().args(["config", "path"]).build();
    assert_eq!(s.exit_code(), Some(0));
    assert_eq!(s.stdout(), s.expand("${CONFIG}/xplain/config.json\n"));
}

#[test]
fn f_config_01_read_env_empty() {
    let s = Sim::builder().env("XPLAIN_CONFIG", "").config(r#"{"theme": "vibrant"}"#).build();
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[vibrant\] \[mcp: off\] ");
}

#[test]
fn f_config_01_read_env() {
    let mut s = Sim::builder()
        .env("XPLAIN_CONFIG", "${TMP}/env/cfg.json")
        .file("${TMP}/env/cfg.json", r#"{"theme": "contrast"}"#)
        .file(CFG, "{\"theme\": \"vibrant\"}\n")
        .build();
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[contrast\] \[mcp: off\] ");
    // theme row, one right of contrast = colorblind, select
    s.keys("Cl<Enter>");
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[colorblind\] ");
    assert_json_file(&s, "${TMP}/env/cfg.json", json!({"theme": "colorblind"}));
    assert_file_eq(&s, CFG, "{\"theme\": \"vibrant\"}\n");
}

#[test]
fn f_config_01_read_flag_empty_space() {
    let s = Sim::builder()
        .env("XPLAIN_CONFIG", "${TMP}/env/cfg.json")
        .args(["--config", ""])
        .file("${TMP}/env/cfg.json", r#"{"theme": "contrast"}"#)
        .file(CFG, r#"{"theme": "vibrant"}"#)
        .build();
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[contrast\] \[mcp: off\] ");
}

#[test]
fn f_config_01_read_flag_empty() {
    let s = Sim::builder()
        .env("XPLAIN_CONFIG", "${TMP}/env/cfg.json")
        .args(["--config="])
        .file("${TMP}/env/cfg.json", r#"{"theme": "contrast"}"#)
        .file(CFG, r#"{"theme": "vibrant"}"#)
        .build();
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[contrast\] \[mcp: off\] ");
}

#[test]
fn f_config_01_read_flag_eq() {
    let s = Sim::builder()
        .env("XPLAIN_CONFIG", "${TMP}/env/cfg.json")
        .args(["--config=${TMP}/flag/cfg.json"])
        .file("${TMP}/flag/cfg.json", r#"{"theme": "light"}"#)
        .file("${TMP}/env/cfg.json", r#"{"theme": "contrast"}"#)
        .build();
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[light\] \[mcp: off\] ");
}

#[test]
fn f_config_01_read_flag() {
    let mut s = Sim::builder()
        .env("XPLAIN_CONFIG", "${TMP}/env/cfg.json")
        .args(["--config", "${TMP}/flag/cfg.json"])
        .file("${TMP}/flag/cfg.json", r#"{"theme": "light"}"#)
        .file("${TMP}/env/cfg.json", "{\"theme\": \"contrast\"}\n")
        .file(CFG, "{\"theme\": \"vibrant\"}\n")
        .build();
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[light\] \[mcp: off\] ");
    // theme row, one left of light = colorblind, select
    s.keys("Ch<Enter>");
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[colorblind\] ");
    assert_json_file(&s, "${TMP}/flag/cfg.json", json!({"theme": "colorblind"}));
    assert_file_eq(&s, "${TMP}/env/cfg.json", "{\"theme\": \"contrast\"}\n");
    assert_file_eq(&s, CFG, "{\"theme\": \"vibrant\"}\n");
}

#[test]
fn f_config_01_read_home() {
    let s = Sim::builder()
        .env_unset("XDG_CONFIG_HOME")
        .file(CFG, r#"{"theme": "vibrant"}"#)
        .file("${HOME}/.config/xplain/config.json", r#"{"theme": "dull"}"#)
        .build();
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[dull\] \[mcp: off\] ");
}

#[test]
fn f_config_01_read_xdg_empty() {
    let s = Sim::builder()
        .env("XDG_CONFIG_HOME", "")
        .file(CFG, r#"{"theme": "vibrant"}"#)
        .file("${HOME}/.config/xplain/config.json", r#"{"theme": "dull"}"#)
        .build();
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[dull\] \[mcp: off\] ");
}

#[test]
fn f_config_01_read_xdg() {
    let s = Sim::builder()
        .file(CFG, r#"{"theme": "vibrant"}"#)
        .file("${HOME}/.config/xplain/config.json", r#"{"theme": "dull"}"#)
        .build();
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[vibrant\] \[mcp: off\] ");
}

// ---- F-CONFIG-02 -----------------------------------------------------------------------------------------------

#[test]
fn f_config_02_all_values() {
    let mut s = cfg(json!({
        "version": 1,
        "theme": "light",
        "view": {"mode": "unstaged", "split": true, "full": false},
        "app": {"confirmQuit": false},
        "mcp": {"autostart": true}
    }));
    s.assert_row_matches(0, r"^\[unstaged\] \[changes\] \[split\] \[light\] \[mcp: on\] \[1/3\] ");
    s.assert_row_matches(-1, r"hjkl move  enter ask  J/K comments  p pane  \? help$");
    s.keys("C");
    assert_matches_all(
        &s,
        &[
            r"│> theme +‹ .*\[light\] +│",
            r"│  mode +all +staged +\[unstaged\] +│",
            r"│  split +off +\[on\] +│",
            r"│  view +full +\[changes\] +│",
            r"│  confirm quit {4}\[off\] +on +│",
            r"│  mcp on startup +off +\[on\] +│",
        ],
    );
    s.keys("<Esc>");
    // confirmQuit false, q quits at once
    s.keys("q");
    assert_eq!(s.exit_code(), Some(0));
    assert_eq!(s.stderr(), "");
}

#[test]
fn f_config_02_defaults() {
    let mut s = Sim::builder().build();
    assert!(!s.file_exists(CFG));
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/4\] ");
    s.keys("C");
    // committed values in the config modal are the defaults
    assert_matches_all(
        &s,
        &[
            r"│> theme +\[solarized\] +vibrant ",
            r"│  mode +\[all\] +staged +unstaged +│",
            r"│  view +\[full\] +changes +│",
            r"│  split +\[off\] +on +│",
            r"│  confirm quit +off +\[on\] +│",
            r"│  mcp on startup  \[off\] +on +│",
        ],
    );
    s.keys("<Esc>");
    s.assert_not_contains(" Config");
    // confirmQuit defaults to true
    s.keys("q");
    s.assert_contains("Quit xplain? (y/n)");
    s.keys("y");
    assert_eq!(s.exit_code(), Some(0));
    // no warning
    assert_eq!(s.stderr(), "");
    assert!(!s.file_exists(CFG));
}

#[test]
fn f_config_02_flags_override() {
    let s = Sim::builder()
        .args(["--mode", "all", "--theme", "dull", "--split", "--changes-only"])
        .config_json(json!({"theme": "light", "view": {"mode": "unstaged", "split": false, "full": true}}))
        .build();
    s.assert_row_matches(0, r"^\[all\] \[changes\] \[split\] \[dull\] \[mcp: off\] \[1/4\] ");
}

#[test]
fn f_config_02_ignored_keys() {
    let mut s = cfg(json!({
        "version": 99,
        "agent": {"name": "whatever"},
        "keys": {"q": "x", "C": "z"},
        "foo": [1, 2],
        "theme": "dull"
    }));
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[dull\] \[mcp: off\] ");
    // keys section does not remap; C still opens config
    s.keys("C");
    s.assert_contains(" Config");
    s.assert_matches(r"│> theme .*\[dull\]");
    s.keys("<Esc>");
    s.keys("q");
    s.assert_contains("Quit xplain? (y/n)");
    s.keys("y");
    assert_eq!(s.exit_code(), Some(0));
    assert_eq!(s.stderr(), "");
}

// ---- F-CONFIG-03 -----------------------------------------------------------------------------------------------

#[test]
fn f_config_03_absent_keys() {
    let mut s = cfg(json!({"view": {}, "app": {}, "mcp": {}}));
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/4\] ");
    s.keys("q");
    s.assert_contains("Quit xplain? (y/n)");
    s.keys("y");
    assert_eq!(s.exit_code(), Some(0));
    assert_eq!(s.stderr(), "");
}

#[test]
fn f_config_03_each_warning() {
    let mut s = Sim::builder()
        .size(160, 30)
        .config_json(
            json!({"mcp": 3, "app": [], "view": {"full": "no", "split": null, "mode": 1}, "theme": "x"}),
        )
        .build();
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/4\] ");
    // app invalid -> confirmQuit default true
    s.keys("q");
    s.assert_contains("Quit xplain? (y/n)");
    s.keys("y");
    assert_eq!(s.exit_code(), Some(0));
    assert_eq!(
        s.stderr(),
        "xplain: config: invalid theme \"x\" (solarized|vibrant|dull|contrast|colorblind|light); using solarized\n\
         xplain: config: invalid view.mode 1 (all|staged|unstaged); using all\n\
         xplain: config: invalid view.split null (boolean); using false\n\
         xplain: config: invalid view.full \"no\" (boolean); using true\n\
         xplain: config: app must be an object; using defaults\n\
         xplain: config: mcp must be an object; using defaults\n"
    );
}

#[test]
fn f_config_03_rest_applied() {
    let mut s = Sim::builder()
        .size(160, 30)
        .config_json(json!({
            "theme": 5,
            "view": {"mode": "unstaged", "split": "yes", "full": false},
            "app": {"confirmQuit": false},
            "mcp": {"autostart": "on"}
        }))
        .build();
    s.assert_row_matches(0, r"^\[unstaged\] \[changes\] \[unified\] \[solarized\] \[mcp: off\] \[1/3\] ");
    // confirmQuit false applied, q quits at once
    s.keys("q");
    assert_eq!(s.exit_code(), Some(0));
    assert_eq!(
        s.stderr(),
        "xplain: config: invalid theme 5 (solarized|vibrant|dull|contrast|colorblind|light); using solarized\n\
         xplain: config: invalid view.split \"yes\" (boolean); using false\n\
         xplain: config: invalid mcp.autostart \"on\" (boolean); using false\n"
    );
}

#[test]
fn f_config_03_view_not_object() {
    let mut s = Sim::builder()
        .size(160, 30)
        .config_json(json!({
            "view": [{"mode": "bogus", "split": 1}],
            "app": {"confirmQuit": 0},
            "mcp": {"autostart": null}
        }))
        .build();
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/4\] ");
    s.keys("q");
    s.assert_contains("Quit xplain? (y/n)");
    s.keys("y");
    assert_eq!(s.exit_code(), Some(0));
    assert_eq!(
        s.stderr(),
        "xplain: config: view must be an object; using defaults\n\
         xplain: config: invalid app.confirmQuit 0 (boolean); using true\n\
         xplain: config: invalid mcp.autostart null (boolean); using false\n"
    );
}

#[test]
fn f_config_03_view_string() {
    let mut s = Sim::builder()
        .size(160, 30)
        .config_json(json!({"theme": "vibrant", "view": "split", "mcp": [true]}))
        .build();
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[vibrant\] \[mcp: off\] ");
    s.keys("qy");
    assert_eq!(s.exit_code(), Some(0));
    assert_eq!(
        s.stderr(),
        "xplain: config: view must be an object; using defaults\n\
         xplain: config: mcp must be an object; using defaults\n"
    );
}

// ---- F-CONFIG-04 -----------------------------------------------------------------------------------------------

/// One warning line: `xplain: config: <path>: <read/parse message>; using defaults, file will not be modified`.
/// The message between prefix and suffix is UNSPEC-29.
#[track_caller]
fn assert_unreadable_warning(s: &Sim) {
    let path = regex::escape(&s.expand(CFG));
    let re =
        Regex::new(&format!(r"\Axplain: config: {path}: .+; using defaults, file will not be modified\n\z"))
            .unwrap();
    assert!(re.is_match(s.stderr()), "stderr: {:?}", s.stderr());
}

#[test]
fn f_config_04_directory() {
    let mut s = Sim::builder().size(300, 20).file("${CONFIG}/xplain/config.json/keep", "x\n").build();
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/4\] ");
    s.keys("qy");
    assert_eq!(s.exit_code(), Some(0));
    assert_unreadable_warning(&s);
}

#[test]
fn f_config_04_empty() {
    let mut s = Sim::builder().size(300, 20).config("").build();
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/4\] ");
    s.keys("q");
    s.assert_contains("Quit xplain? (y/n)");
    s.keys("y");
    assert_eq!(s.exit_code(), Some(0));
    assert_unreadable_warning(&s);
    assert_file_eq(&s, CFG, "");
}

#[test]
fn f_config_04_invalid_json() {
    let mut s = Sim::builder().size(300, 20).config("{\"theme\": \"dull\",}\n").build();
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/4\] ");
    s.keys("q");
    s.assert_contains("Quit xplain? (y/n)");
    s.keys("y");
    assert_eq!(s.exit_code(), Some(0));
    assert_unreadable_warning(&s);
    assert_file_eq(&s, CFG, "{\"theme\": \"dull\",}\n");
}

/// Top level not an object: warning, defaults used, file never rewritten.
#[track_caller]
fn top_level_not_object(content: &str) {
    let mut s = Sim::builder().size(300, 20).config(content).build();
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/4\] ");
    // select theme vibrant; save must fail and leave the file alone
    s.keys("Cl<Enter>");
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[vibrant\] ");
    assert_file_eq(&s, CFG, content);
    s.keys("<Esc>q");
    s.assert_contains("Quit xplain? (y/n)");
    s.keys("y");
    assert_eq!(s.exit_code(), Some(0));
    assert_eq!(
        s.stderr(),
        s.expand("xplain: config: ${CONFIG}/xplain/config.json: top level must be an object; using defaults, file will not be modified\n")
    );
}

#[test]
fn f_config_04_top_array() {
    top_level_not_object("[{\"theme\": \"dull\"}]\n");
}

#[test]
fn f_config_04_top_null() {
    top_level_not_object("null\n");
}

#[test]
fn f_config_04_top_number() {
    top_level_not_object("5\n");
}

#[test]
fn f_config_04_top_string() {
    top_level_not_object("\"dull\"\n");
}

// ---- F-CONFIG-05 -----------------------------------------------------------------------------------------------

#[test]
fn f_config_05_fresh_default_path() {
    let mut s = Sim::builder().build();
    assert!(!s.file_exists(CFG));
    s.keys("Cll<Enter>");
    assert_file_eq(&s, CFG, "{\n\t\"version\": 1,\n\t\"theme\": \"dull\"\n}\n");
}

#[test]
fn f_config_05_fresh_file() {
    let mut s = Sim::builder().args(["--config", "${TMP}/new/deep/cfg.json"]).build();
    assert!(!s.file_exists("${TMP}/new"));
    // theme row, two right = dull, select
    s.keys("Cll<Enter>");
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[dull\] ");
    assert_file_eq(&s, "${TMP}/new/deep/cfg.json", "{\n\t\"version\": 1,\n\t\"theme\": \"dull\"\n}\n");
}

#[test]
fn f_config_05_merge() {
    let mut s = cfg(json!({
        "version": 3,
        "agent": {"name": "someone", "opts": [1, 2]},
        "keys": {"q": "z"},
        "foo": "bar",
        "theme": "vibrant",
        "view": {"mode": "all", "extra": true},
        "app": {"confirmQuit": true, "other": 7}
    }));
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[vibrant\] ");
    // split row, choose on
    s.keys("Cjjl<Enter>");
    s.assert_row_matches(0, r"^\[all\] \[full\] \[split\] \[vibrant\] ");
    let text = s.file(CFG);
    assert!(text.starts_with("{\n\t\""), "{text}");
    assert!(text.contains("\n\t\t\"split\": true"), "{text}");
    assert!(text.ends_with("\n}\n"), "{text}");
    assert_json_file(
        &s,
        CFG,
        json!({
            "version": 3,
            "agent": {"name": "someone", "opts": [1, 2]},
            "keys": {"q": "z"},
            "foo": "bar",
            "theme": "vibrant",
            "view": {"mode": "all", "extra": true, "split": true},
            "app": {"confirmQuit": true, "other": 7}
        }),
    );
    // confirm quit row, choose off
    s.keys("jjh<Enter>");
    assert_json_file(
        &s,
        CFG,
        json!({
            "version": 3,
            "foo": "bar",
            "view": {"mode": "all", "extra": true, "split": true},
            "app": {"confirmQuit": false, "other": 7}
        }),
    );
}

#[test]
fn f_config_05_note_cleared() {
    let mut s = Sim::builder()
        .size(300, 20)
        .args(["--config", "${TMP}/plain/cfg.json"])
        .file("${TMP}/plain", "i am a file\n")
        .build();
    s.assert_row_matches(-1, r"^\(1-5/5\) hjkl move");
    // Enter on the current theme; parent path is a file, save fails and sets a note
    s.keys("C<Enter>");
    s.assert_contains(" Config");
    s.assert_row_matches(-1, r"^config save failed: .+ \| \(1-5/5\) ");
    assert_file_eq(&s, "${TMP}/plain", "i am a file\n");
    s.remove_file("${TMP}/plain");
    // same select again now succeeds and clears the note
    s.keys("<Enter>");
    s.assert_contains(" Config");
    s.assert_row_matches(-1, r"^\(1-5/5\) hjkl move");
    assert_file_eq(&s, "${TMP}/plain/cfg.json", "{\n\t\"version\": 1,\n\t\"theme\": \"solarized\"\n}\n");
}

#[test]
fn f_config_05_patch_per_setting() {
    let mut s = Sim::builder().build();
    s.keys("Clll<Enter>");
    assert_json_file(
        &s,
        CFG,
        json!({"version": 1, "theme": "contrast", "view": {"$exists": false}, "app": {"$exists": false}, "mcp": {"$exists": false}}),
    );
    // mode row, unstaged
    s.keys("jll<Enter>");
    assert_json_file(
        &s,
        CFG,
        json!({"version": 1, "theme": "contrast", "view": {"mode": "unstaged", "split": {"$exists": false}, "full": {"$exists": false}}}),
    );
    // split row, on
    s.keys("jl<Enter>");
    assert_json_file(
        &s,
        CFG,
        json!({"view": {"mode": "unstaged", "split": true, "full": {"$exists": false}}}),
    );
    // view row, changes
    s.keys("jl<Enter>");
    assert_json_file(
        &s,
        CFG,
        json!({"view": {"mode": "unstaged", "split": true, "full": false}, "app": {"$exists": false}}),
    );
    // confirm quit row, off
    s.keys("jh<Enter>");
    assert_json_file(&s, CFG, json!({"app": {"confirmQuit": false}, "mcp": {"$exists": false}}));
    // mcp on startup row, on
    s.keys("jl<Enter>");
    let text = s.file(CFG);
    assert!(text.starts_with("{\n\t\"") && text.ends_with("\n}\n"), "{text}");
    assert_json_file(
        &s,
        CFG,
        json!({
            "version": 1,
            "theme": "contrast",
            "view": {"mode": "unstaged", "split": true, "full": false},
            "app": {"confirmQuit": false},
            "mcp": {"autostart": true}
        }),
    );
    // saved values are read back on the next start (see config_02_all_values); here the live state
    s.assert_row_matches(0, r"^\[unstaged\] \[changes\] \[split\] \[contrast\] \[mcp: off\] ");
}

/// Restores permissions so the temp dir can be removed even when an assertion fails.
struct Restore(std::path::PathBuf);
impl Drop for Restore {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
    }
}

#[test]
fn f_config_05_save_denied() {
    use std::os::unix::fs::PermissionsExt;
    let mut s = Sim::builder()
        .size(300, 20)
        .args(["--config", "${TMP}/ro/cfg.json"])
        .file("${TMP}/ro/keep.txt", "x\n")
        .build();
    let ro = s.path("${TMP}/ro");
    let _restore = Restore(ro.clone());
    std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o555)).unwrap();
    s.keys("Cll<Enter>");
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[dull\] ");
    let pat = format!(
        r"^config save failed: {}/ro/cfg\.json: permission denied \| \(1-5/5\) ",
        regex::escape(&s.expand("${TMP}"))
    );
    s.assert_row_matches(-1, &pat);
    std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o755)).unwrap();
    // nothing written, no temp file left
    assert!(s.list_dir("${TMP}/ro").iter().all(|n| !n.starts_with("cfg")), "{:?}", s.list_dir("${TMP}/ro"));
}

#[test]
fn f_config_05_save_failed() {
    let mut s = Sim::builder()
        .size(300, 20)
        .args(["--config", "${TMP}/plain/cfg.json"])
        .file("${TMP}/plain", "i am a file\n")
        .build();
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[solarized\] ");
    s.keys("Cll<Enter>");
    // mkdir of the parent fails, parent path is a regular file
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[dull\] ");
    let pat = format!(
        r"^config save failed: {}/plain/cfg\.json: not a directory \| \(1-5/5\) ",
        regex::escape(&s.expand("${TMP}"))
    );
    s.assert_row_matches(-1, &pat);
    assert_file_eq(&s, "${TMP}/plain", "i am a file\n");
}

#[test]
fn f_config_05_unreadable() {
    let mut s = Sim::builder().size(300, 20).config("[\"not\", \"an object\"]\n").build();
    s.keys("Cjjl<Enter>");
    s.assert_row_matches(0, r"^\[all\] \[full\] \[split\] \[solarized\] ");
    s.assert_row_contains(-1, &s.expand("config unreadable, not saved (${CONFIG}/xplain/config.json) | "));
    assert_file_eq(&s, CFG, "[\"not\", \"an object\"]\n");
}
