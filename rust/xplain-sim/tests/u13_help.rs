//! Ported from `e2e/scenarios/u13-help`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use regex::Regex;
use serde_json::{Value, json};
use xplain_sim::{CellExpect as C, Fixture, Sim};

fn j(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|e| panic!("bad json {text}: {e}"))
}

/// Multiline regex over the screen text where `(?P<wI_N>..)` groups mark whole lines that must be exactly N
/// chars wide (the e2e regexes used a look-ahead for that; the regex crate has none).
#[track_caller]
fn assert_wide(s: &Sim, pattern: &str) {
    s.assert_matches(pattern);
    let text = s.text();
    let re = Regex::new(&format!("(?m){pattern}")).unwrap();
    let caps = re.captures(&text).unwrap();
    for name in re.capture_names().flatten() {
        let Some(w) =
            name.strip_prefix('w').and_then(|n| n.split('_').nth(1)).and_then(|n| n.parse::<usize>().ok())
        else {
            continue;
        };
        let got = caps.name(name).map_or(0, |m| m.as_str().chars().count());
        assert_eq!(got, w, "line {name} width\n{}", s.dump());
    }
}

/// F-HELP-01: text input (find) with items cut: no hint, no credit; last row is the more row
#[test]
fn f_help_01_credit_text_input_cut() {
    let mut s = Sim::builder().size(80, 10).build();
    s.keys("?/");
    s.assert_row_matches(-1, "^/█");
    s.assert_not_contains("Made by");
    s.assert_not_contains("? close");
    s.assert_not_contains("? move keys");
    s.assert_matches(r"│ Help · Find in file *│$\n^[^\n]*│ Find *│$\n^[^\n]*│  type      search text *│$\n^[^\n]*│  backspace delete char *│$\n^[^\n]*│ … 2 more *│$\n^[^\n]*╰─+╯$");
}

/// F-HELP-01: credit shown only when inner width >= hint len + 1 + 30 (70 cols: L1 no, L2 yes)
#[test]
fn f_help_01_credit() {
    let mut s = Sim::builder().size(70, 40).build();
    s.keys("?");
    s.assert_not_contains("Made by");
    s.assert_matches(r"│ \? move keys *│$\n^[^\n]*╰─+╯$");
    s.keys("?");
    s.assert_matches(r"│ \? close +Made by Wouter de Wild - 2026 │$\n^[^\n]*╰─+╯$");
}

/// F-HELP-01: contexts without motion keys (Visual selection, Confirm): ? cycles closed -> level 1 -> closed; hint ? close
#[test]
fn f_help_01_cycle_no_level2() {
    let mut s = Sim::builder().size(120, 40).build();
    s.keys("v");
    s.assert_row_contains(0, "[visual L2:C1]");
    s.assert_not_contains("Help ·");
    s.keys("?");
    s.assert_not_contains("? move keys");
    s.assert_matches(r"│ Help · Visual selection *│$\n^[^\n]*│ Selection *│$");
    s.assert_matches(r"│ \? close +Made by");
    s.keys("?");
    // closed again, visual still active
    s.assert_row_contains(0, "[visual L2:C1]");
    s.assert_not_contains("Help ·");
    s.keys("<Esc>");
    s.assert_row_contains(0, "[cursor L2:C1]");
    s.keys("q");
    s.assert_contains("Quit xplain? (y/n)");
    s.assert_not_contains("Help ·");
    s.keys("?");
    s.assert_contains("Quit xplain? (y/n)");
    s.assert_contains("Help · Confirm");
    s.assert_contains("? close");
    s.keys("?");
    s.assert_contains("Quit xplain? (y/n)");
    s.assert_not_contains("Help ·");
}

/// F-HELP-01: ? cycles closed -> level 1 -> level 2 -> closed in Diff view (context with motion keys)
#[test]
fn f_help_01_cycle() {
    let mut s = Sim::builder().size(120, 40).build();
    s.assert_not_contains("Help ·");
    s.keys("?");
    s.assert_not_contains("Move");
    s.assert_matches(r"│ Help · Diff view *│$\n^[^\n]*│ Find *│$");
    s.assert_matches(r"│ \? move keys +Made by");
    s.keys("?");
    s.assert_not_contains("? move keys");
    s.assert_matches(r"│ Help · Diff view *│$\n^[^\n]*│ Move *│$");
    s.assert_matches(r"│ \? close +Made by");
    s.keys("?");
    s.assert_row_matches(-1, r"^\(1-5/5\) hjkl move  enter ask  J/K comments  \? help$");
    s.assert_not_contains("Help ·");
    s.assert_not_contains("Made by");
    s.keys("?");
    // cycle starts again at level 1
    s.assert_matches(r"│ Help · Diff view *│$\n^[^\n]*│ Find *│$");
}

/// F-HELP-01: spec example 80x24 Diff view L1: bottom-right, bottom border row above footer, last row hint + credit; colors (solarized)
#[test]
fn f_help_01_layout_80x24() {
    let mut s = Sim::builder().size(80, 24).build();
    s.keys("?");
    assert_eq!(s.row(2).chars().count(), 80, "row 2 width\n{}", s.dump());
    s.assert_row_matches(2, "^.*╭─+╮$");
    assert_eq!(s.row(3).chars().count(), 80, "row 3 width\n{}", s.dump());
    s.assert_row_matches(3, "^.*│ Help · Diff view +│$");
    assert_eq!(s.row(21).chars().count(), 80, "row 21 width\n{}", s.dump());
    s.assert_row_matches(21, r"^.*│ \? move keys +Made by Wouter de Wild - 2026 │$");
    assert_eq!(s.row(22).chars().count(), 80, "row 22 width\n{}", s.dump());
    s.assert_row_matches(22, "^.*╰─+╯$");
    s.assert_row_matches(23, r"^\(1-5/5\) hjkl move  enter ask  J/K comments  \? help$");
    // round border in modalBorder (border cell bg not pinned by spec)
    {
        let p = s.find_in_row(22, "╰").unwrap_or_else(|| panic!("text not in row\n{}", s.dump()));
        s.assert_cell(p.x, 22, C::new().fg("#268bd2"));
    }
    {
        let p = s.find_in_row(2, "╮").unwrap_or_else(|| panic!("text not in row\n{}", s.dump()));
        s.assert_cell(p.x, 2, C::new().fg("#268bd2"));
    }
    // title bold
    {
        let p = s.find_in_row(3, "Help ·").unwrap_or_else(|| panic!("text not in row\n{}", s.dump()));
        s.assert_cell(p.x, 3, C::new().bg("#002b36").bold(true));
    }
    // group title accent
    {
        let p = s.find_in_row(4, "Find").unwrap_or_else(|| panic!("text not in row\n{}", s.dump()));
        s.assert_cell(p.x, 4, C::new().fg("#cb4b16").bg("#002b36"));
    }
    // key column mode color, description modal fg
    {
        let p = s.find_in_row(5, "]/[").unwrap_or_else(|| panic!("text not in row\n{}", s.dump()));
        s.assert_cell(p.x, 5, C::new().fg("#b58900").bg("#002b36"));
    }
    {
        let p =
            s.find_in_row(5, "next/prev change").unwrap_or_else(|| panic!("text not in row\n{}", s.dump()));
        s.assert_cell(p.x, 5, C::new().fg("#93a1a1").bg("#002b36"));
    }
    // hint and credit dim
    {
        let p = s.find_in_row(21, "? move keys").unwrap_or_else(|| panic!("text not in row\n{}", s.dump()));
        s.assert_cell(p.x, 21, C::new().fg("#586e75"));
    }
    {
        let p = s.find_in_row(21, "Made by").unwrap_or_else(|| panic!("text not in row\n{}", s.dump()));
        s.assert_cell(p.x, 21, C::new().fg("#586e75"));
    }
}

/// F-HELP-01: tiny R=6 (M=3): border, title, border only; top on screen row 3
#[test]
fn f_help_01_max_height_r6() {
    let mut s = Sim::builder().size(80, 6).build();
    s.keys("?");
    s.assert_row_matches(0, r"^\[all\] ");
    s.assert_row_matches(1, "^─+$");
    assert_eq!(s.row(2).chars().count(), 80, "row 2 width\n{}", s.dump());
    s.assert_row_matches(2, "^.*╭─+╮$");
    s.assert_row_matches(3, "│ Help · Diff view +│$");
    assert_eq!(s.row(4).chars().count(), 80, "row 4 width\n{}", s.dump());
    s.assert_row_matches(4, "^.*╰─+╯$");
    s.assert_row_matches(5, r"^\(\d+-\d+/5\) hjkl move  enter ask  J/K comments  \? help$");
    s.assert_not_contains("more");
    s.assert_not_contains("? move keys");
    s.assert_not_contains("Made by");
}

/// F-HELP-01: tiny R=7 (M=4): border, title, … <all items> more, border; no bottom row; top on screen row 3
#[test]
fn f_help_01_max_height_r7() {
    let mut s = Sim::builder().size(80, 7).build();
    s.keys("?");
    s.assert_row_matches(0, r"^\[all\] ");
    s.assert_row_matches(1, "^─+$");
    assert_eq!(s.row(2).chars().count(), 80, "row 2 width\n{}", s.dump());
    s.assert_row_matches(2, "^.*╭─+╮$");
    s.assert_row_matches(3, "│ Help · Diff view +│$");
    s.assert_row_matches(4, "│ … 14 more +│$");
    assert_eq!(s.row(5).chars().count(), 80, "row 5 width\n{}", s.dump());
    s.assert_row_matches(5, "^.*╰─+╯$");
    s.assert_row_matches(6, r"^\(\d+-\d+/5\) hjkl move  enter ask  J/K comments  \? help$");
    s.assert_not_contains("? move keys");
    s.assert_not_contains("Made by");
}

/// F-HELP-01: max height max(3, R-3): 80x10 box is 7 rows, top on screen row 3 (0-based 2), bottom on row above footer
#[test]
fn f_help_01_max_height() {
    let mut s = Sim::builder().size(80, 10).build();
    s.keys("?");
    s.assert_row_matches(1, "^─+$");
    assert_eq!(s.row(2).chars().count(), 80, "row 2 width\n{}", s.dump());
    s.assert_row_matches(2, "^.*╭─+╮$");
    s.assert_row_matches(3, "│ Help · Diff view +│$");
    s.assert_row_matches(4, "│ Find +│$");
    s.assert_row_matches(5, r"│  \]/\[       next/prev change +│$");
    s.assert_row_matches(6, "│ … 13 more +│$");
    s.assert_row_matches(7, r"│ \? move keys +Made by Wouter de Wild - 2026 │$");
    assert_eq!(s.row(8).chars().count(), 80, "row 8 width\n{}", s.dump());
    s.assert_row_matches(8, "^.*╰─+╯$");
    s.assert_row_matches(9, r"^\(1-5/5\) hjkl move  enter ask  J/K comments  \? help$");
}

/// F-HELP-01: overflow keeps whole items only: 2-row wrapped item that does not fit is left out entirely (40x13)
#[test]
fn f_help_01_overflow_whole_cut() {
    let mut s = Sim::builder().size(40, 13).build();
    s.keys("??");
    s.assert_row_matches(-1, r"^\(1-5/5\) ");
    s.assert_not_contains("PgDn");
    s.assert_not_contains("down)");
    s.assert_matches(
        r"│  d/u       half page down/up *│$\n^[^\n]*│ … 17 more *│$\n^[^\n]*│ \? close *│$\n^[^\n]*╰─+╯$",
    );
}

/// F-HELP-01: overflow keeps whole items only: one more row lets the 2-row wrapped item in (40x14)
#[test]
fn f_help_01_overflow_whole_fit() {
    let mut s = Sim::builder().size(40, 14).build();
    s.keys("??");
    s.assert_not_contains("first / last line");
    s.assert_matches(r"│  PgDn/PgUp page down/up \(space: *│$\n^[^\n]*│ +down\) *│$\n^[^\n]*│ … 16 more *│$\n^[^\n]*│ \? close *│$");
}

/// F-HELP-01: overflow at 80x24 L2 (max height R-3 = 21, top on screen row 3): whole items then " … <k> more" (dim)
#[test]
fn f_help_01_overflow() {
    let mut s = Sim::builder().size(80, 24).build();
    s.keys("??");
    s.assert_row_matches(1, "^─+$");
    assert_eq!(s.row(2).chars().count(), 80, "row 2 width\n{}", s.dump());
    s.assert_row_matches(2, "^.*╭─+╮$");
    s.assert_row_matches(3, "│ Help · Diff view +│$");
    s.assert_row_matches(4, "│ Move +│$");
    assert_eq!(s.row(22).chars().count(), 80, "row 22 width\n{}", s.dump());
    s.assert_row_matches(22, "^.*╰─+╯$");
    s.assert_not_contains("numbered, any file");
    s.assert_not_contains("export comments");
    s.assert_not_contains("General");
    s.assert_not_contains("quit");
    s.assert_matches(r"│  a         comment on line *│$\n^[^\n]*│ … 7 more *│$\n^[^\n]*│ \? close +Made by Wouter de Wild - 2026 │$");
    {
        let p = s.find_in_row(20, "… 7 more").unwrap_or_else(|| panic!("text not in row\n{}", s.dump()));
        s.assert_cell(p.x, 20, C::new().fg("#586e75"));
    }
}

/// F-HELP-01: with panel open other keys reach the app, panel follows context live; Esc does not close it
#[test]
fn f_help_01_passthrough() {
    let mut s = Sim::builder().size(120, 40).build();
    s.keys("?");
    s.assert_row_contains(0, "[cursor L2:C1] README.md");
    s.assert_matches(r"│ Help · Diff view *│$\n^[^\n]*│ Find *│$");
    s.keys("jj");
    // cursor moved, panel still open at level 1
    s.assert_row_contains(0, "[cursor L3:C1] README.md");
    s.assert_matches(r"│ Help · Diff view *│$\n^[^\n]*│ Find *│$");
    s.keys("v");
    s.assert_row_contains(0, "[visual L3:C1]");
    s.assert_matches(r"│ Help · Visual selection *│$\n^[^\n]*│ Selection *│$");
    s.keys("<Esc>");
    // Esc ended visual (reached app), panel stays open, back to Diff view
    s.assert_row_contains(0, "[cursor L3:C1]");
    s.assert_matches(r"│ Help · Diff view *│$\n^[^\n]*│ Find *│$");
    s.keys("<Esc>");
    s.assert_matches(r"│ Help · Diff view *│$\n^[^\n]*│ Find *│$");
    s.keys("FREADME<Enter>");
    s.assert_row_matches(0, r"^\[browse\] ");
    s.assert_matches(r"│ Help · File viewer *│$\n^[^\n]*│ File viewer *│$");
    s.keys("<Esc>");
    // Esc left browse (app handled it), panel still open
    s.assert_row_matches(0, r"^\[all\] ");
    s.assert_matches(r"│ Help · Diff view *│$\n^[^\n]*│ Find *│$");
    s.keys("ahi<Enter>");
    s.assert_contains("question saved (1)");
    s.assert_matches(r"│ Help · Diff view *│$\n^[^\n]*│ Find *│$");
    s.keys("J");
    s.assert_contains("▸ sent  line L2");
    s.assert_matches(r"│ Help · Focused comment *│$\n^[^\n]*│ Comments *│$");
    s.keys("<Esc>");
    s.assert_not_contains("▸ sent");
    s.assert_matches(r"│ Help · Diff view *│$\n^[^\n]*│ Find *│$");
}

/// F-HELP-01: opening a text input keeps the panel open and switches it to that context; closing returns to Diff view
#[test]
fn f_help_01_text_input_keeps_open() {
    let mut s = Sim::builder().size(120, 40).build();
    s.keys("?");
    s.assert_matches(r"│ Help · Diff view *│$\n^[^\n]*│ Find *│$");
    s.keys("/");
    s.assert_row_matches(-1, "^/█");
    s.assert_contains("Help · Find in file");
    s.keys("<Esc>");
    s.assert_row_matches(-1, r"^\(1-5/5\) hjkl move  enter ask  J/K comments  \? help$");
    s.assert_matches(r"│ Help · Diff view *│$\n^[^\n]*│ Find *│$");
    s.keys(":");
    s.assert_row_matches(-1, "^:█");
    s.assert_contains("Help · Go to line");
    s.keys("<Esc>");
    s.assert_matches(r"│ Help · Diff view *│$\n^[^\n]*│ Find *│$");
    s.keys("F");
    s.assert_contains("Help · File search");
    s.assert_contains("Search (1/6)");
    s.keys("<Esc>");
    s.assert_not_contains("Search (");
    s.assert_matches(r"│ Help · Diff view *│$\n^[^\n]*│ Find *│$");
    s.keys("a");
    s.assert_contains("Help · Editor");
    s.assert_contains("[save] enter send");
    s.keys("<Esc>");
    s.assert_not_contains("enter send");
    s.assert_matches(r"│ Help · Diff view *│$\n^[^\n]*│ Find *│$");
}

/// F-HELP-01: ? is typed (panel not toggled) in editor, find, goto, search; with panel open ? in editor is typed and panel stays
#[test]
fn f_help_01_typed_in_inputs() {
    let mut s = Sim::builder().size(80, 24).build();
    s.keys("a?");
    s.assert_row_matches(6, r"^│ \? +│$");
    s.assert_not_contains("Help ·");
    s.keys("<Esc>");
    s.assert_not_contains("Help ·");
    s.assert_not_contains("enter send");
    s.keys(":?");
    s.assert_row_matches(-1, r"^:\?█\(1-5/5\) ");
    s.assert_not_contains("Help ·");
    s.keys("<Esc>");
    s.keys("/?");
    s.assert_row_matches(-1, r"^/\?█\(1-5/5\) ");
    s.assert_not_contains("Help ·");
    s.keys("<Esc>");
    s.keys("F?");
    s.assert_contains("Search (0/0)");
    s.assert_contains("│ > ?");
    s.assert_not_contains("Help ·");
    s.keys("<Esc>");
    s.assert_row_matches(-1, r"^\(1-5/5\) hjkl move  enter ask  J/K comments  \? help$");
    s.assert_not_contains("Search (");
    s.assert_not_contains("Help ·");
    s.keys("?a");
    s.assert_row_matches(6, "^│ +│$");
    s.assert_contains("Help · Editor");
    s.keys("?");
    // typed into editor, panel neither closed nor cycled
    s.assert_row_matches(6, r"^│ \? +│$");
    s.assert_contains("Help · Editor");
}

/// F-HELP-01: width floor(cols*0.6) = 38 at 64 cols when content is wider; description wraps
#[test]
fn f_help_01_width_60pct() {
    let mut s = Sim::builder().size(64, 40).build();
    s.keys("??");
    s.assert_row_matches(-2, "^ {26}╰─{36}╯$");
    s.assert_not_contains("(space: down)");
    s.assert_matches(r"│  PgDn/PgUp page down/up \(space: *│$\n^[^\n]*│ +down\) *│$");
}

/// F-HELP-01: width min 36 at 40 cols (floor(cols*0.6) = 24); long description word-wrapped; no credit when it does not fit
#[test]
fn f_help_01_width_min() {
    let mut s = Sim::builder().size(40, 40).build();
    s.keys("??");
    s.assert_row(-2, "    ╰──────────────────────────────────╯");
    s.assert_row_matches(-3, r"^    │ \? close +│$");
    s.assert_not_contains("(space: down)");
    s.assert_not_contains("Made by");
    s.assert_matches(r"│  PgDn/PgUp page down/up \(space: *│$\n^[^\n]*│ +down\) *│$");
}

/// F-HELP-02: browse = File viewer; overlays over browse win: visual, focused comment, find, goto, search, MCP, config, confirm, editor
#[test]
fn f_help_02_browse() {
    let mut s = Sim::builder().size(120, 40).build();
    s.keys("FREADME<Enter>?");
    s.assert_row_matches(0, r"^\[browse\] ");
    s.assert_contains("Help · File viewer");
    s.keys("v");
    s.assert_contains("Help · Visual selection");
    s.assert_not_contains("Help · File viewer");
    s.keys("<Esc>");
    s.assert_row_matches(0, r"^\[browse\] ");
    s.assert_contains("Help · File viewer");
    s.keys("/");
    s.assert_contains("Help · Find in file");
    s.keys("<Esc>");
    s.keys(":");
    s.assert_contains("Help · Go to line");
    s.keys("<Esc>");
    s.keys("F");
    s.assert_contains("Help · File search");
    s.assert_contains("Search (");
    s.keys("<Esc>");
    s.keys("M");
    s.assert_contains("Help · MCP");
    s.assert_contains("INTEGRATIONS");
    s.keys("<Esc>");
    s.keys("C");
    s.assert_contains("Help · Config");
    s.assert_contains("mcp on startup");
    s.keys("<Esc>");
    s.keys("q");
    s.assert_contains("Help · Confirm");
    s.assert_contains("Quit xplain? (y/n)");
    s.keys("n");
    s.assert_contains("Help · File viewer");
    s.assert_not_contains("Quit xplain?");
    // f is a no-op in browse, no picker
    s.keys("f");
    s.assert_contains("Help · File viewer");
    s.assert_not_contains("Files (");
    s.keys("a");
    s.assert_contains("Help · Editor");
    s.keys("hi<Enter>");
    s.assert_contains("Help · File viewer");
    s.assert_contains("question saved (1)");
    s.keys("J");
    s.assert_contains("Help · Focused comment");
    s.assert_contains("▸ sent  line L1");
    s.keys("<Esc>");
    s.assert_contains("Help · File viewer");
    s.assert_not_contains("▸ sent");
}

/// F-HELP-02: diff view with cursor is labelled Diff view (no Cursor mode context)
#[test]
fn f_help_02_diff_label() {
    let mut s = Sim::builder().size(120, 40).build();
    s.assert_row_contains(0, "[cursor L2:C1]");
    s.keys("?");
    s.assert_contains("Help · Diff view");
    s.assert_not_contains("Cursor mode");
    s.assert_not_contains("Help · File viewer");
    s.keys("l");
    s.assert_row_contains(0, "[cursor L2:C2]");
    s.assert_contains("Help · Diff view");
    s.assert_not_contains("Cursor mode");
}

/// F-HELP-02: modal labels over diff: File picker, MCP, Config, Confirm, File search
#[test]
fn f_help_02_modals() {
    let mut s = Sim::builder().size(120, 40).build();
    s.keys("?");
    s.assert_contains("Help · Diff view");
    s.keys("f");
    s.assert_contains("Help · File picker");
    s.assert_contains("Files (1/4)");
    s.keys("<Esc>");
    s.keys("M");
    s.assert_contains("Help · MCP");
    s.assert_contains("INTEGRATIONS");
    s.keys("<Esc>");
    s.keys("C");
    s.assert_contains("Help · Config");
    s.assert_contains("mcp on startup");
    s.keys("<Esc>");
    s.keys("q");
    s.assert_contains("Help · Confirm");
    s.assert_contains("Quit xplain? (y/n)");
    s.keys("n");
    s.keys("F");
    s.assert_contains("Help · File search");
    s.assert_contains("Search (1/6)");
    s.keys("<Esc>");
    s.assert_contains("Help · Diff view");
    s.assert_not_contains("Search (");
}

/// F-HELP-02: no-changes screen uses Diff view context
#[test]
fn f_help_02_no_changes() {
    let mut s = Sim::builder().size(80, 24).fixture(Fixture::Empty).build();
    s.assert_row_contains(0, "No changes");
    s.assert_not_contains("Help ·");
    s.keys("?");
    s.assert_contains("Help · Diff view");
    s.assert_not_contains("Cursor mode");
}

/// F-HELP-02: editor, find, dialog, picker beat focused comment; focused comment back after they close
#[test]
fn f_help_02_priority_focused() {
    let mut s = Sim::builder().size(120, 40).build();
    s.keys("ahi<Enter>J?");
    s.assert_contains("Help · Focused comment");
    s.assert_contains("▸ sent  line L2");
    s.keys("e");
    s.assert_contains("Help · Editor");
    s.assert_contains("edit line L2");
    s.keys("<Esc>");
    s.assert_contains("Help · Focused comment");
    s.assert_contains("▸ sent  line L2");
    s.keys("D");
    s.assert_contains("Help · Confirm");
    s.assert_contains("Delete comment? (y/n)");
    s.keys("n");
    s.assert_contains("Help · Focused comment");
    s.assert_not_contains("Delete comment?");
    s.keys("f");
    s.assert_contains("Help · File picker");
    s.assert_contains("Files (1/4)");
    s.keys("<Esc>");
    s.assert_contains("Help · Focused comment");
    s.assert_not_contains("Files (");
    s.keys("/");
    s.assert_row_matches(-1, "^/█");
    s.assert_contains("Help · Find in file");
    s.keys("<Esc>");
    s.assert_contains("Help · Focused comment");
}

/// F-HELP-02: find, goto, editor beat visual selection; Visual selection back after find/goto close
#[test]
fn f_help_02_priority_visual() {
    let mut s = Sim::builder().size(120, 40).build();
    s.keys("v?");
    s.assert_row_contains(0, "[visual L2:C1]");
    s.assert_contains("Help · Visual selection");
    s.keys("/");
    s.assert_row_matches(-1, "^/█");
    s.assert_contains("Help · Find in file");
    s.keys("<Esc>");
    s.assert_row_contains(0, "[visual L2:C1]");
    s.assert_contains("Help · Visual selection");
    s.keys(":");
    s.assert_row_matches(-1, "^:█");
    s.assert_contains("Help · Go to line");
    s.keys("<Esc>");
    s.assert_contains("Help · Visual selection");
    s.keys("a");
    s.assert_contains("Help · Editor");
    s.assert_contains("selection L2:C1-C1");
}

/// F-HELP-03: Config entries verbatim: L1 [Config (C)] h/l, ⏎/space, esc/q/C; L2 [Move] j/k first
#[test]
fn f_help_03_config() {
    let mut s = Sim::builder().size(120, 40).build();
    s.keys("C");
    s.assert_contains("mcp on startup");
    s.assert_not_contains("Help ·");
    s.keys("?");
    s.assert_contains("mcp on startup");
    s.assert_not_contains("│  i ");
    s.assert_not_contains("Move");
    assert_wide(
        &s,
        r"^(?P<w1_120>[^\n]*│ Help · Config *│)$\n^[^\n]*│ Config \(C\) *│$\n^[^\n]*│  h/l     change value *│$\n^[^\n]*│  ⏎/space toggle / apply *│$\n^[^\n]*│  esc/q/C close *│$\n^[^\n]*│ \? move keys +Made by Wouter de Wild - 2026 │$\n^(?P<w2_120>[^\n]*╰─+╯)$",
    );
    s.keys("?");
    s.assert_contains("mcp on startup");
    s.assert_not_contains("│  i ");
    s.assert_not_contains("? move keys");
    assert_wide(
        &s,
        r"^(?P<w1_120>[^\n]*│ Help · Config *│)$\n^[^\n]*│ Move *│$\n^[^\n]*│  j/k     select setting *│$\n^[^\n]*│ Config \(C\) *│$\n^[^\n]*│  h/l     change value *│$\n^[^\n]*│  ⏎/space toggle / apply *│$\n^[^\n]*│  esc/q/C close *│$\n^[^\n]*│ \? close +Made by Wouter de Wild - 2026 │$\n^(?P<w2_120>[^\n]*╰─+╯)$",
    );
}

/// F-HELP-03: Confirm entries verbatim (quit and delete modal): [Dialogs] y/Enter, n/esc; no L2
#[test]
fn f_help_03_confirm() {
    let mut s = Sim::builder().size(120, 40).build();
    s.keys("q");
    s.assert_contains("Quit xplain? (y/n)");
    s.assert_not_contains("Help ·");
    s.keys("?");
    s.assert_contains("Quit xplain? (y/n)");
    s.assert_not_contains("│  i ");
    s.assert_not_contains("Move");
    s.assert_not_contains("? move keys");
    assert_wide(
        &s,
        r"^(?P<w1_120>[^\n]*│ Help · Confirm *│)$\n^[^\n]*│ Dialogs *│$\n^[^\n]*│  y/Enter confirm *│$\n^[^\n]*│  n/esc   cancel \(q: quit\) *│$\n^[^\n]*│ \? close +Made by Wouter de Wild - 2026 │$\n^(?P<w2_120>[^\n]*╰─+╯)$",
    );
    s.keys("?");
    s.assert_contains("Quit xplain? (y/n)");
    s.assert_not_contains("Help ·");
    s.keys("n");
    s.assert_not_contains("Quit xplain?");
    s.assert_not_contains("Help ·");
    s.keys("ahi<Enter>JD");
    s.assert_contains("Delete comment? (y/n)");
    s.assert_not_contains("Help ·");
    s.keys("?");
    s.assert_contains("Delete comment? (y/n)");
    assert_wide(
        &s,
        r"^(?P<w1_120>[^\n]*│ Help · Confirm *│)$\n^[^\n]*│ Dialogs *│$\n^[^\n]*│  y/Enter confirm *│$\n^[^\n]*│  n/esc   cancel \(q: quit\) *│$\n^[^\n]*│ \? close +Made by Wouter de Wild - 2026 │$\n^(?P<w2_120>[^\n]*╰─+╯)$",
    );
}

/// F-HELP-03: Diff view with split effective (--split, 120 cols): same L1 entries incl. p
#[test]
fn f_help_03_diff_split() {
    let mut s = Sim::builder().size(120, 40).args(["--split"]).build();
    s.assert_row_matches(0, r"^\[all\] \[full\] \[split\] .*\[cursor new L2:C1\] README\.md ");
    s.assert_not_contains("Help ·");
    s.keys("?");
    s.assert_not_contains("│  i ");
    s.assert_not_contains("│  J/K");
    assert_wide(
        &s,
        r"^(?P<w1_120>[^\n]*│ Help · Diff view *│)$\n^[^\n]*│ Find *│$\n^[^\n]*│  \]/\[       next/prev change *│$\n^[^\n]*│  / n/N     find, next/prev *│$\n^[^\n]*│  :         go to line *│$\n^[^\n]*│  tab/S-tab next / prev file *│$\n^[^\n]*│  f/F       file picker / search *│$\n^[^\n]*│ Comments *│$\n^[^\n]*│  v/V       select chars/lines *│$\n^[^\n]*│  a         comment on line *│$\n^[^\n]*│  \)/\(       numbered, any file *│$\n^[^\n]*│  E         export comments *│$\n^[^\n]*│ General *│$\n^[^\n]*│  s/c/m     split, full, staged *│$\n^[^\n]*│  t/r       theme / reload *│$\n^[^\n]*│  p         old/new pane *│$\n^[^\n]*│  M/C       MCP / config *│$\n^[^\n]*│  q         quit *│$\n^[^\n]*│ \? move keys +Made by Wouter de Wild - 2026 │$\n^(?P<w2_120>[^\n]*╰─+╯)$",
    );
}

/// F-HELP-03: Diff view entries verbatim: L1 [Find] [Comments] [General], L2 [Move] first then L1 groups; no i, J/K, ? entries; p listed with split not effective
#[test]
fn f_help_03_diff() {
    let mut s = Sim::builder().size(120, 40).build();
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] .*\[cursor L2:C1\] README\.md ");
    s.assert_row_matches(-1, r"^\(1-5/5\) hjkl move  enter ask  J/K comments  \? help$");
    s.assert_not_contains("Help ·");
    s.keys("?");
    // level 1
    s.assert_not_contains("│  i ");
    s.assert_not_contains("│  J/K");
    s.assert_not_contains("│  ?");
    s.assert_not_contains("Move");
    s.assert_not_contains("word fwd/back/end");
    assert_wide(
        &s,
        r"^(?P<w1_120>[^\n]*│ Help · Diff view *│)$\n^[^\n]*│ Find *│$\n^[^\n]*│  \]/\[       next/prev change *│$\n^[^\n]*│  / n/N     find, next/prev *│$\n^[^\n]*│  :         go to line *│$\n^[^\n]*│  tab/S-tab next / prev file *│$\n^[^\n]*│  f/F       file picker / search *│$\n^[^\n]*│ Comments *│$\n^[^\n]*│  v/V       select chars/lines *│$\n^[^\n]*│  a         comment on line *│$\n^[^\n]*│  \)/\(       numbered, any file *│$\n^[^\n]*│  E         export comments *│$\n^[^\n]*│ General *│$\n^[^\n]*│  s/c/m     split, full, staged *│$\n^[^\n]*│  t/r       theme / reload *│$\n^[^\n]*│  p         old/new pane *│$\n^[^\n]*│  M/C       MCP / config *│$\n^[^\n]*│  q         quit *│$\n^[^\n]*│ \? move keys +Made by Wouter de Wild - 2026 │$\n^(?P<w2_120>[^\n]*╰─+╯)$",
    );
    s.keys("?");
    // level 2
    s.assert_not_contains("│  i ");
    s.assert_not_contains("│  J/K");
    s.assert_not_contains("│  ?");
    s.assert_not_contains("? move keys");
    assert_wide(
        &s,
        r"^(?P<w1_120>[^\n]*│ Help · Diff view *│)$\n^[^\n]*│ Move *│$\n^[^\n]*│  w/b/e     word fwd/back/end *│$\n^[^\n]*│  0/\^/\$     start/nonblank/end *│$\n^[^\n]*│  d/u       half page down/up *│$\n^[^\n]*│  PgDn/PgUp page down/up \(space: down\) *│$\n^[^\n]*│  g/G       first / last line *│$\n^[^\n]*│  1-9       count \(5j, 12G\) *│$\n^[^\n]*│ Find *│$\n^[^\n]*│  \]/\[       next/prev change *│$\n^[^\n]*│  / n/N     find, next/prev *│$\n^[^\n]*│  :         go to line *│$\n^[^\n]*│  tab/S-tab next / prev file *│$\n^[^\n]*│  f/F       file picker / search *│$\n^[^\n]*│ Comments *│$\n^[^\n]*│  v/V       select chars/lines *│$\n^[^\n]*│  a         comment on line *│$\n^[^\n]*│  \)/\(       numbered, any file *│$\n^[^\n]*│  E         export comments *│$\n^[^\n]*│ General *│$\n^[^\n]*│  s/c/m     split, full, staged *│$\n^[^\n]*│  t/r       theme / reload *│$\n^[^\n]*│  p         old/new pane *│$\n^[^\n]*│  M/C       MCP / config *│$\n^[^\n]*│  q         quit *│$\n^[^\n]*│ \? close +Made by Wouter de Wild - 2026 │$\n^(?P<w2_120>[^\n]*╰─+╯)$",
    );
}

/// F-HELP-03: Editor entries verbatim: [Editor] type, tab, left/right, backspace; no hint, credit only
#[test]
fn f_help_03_editor() {
    let mut s = Sim::builder().size(120, 40).build();
    s.keys("?");
    s.assert_contains("Help · Diff view");
    s.assert_not_contains("[save] enter send");
    s.keys("a");
    s.assert_contains("[save] enter send  tab save/ask  esc cancel");
    s.assert_not_contains("│  i ");
    s.assert_not_contains("? move keys");
    s.assert_not_contains("? close");
    s.assert_not_contains("Help · Diff view");
    assert_wide(
        &s,
        r"^(?P<w1_120>[^\n]*│ Help · Editor *│)$\n^[^\n]*│ Editor *│$\n^[^\n]*│  type       comment text *│$\n^[^\n]*│  tab        save / ask agent *│$\n^[^\n]*│  left/right move cursor *│$\n^[^\n]*│  backspace  delete char *│$\n^[^\n]*│ *Made by Wouter de Wild - 2026 │$\n^(?P<w2_120>[^\n]*╰─+╯)$",
    );
}

/// F-HELP-03: File viewer entries verbatim: L1 [File viewer] esc, s/c/m/f + [Comments]; L2 [Move] (6 as Diff view) first
#[test]
fn f_help_03_fileviewer() {
    let mut s = Sim::builder().size(120, 40).build();
    s.keys("FREADME<Enter>");
    s.assert_row_matches(0, r"^\[browse\] .*\[cursor L1:C1\] README\.md$");
    s.assert_not_contains("Help ·");
    s.keys("?");
    s.assert_not_contains("│  i ");
    s.assert_not_contains("│  J/K");
    s.assert_not_contains("Move");
    s.assert_not_contains("Find");
    s.assert_not_contains("General");
    assert_wide(
        &s,
        r"^(?P<w1_120>[^\n]*│ Help · File viewer *│)$\n^[^\n]*│ File viewer *│$\n^[^\n]*│  esc     back to diff *│$\n^[^\n]*│  s/c/m/f diff-only, no-op *│$\n^[^\n]*│ Comments *│$\n^[^\n]*│  v/V     select chars/lines *│$\n^[^\n]*│  a       comment on line *│$\n^[^\n]*│  \)/\(     numbered, any file *│$\n^[^\n]*│ \? move keys +Made by Wouter de Wild - 2026 │$\n^(?P<w2_120>[^\n]*╰─+╯)$",
    );
    s.keys("?");
    s.assert_not_contains("│  i ");
    s.assert_not_contains("│  J/K");
    s.assert_not_contains("? move keys");
    assert_wide(
        &s,
        r"^(?P<w1_120>[^\n]*│ Help · File viewer *│)$\n^[^\n]*│ Move *│$\n^[^\n]*│  w/b/e     word fwd/back/end *│$\n^[^\n]*│  0/\^/\$     start/nonblank/end *│$\n^[^\n]*│  d/u       half page down/up *│$\n^[^\n]*│  PgDn/PgUp page down/up \(space: down\) *│$\n^[^\n]*│  g/G       first / last line *│$\n^[^\n]*│  1-9       count \(5j, 12G\) *│$\n^[^\n]*│ File viewer *│$\n^[^\n]*│  esc       back to diff *│$\n^[^\n]*│  s/c/m/f   diff-only, no-op *│$\n^[^\n]*│ Comments *│$\n^[^\n]*│  v/V       select chars/lines *│$\n^[^\n]*│  a         comment on line *│$\n^[^\n]*│  \)/\(       numbered, any file *│$\n^[^\n]*│ \? close +Made by Wouter de Wild - 2026 │$\n^(?P<w2_120>[^\n]*╰─+╯)$",
    );
}

/// F-HELP-03: Find in file entries verbatim: [Find] type, backspace, Enter, esc; no hint, credit only
#[test]
fn f_help_03_find() {
    let mut s = Sim::builder().size(120, 40).build();
    s.keys("?");
    s.assert_row_matches(-1, r"^\(1-5/5\) hjkl move  enter ask  J/K comments  \? help$");
    s.assert_contains("Help · Diff view");
    s.keys("/");
    s.assert_row_matches(-1, "^/█");
    s.assert_not_contains("│  i ");
    s.assert_not_contains("? move keys");
    s.assert_not_contains("? close");
    assert_wide(
        &s,
        r"^(?P<w1_120>[^\n]*│ Help · Find in file *│)$\n^[^\n]*│ Find *│$\n^[^\n]*│  type      search text *│$\n^[^\n]*│  backspace delete char *│$\n^[^\n]*│  Enter     jump to match *│$\n^[^\n]*│  esc       cancel *│$\n^[^\n]*│ *Made by Wouter de Wild - 2026 │$\n^(?P<w2_120>[^\n]*╰─+╯)$",
    );
}

/// F-HELP-03: Focused comment entries verbatim: L1 [Comments] 6 items; L2 same [Comments] then [Move] d/u, g/G
#[test]
fn f_help_03_focused() {
    let mut s = Sim::builder().size(120, 40).build();
    s.keys("ahi<Enter>");
    s.assert_contains("question saved (1)");
    s.assert_not_contains("▸ sent");
    s.keys("J");
    s.assert_contains("▸ sent  line L2");
    s.assert_not_contains("Help ·");
    s.keys("?");
    s.assert_not_contains("│  i ");
    s.assert_not_contains("Move");
    assert_wide(
        &s,
        r"^(?P<w1_120>[^\n]*│ Help · Focused comment *│)$\n^[^\n]*│ Comments *│$\n^[^\n]*│  J/K     next/prev in file *│$\n^[^\n]*│  \)/\(     numbered, any file *│$\n^[^\n]*│  Enter   edit \(no replies\) *│$\n^[^\n]*│  A       ask: all *│$\n^[^\n]*│  up/down pick block to copy *│$\n^[^\n]*│  hjkl…   motion unfocuses *│$\n^[^\n]*│ \? move keys +Made by Wouter de Wild - 2026 │$\n^(?P<w2_120>[^\n]*╰─+╯)$",
    );
    s.keys("?");
    s.assert_not_contains("│  i ");
    s.assert_not_contains("? move keys");
    assert_wide(
        &s,
        r"^(?P<w1_120>[^\n]*│ Help · Focused comment *│)$\n^[^\n]*│ Comments *│$\n^[^\n]*│  J/K     next/prev in file *│$\n^[^\n]*│  \)/\(     numbered, any file *│$\n^[^\n]*│  Enter   edit \(no replies\) *│$\n^[^\n]*│  A       ask: all *│$\n^[^\n]*│  up/down pick block to copy *│$\n^[^\n]*│  hjkl…   motion unfocuses *│$\n^[^\n]*│ Move *│$\n^[^\n]*│  d/u     scroll thread *│$\n^[^\n]*│  g/G     thread top/bottom *│$\n^[^\n]*│ \? close +Made by Wouter de Wild - 2026 │$\n^(?P<w2_120>[^\n]*╰─+╯)$",
    );
}

/// F-HELP-03: Go to line entries verbatim: [Go to line] type, backspace, Enter, esc; no hint, credit only
#[test]
fn f_help_03_goto() {
    let mut s = Sim::builder().size(120, 40).build();
    s.keys("?");
    s.assert_row_matches(-1, r"^\(1-5/5\) hjkl move  enter ask  J/K comments  \? help$");
    s.assert_contains("Help · Diff view");
    s.keys(":");
    s.assert_row_matches(-1, "^:█");
    s.assert_not_contains("│  i ");
    s.assert_not_contains("? move keys");
    s.assert_not_contains("? close");
    assert_wide(
        &s,
        r"^(?P<w1_120>[^\n]*│ Help · Go to line *│)$\n^[^\n]*│ Go to line *│$\n^[^\n]*│  type      line number *│$\n^[^\n]*│  backspace delete char *│$\n^[^\n]*│  Enter     go to line *│$\n^[^\n]*│  esc       cancel *│$\n^[^\n]*│ *Made by Wouter de Wild - 2026 │$\n^(?P<w2_120>[^\n]*╰─+╯)$",
    );
}

/// F-HELP-03: MCP entries verbatim: L1 [MCP (M)] 7 items; L2 [Move] j/k first
#[test]
fn f_help_03_mcp() {
    let mut s = Sim::builder().size(120, 40).build();
    s.keys("M");
    s.assert_contains("INTEGRATIONS");
    s.assert_not_contains("Help ·");
    s.keys("?");
    s.assert_contains("INTEGRATIONS");
    s.assert_not_contains("│  i ");
    s.assert_not_contains("Move");
    assert_wide(
        &s,
        r"^(?P<w1_120>[^\n]*│ Help · MCP *│)$\n^[^\n]*│ MCP \(M\) *│$\n^[^\n]*│  ⏎/space server on / off *│$\n^[^\n]*│  Enter   register agent *│$\n^[^\n]*│  d       unregister agent *│$\n^[^\n]*│  y/n     confirm / cancel *│$\n^[^\n]*│  c/w     copy cmd / prompt *│$\n^[^\n]*│  R       refresh status *│$\n^[^\n]*│  esc/q/M close *│$\n^[^\n]*│ \? move keys +Made by Wouter de Wild - 2026 │$\n^(?P<w2_120>[^\n]*╰─+╯)$",
    );
    s.keys("?");
    s.assert_contains("INTEGRATIONS");
    s.assert_not_contains("│  i ");
    s.assert_not_contains("? move keys");
    assert_wide(
        &s,
        r"^(?P<w1_120>[^\n]*│ Help · MCP *│)$\n^[^\n]*│ Move *│$\n^[^\n]*│  j/k     move *│$\n^[^\n]*│ MCP \(M\) *│$\n^[^\n]*│  ⏎/space server on / off *│$\n^[^\n]*│  Enter   register agent *│$\n^[^\n]*│  d       unregister agent *│$\n^[^\n]*│  y/n     confirm / cancel *│$\n^[^\n]*│  c/w     copy cmd / prompt *│$\n^[^\n]*│  R       refresh status *│$\n^[^\n]*│  esc/q/M close *│$\n^[^\n]*│ \? close +Made by Wouter de Wild - 2026 │$\n^(?P<w2_120>[^\n]*╰─+╯)$",
    );
}

/// F-HELP-03: File picker entries verbatim: L1 [File picker] Enter, esc/f/q; L2 [Move] j/k, d/u first
#[test]
fn f_help_03_picker() {
    let mut s = Sim::builder().size(120, 40).build();
    s.keys("f");
    s.assert_contains("Files (1/4)");
    s.assert_not_contains("Help ·");
    s.keys("?");
    s.assert_contains("Files (1/4)");
    s.assert_not_contains("│  i ");
    s.assert_not_contains("Move");
    assert_wide(
        &s,
        r"^(?P<w1_120>[^\n]*│ Help · File picker *│)$\n^[^\n]*│ File picker *│$\n^[^\n]*│  Enter   open file *│$\n^[^\n]*│  esc/f/q close *│$\n^[^\n]*│ \? move keys +Made by Wouter de Wild - 2026 │$\n^(?P<w2_120>[^\n]*╰─+╯)$",
    );
    s.keys("?");
    s.assert_contains("Files (1/4)");
    s.assert_not_contains("│  i ");
    s.assert_not_contains("? move keys");
    assert_wide(
        &s,
        r"^(?P<w1_120>[^\n]*│ Help · File picker *│)$\n^[^\n]*│ Move *│$\n^[^\n]*│  j/k     move *│$\n^[^\n]*│  d/u     half page down/up *│$\n^[^\n]*│ File picker *│$\n^[^\n]*│  Enter   open file *│$\n^[^\n]*│  esc/f/q close *│$\n^[^\n]*│ \? close +Made by Wouter de Wild - 2026 │$\n^(?P<w2_120>[^\n]*╰─+╯)$",
    );
}

/// F-HELP-03: File search entries verbatim: [Search] type, backspace, down/up, Enter, esc; no hint, credit only
#[test]
fn f_help_03_search() {
    let mut s = Sim::builder().size(120, 40).build();
    s.keys("?");
    s.assert_contains("Help · Diff view");
    s.assert_not_contains("Search (");
    s.keys("F");
    s.assert_contains("Search (1/6)");
    s.assert_not_contains("│  i ");
    s.assert_not_contains("? move keys");
    s.assert_not_contains("? close");
    assert_wide(
        &s,
        r"^(?P<w1_120>[^\n]*│ Help · File search *│)$\n^[^\n]*│ Search *│$\n^[^\n]*│  type      filter files *│$\n^[^\n]*│  backspace delete char *│$\n^[^\n]*│  down/up   next / prev hit *│$\n^[^\n]*│  Enter     open hit *│$\n^[^\n]*│  esc       close *│$\n^[^\n]*│ *Made by Wouter de Wild - 2026 │$\n^(?P<w2_120>[^\n]*╰─+╯)$",
    );
}

/// F-HELP-03: Visual selection entries verbatim (no L2): [Selection] V, a; [Comments] J/K, )/(
#[test]
fn f_help_03_visual() {
    let mut s = Sim::builder().size(120, 40).build();
    s.keys("v");
    s.assert_row_contains(0, "[visual L2:C1] README.md");
    s.assert_not_contains("Help ·");
    s.keys("?");
    s.assert_not_contains("│  i ");
    s.assert_not_contains("Move");
    s.assert_not_contains("? move keys");
    assert_wide(
        &s,
        r"^(?P<w1_120>[^\n]*│ Help · Visual selection *│)$\n^[^\n]*│ Selection *│$\n^[^\n]*│  V   lines, end *│$\n^[^\n]*│  a   comment on it *│$\n^[^\n]*│ Comments *│$\n^[^\n]*│  J/K next/prev in file *│$\n^[^\n]*│  \)/\( numbered, any file *│$\n^[^\n]*│ \? close +Made by Wouter de Wild - 2026 │$\n^(?P<w2_120>[^\n]*╰─+╯)$",
    );
}

/// F-HELP-04: browse footer is the plain diff hint (also with split on, 120 cols); no esc exit
#[test]
fn f_help_04_browse() {
    let mut s = Sim::builder().size(120, 40).args(["--split"]).build();
    s.assert_row_contains(-1, "p pane");
    s.keys("FREADME<Enter>");
    s.assert_row_matches(0, r"^\[browse\] ");
    s.assert_row_matches(-1, r"^\(1-3/3\) hjkl move  enter ask  J/K comments  \? help$");
    s.assert_not_contains("esc exit");
    s.assert_not_contains("p pane");
}

/// F-HELP-04: diff footer (unified): hjkl move  enter ask  J/K comments  ? help; no esc exit, no p pane
#[test]
fn f_help_04_diff_unified() {
    let mut s = Sim::builder().size(80, 24).build();
    s.assert_row_matches(-1, r"^\(1-5/5\) hjkl move  enter ask  J/K comments  \? help$");
    s.assert_not_contains("esc exit");
    s.assert_not_contains("p pane");
    s.keys("jj");
    // cursor movement keeps the same footer (no scroll-mode footer)
    s.assert_row_contains(0, "[cursor L3:C1]");
    s.assert_row_matches(-1, r"^\(1-5/5\) hjkl move  enter ask  J/K comments  \? help$");
}

/// F-HELP-04: editor footer: new comment enter send  tab save/ask  esc cancel; edit enter send  esc cancel
#[test]
fn f_help_04_editor() {
    let mut s = Sim::builder().size(120, 40).build();
    s.keys("a");
    s.assert_row_matches(-1, r"^\(1-5/5\) enter send  tab save/ask  esc cancel$");
    s.keys("hi<Enter>J");
    s.assert_row_contains(-1, "e edit  D delete");
    s.keys("e");
    s.assert_row_matches(-1, r"\(1-5/5\) enter send  esc cancel$");
    s.assert_contains("edit line L2");
    assert!(!s.row(-1).contains("tab save/ask"), "row -1 contains {:?}\n{}", "tab save/ask", s.dump());
}

/// F-HELP-04: focused comment footer: e edit  D delete  a ask/follow up  j/k scroll  esc back  ? help
#[test]
fn f_help_04_focused() {
    let mut s = Sim::builder().size(120, 40).build();
    s.keys("ahi<Enter>");
    s.assert_row_matches(-1, r"hjkl move  enter ask  J/K comments  \? help$");
    s.keys("J");
    s.assert_row_matches(-1, r"\(1-5/5\) e edit  D delete  a ask/follow up  j/k scroll  esc back  \? help$");
    s.keys("<Esc>");
    s.assert_row_matches(-1, r"\(1-5/5\) hjkl move  enter ask  J/K comments  \? help$");
}

/// F-HELP-04: follow-up editor footer: enter send  esc cancel
#[test]
fn f_help_04_followup() {
    let mut s = Sim::builder().config_json(json!({"mcp": {"autostart": true}})).size(120, 30).build();
    s.assert_row_contains(0, "[mcp: on]");
    s.keys("afirst q<Enter>");
    let r = s.mcp_call("next_question", j(r#"{"wait_seconds": 1}"#));
    let t1 = r.tool_result()["thread_id"].as_str().unwrap_or_default().to_string();
    s.mcp_call("answer", j(&r#"{"thread_id": "${T1}", "text": "ans one"}"#.replace("${T1}", &t1)));
    s.keys("J");
    s.assert_row_contains(-1, "e edit  D delete  a ask/follow up");
    s.keys("a");
    s.assert_row_matches(-1, r"\(1-5/5\) enter send  esc cancel$");
    s.assert_contains("│ follow-up");
    assert!(!s.row(-1).contains("tab save/ask"), "row -1 contains {:?}\n{}", "tab save/ask", s.dump());
}

/// F-HELP-04: modals and find/goto keep the underlying hint (diff, visual, focused comment)
#[test]
fn f_help_04_keep_underlying() {
    let mut s = Sim::builder().size(120, 40).build();
    s.keys("/");
    s.assert_row_matches(-1, r"^/█\(1-5/5\) hjkl move  enter ask  J/K comments  \? help$");
    s.keys("<Esc>");
    s.keys(":");
    s.assert_row_matches(-1, r"^:█\(1-5/5\) hjkl move  enter ask  J/K comments  \? help$");
    s.keys("<Esc>");
    s.keys("f");
    s.assert_row_matches(-1, r"^\(1-5/5\) hjkl move  enter ask  J/K comments  \? help$");
    s.assert_contains("Files (1/4)");
    s.keys("<Esc>");
    s.keys("F");
    s.assert_row_matches(-1, r"^\(1-5/5\) hjkl move  enter ask  J/K comments  \? help$");
    s.assert_contains("Search (1/6)");
    s.keys("<Esc>");
    s.keys("M");
    s.assert_row_matches(-1, r"^\(1-5/5\) hjkl move  enter ask  J/K comments  \? help$");
    s.assert_contains("INTEGRATIONS");
    s.keys("<Esc>");
    s.keys("C");
    s.assert_row_matches(-1, r"^\(1-5/5\) hjkl move  enter ask  J/K comments  \? help$");
    s.assert_contains("mcp on startup");
    s.keys("<Esc>");
    s.keys("q");
    s.assert_row_matches(-1, r"^\(1-5/5\) hjkl move  enter ask  J/K comments  \? help$");
    s.assert_contains("Quit xplain? (y/n)");
    s.keys("n");
    s.keys("v/");
    s.assert_row_matches(-1, r"^/█\(1-5/5\) v/esc end  enter ask  hjkl move  \? help$");
    s.keys("<Esc>");
    s.keys(":");
    s.assert_row_matches(-1, r"^:█\(1-5/5\) v/esc end  enter ask  hjkl move  \? help$");
    s.keys("<Esc><Esc>");
    s.keys("ahi<Enter>JD");
    s.assert_row_matches(-1, r"e edit  D delete  a ask/follow up  j/k scroll  esc back  \? help$");
    s.assert_contains("Delete comment? (y/n)");
}

/// F-HELP-04: no-changes screen footer is the plain diff hint
#[test]
fn f_help_04_no_changes() {
    let s = Sim::builder().size(80, 24).fixture(Fixture::Empty).build();
    s.assert_row_contains(0, "No changes");
    s.assert_row_matches(-1, r"hjkl move  enter ask  J/K comments  \? help$");
    s.assert_not_contains("esc exit");
    s.assert_not_contains("p pane");
}

/// F-HELP-04: split set but not effective (80 cols): no p pane
#[test]
fn f_help_04_split_narrow() {
    let s = Sim::builder().size(80, 24).args(["--split"]).build();
    s.assert_row_matches(-1, r"hjkl move  enter ask  J/K comments  \? help$");
    s.assert_not_contains("p pane");
}

/// F-HELP-04: split effective adds p pane; toggling to unified drops it
#[test]
fn f_help_04_split() {
    let mut s = Sim::builder().size(120, 40).args(["--split"]).build();
    s.assert_row_contains(0, "[split]");
    s.assert_row_matches(-1, r"^\(\d+-\d+/\d+\) hjkl move  enter ask  J/K comments  p pane  \? help$");
    s.keys("s");
    s.assert_row_contains(0, "[unified]");
    s.assert_row_matches(-1, r"^\(1-5/5\) hjkl move  enter ask  J/K comments  \? help$");
    s.keys("s");
    s.assert_row_matches(-1, r"hjkl move  enter ask  J/K comments  p pane  \? help$");
}

/// F-HELP-04: visual footer: v/esc end  enter ask  hjkl move  ? help; back to diff hint when ended
#[test]
fn f_help_04_visual() {
    let mut s = Sim::builder().size(80, 24).build();
    s.keys("v");
    s.assert_row_matches(-1, r"^\(1-5/5\) v/esc end  enter ask  hjkl move  \? help$");
    s.keys("V");
    s.assert_row_matches(-1, r"v/esc end  enter ask  hjkl move  \? help$");
    s.keys("V");
    s.assert_row_matches(-1, r"^\(1-5/5\) hjkl move  enter ask  J/K comments  \? help$");
}
