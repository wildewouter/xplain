//! Ported from `e2e/scenarios/u10-visual`.

#![allow(clippy::panic, clippy::unwrap_used)]

use regex::Regex;
use serde_json::json;
use xplain_sim::{CellExpect as C, Fixture, Http, Sim};

const MCP_ON: &str = r#"{"mcp": {"autostart": true}}"#;

/// Empty fixture, size, files.
fn sim(cols: u16, rows: u16, files: &[(&str, &str)]) -> Sim {
    let mut b = Sim::builder().fixture(Fixture::Empty).size(cols, rows);
    for (p, t) in files {
        b = b.file(p, t);
    }
    b.build()
}

fn sim_mcp(cols: u16, rows: u16, files: &[(&str, &str)]) -> Sim {
    let mut b = Sim::builder().fixture(Fixture::Empty).size(cols, rows).config(MCP_ON);
    for (p, t) in files {
        b = b.file(p, t);
    }
    b.build()
}

fn sim_split(cols: u16, rows: u16) -> Sim {
    Sim::builder().fixture(Fixture::Empty).size(cols, rows).args(["--split"]).build()
}

/// Row matches regex.
#[track_caller]
fn m(s: &Sim, row: isize, re: &str) {
    s.assert_row_matches(row, re);
}
/// Row equals text.
#[track_caller]
fn e(s: &Sim, row: isize, text: &str) {
    s.assert_row(row, text);
}
/// Row contains text.
#[track_caller]
fn k(s: &Sim, row: isize, text: &str) {
    s.assert_row_contains(row, text);
}

fn expect(ch: char, spec: &str) -> C {
    let mut x = C::new();
    if ch != '\0' {
        x = x.ch(ch);
    }
    for t in spec.split_whitespace() {
        x = match t {
            "inv" => x.reverse(true),
            "!inv" => x.reverse(false),
            "bold" => x.bold(true),
            "!bold" => x.bold(false),
            _ => match (t.strip_prefix("bg:"), t.strip_prefix("fg:")) {
                (Some(v), _) => x.bg(v),
                (_, Some(v)) => x.fg(v),
                _ => panic!("bad cell spec {t:?}"),
            },
        };
    }
    x
}

/// Cell at (row, col): char (`'\0'` = unchecked) and a spec of `bg:X fg:X inv !inv bold !bold`.
#[track_caller]
fn cl(s: &Sim, row: isize, col: usize, ch: char, spec: &str) {
    s.assert_cell(col, row, expect(ch, spec));
}

/// Cell at the first `text` (within `row` when given) plus `off` columns; `ch` unchecked unless not `'\0'`.
#[track_caller]
fn ct(s: &Sim, text: &str, row: Option<isize>, off: usize, ch: char, spec: &str) {
    let p = match row {
        Some(r) => s.find_in_row(r, text),
        None => s.find(text),
    };
    let p = p.unwrap_or_else(|| panic!("text {text:?} not found\n{}", s.dump()));
    s.assert_cell(p.x + off, p.y as isize, expect(ch, spec));
}

#[track_caller]
fn re_ok(text: &str, pattern: &str) {
    assert!(Regex::new(pattern).unwrap().is_match(text), "{text:?} !~ /{pattern}/");
}

#[track_caller]
fn annotate_ok(s: &mut Sim, args: serde_json::Value) {
    let r = s.mcp_call("annotate", args);
    assert_eq!(r.status, 200);
    assert_eq!(r.tool_result()["ok"], true);
}

/// Commit `base` as f.txt.
fn commit_base(s: &Sim, base: &str) {
    s.write_file("f.txt", base);
    s.git(&["add", "-A"]);
    s.git(&["commit", "-qm", "base"]);
}

const BOX80: &str = "╭─────────────────────────────────────────────────────────────────────────────╮";
const BOX80B: &str = "╰─────────────────────────────────────────────────────────────────────────────╯";

// ---------------------------------------------------------------------------------------------------------------
// F-COMMENT-01
// ---------------------------------------------------------------------------------------------------------------

/// F-COMMENT-01: edit editor head `edit <comment head>`, quoted lines, prefilled text, edit hint.
#[test]
fn f_comment_01_edit_head() {
    let mut s = sim(80, 30, &[("f.txt", "alpha beta\ntwo words here\nthree\n")]);
    s.keys("Vja");
    s.keys("msg<Enter>");
    s.assert_contains("sent  selection L1-2");
    s.assert_not_contains("edit selection");
    s.keys("J");
    s.assert_contains("▸ sent  selection L1-2");
    s.keys("e");
    s.assert_contains("│ edit selection L1-2 ");
    s.assert_matches(r"│ > alpha beta +│\n│ > two words here +│\n│ msg +│\n│ enter send  esc cancel +│\n╰");
    s.assert_matches(r"│ edit selection L1-2 +│\n│ > alpha beta");
    s.assert_not_contains("[save]");
    m(&s, -1, r"enter send  esc cancel$");
    ct(&s, "edit selection L1-2", None, 0, '\0', "fg:#cb4b16");
    // caret at end of prefilled text
    ct(&s, "│ msg ", None, 5, ' ', "inv");
}

/// F-COMMENT-01: Enter opens the editor too; placed below the cursor row's existing comment boxes.
#[test]
fn f_comment_01_enter_below_boxes() {
    let mut s = sim(80, 24, &[("f.txt", "alpha beta\ntwo words here\n")]);
    s.keys("<Enter>");
    m(&s, 4, r"^╭─+╮$");
    m(&s, 6, r"^│ \[save\] enter send");
    s.keys("first<Enter>");
    m(&s, 4, r"^╭─+╮$");
    m(&s, 5, r"^│ sent  line L1");
    m(&s, 6, r"^│ first +│$");
    m(&s, 7, r"^╰─+╯$");
    e(&s, 8, "        2 + two words here");
    s.keys("<Enter>");
    m(&s, 5, r"^│ sent  line L1");
    m(&s, 7, r"^╰─+╯$");
    m(&s, 8, r"^╭─+╮$");
    m(&s, 9, r"^│  +│$");
    m(&s, 10, r"^│ \[save\] enter send  tab save/ask  esc cancel +│$");
    m(&s, 11, r"^╰─+╯$");
    e(&s, 12, "        2 + two words here");
}

/// F-COMMENT-01: follow-up editor (a on focused agent note) has head `follow-up` and edit/follow-up hint.
#[test]
fn f_comment_01_follow_up_head() {
    let mut s = sim_mcp(80, 30, &[("f.txt", "alpha beta\ntwo words here\n")]);
    k(&s, 0, "[mcp: on]");
    annotate_ok(&mut s, json!({"file": "f.txt", "line": 1, "text": "agent says hi"}));
    s.assert_contains("agent note L1");
    s.assert_not_contains("follow-up");
    s.keys("Ja");
    s.assert_matches(r"│ follow-up +│\n│  +│\n│ enter send  esc cancel +│\n╰");
    s.assert_not_contains("[ask]");
    s.assert_not_contains("tab save/ask  esc");
    m(&s, -1, r"enter send  esc cancel$");
    ct(&s, "│ follow-up", None, 2, 'f', "fg:#cb4b16");
}

/// F-COMMENT-01: new-comment hint at cols 25 falls back to `enter send`.
#[test]
fn f_comment_01_hint_25() {
    let mut s = sim(25, 16, &[("f.txt", "alpha\n")]);
    s.keys("a");
    m(&s, 4, r"^╭─+╮$");
    m(&s, 6, r"^│ enter send +│$");
    s.assert_not_contains("esc cancel │");
    cl(&s, 4, 23, '╮', "");
}

/// F-COMMENT-01: hint at cols 26 (width 25) - `enter send  esc cancel` fits exactly.
#[test]
fn f_comment_01_hint_26() {
    let mut s = sim(26, 16, &[("f.txt", "alpha\n")]);
    s.keys("a");
    m(&s, 4, r"^╭─+╮$");
    m(&s, 6, r"^│ enter send  esc cancel│$");
}

/// F-COMMENT-01: hint at cols 39 falls back to `enter send  esc cancel`.
#[test]
fn f_comment_01_hint_39() {
    let mut s = sim(39, 16, &[("f.txt", "alpha\n")]);
    s.keys("a");
    m(&s, 4, r"^╭─+╮$");
    m(&s, 6, r"^│ enter send  esc cancel +│$");
}

/// F-COMMENT-01: hint at cols 40 (width 39) - `enter send  tab save/ask  esc cancel` fits exactly.
#[test]
fn f_comment_01_hint_40() {
    let mut s = sim(40, 16, &[("f.txt", "alpha\n")]);
    s.keys("a");
    m(&s, 4, r"^╭─+╮$");
    m(&s, 6, r"^│ enter send  tab save/ask  esc cancel│$");
}

/// F-COMMENT-01: hint at cols 46 - full hint no longer fits, drops the [save] chip.
#[test]
fn f_comment_01_hint_46() {
    let mut s = sim(46, 16, &[("f.txt", "alpha\n")]);
    s.keys("a");
    m(&s, 4, r"^╭─+╮$");
    m(&s, 6, r"^│ enter send  tab save/ask  esc cancel +│$");
    s.assert_not_contains("[save]");
}

/// F-COMMENT-01: hint at cols 47 (width 46) - full hint fits exactly (width-3).
#[test]
fn f_comment_01_hint_47() {
    let mut s = sim(47, 16, &[("f.txt", "alpha\n")]);
    s.keys("a");
    m(&s, 4, r"^╭─+╮$");
    m(&s, 6, r"^│ \[save\] enter send  tab save/ask  esc cancel│$");
    cl(&s, 4, 45, '╮', "");
}

/// F-COMMENT-01: long input scrolls a window of width-4 chars, start = max(0, caret-(width-4)+1). Width 39, window 35.
#[test]
fn f_comment_01_long_input() {
    let mut s = sim(40, 16, &[("f.txt", "alpha\n")]);
    s.keys("a");
    s.keys("abcdefghijklmnopqrstuvwxyz0123456789ABCD");
    // caret 40 at end - start 6, shows `g`..`D` then caret cell
    e(&s, 5, "│ ghijklmnopqrstuvwxyz0123456789ABCD  │");
    cl(&s, 5, 36, ' ', "inv");
    cl(&s, 5, 35, 'D', "!inv");
    s.keys("<Left><Left><Left><Left><Left><Left><Left><Left><Left><Left>");
    // caret 30 - start 0, window `a`..`8`, caret on `4`
    e(&s, 5, "│ abcdefghijklmnopqrstuvwxyz012345678 │");
    cl(&s, 5, 32, '4', "inv");
    cl(&s, 5, 31, '3', "!inv");
}

/// F-COMMENT-01: editor box width is max(10, cols-1) - at cols 10 the box spans all 10 columns.
#[test]
fn f_comment_01_min_width() {
    let mut s = sim(10, 16, &[("f.txt", "alpha\n")]);
    s.keys("a");
    e(&s, 4, "╭────────╮");
    e(&s, 7, "╰────────╯");
}

/// F-COMMENT-01: `a` opens the editor box under the cursor row - round modalBorder border, modalBg, width cols-1,
/// no head, input + hint rows.
#[test]
fn f_comment_01_open_box() {
    let mut s = sim(80, 20, &[("f.txt", "alpha beta\ntwo words here\n\nfourth line\nfifth\n")]);
    m(&s, 3, r"^        1 \+▶alpha beta");
    e(&s, 4, "        2 + two words here");
    s.assert_not_contains("╭");
    s.keys("a");
    // box height 4 (top border, input, hint, bottom border), rows below pushed down
    m(&s, 3, r"^        1 \+▶alpha beta");
    e(&s, 4, BOX80);
    m(&s, 5, r"^│  +│$");
    m(&s, 6, r"^│ \[save\] enter send  tab save/ask  esc cancel +│$");
    e(&s, 7, BOX80B);
    e(&s, 8, "        2 + two words here");
    m(&s, -1, r"enter send  tab save/ask  esc cancel$");
    cl(&s, 4, 0, '╭', "fg:#268bd2");
    cl(&s, 4, 78, '╮', "fg:#268bd2");
    cl(&s, 7, 0, '╰', "fg:#268bd2");
    cl(&s, 5, 10, '\0', "bg:#002b36");
    // empty input - caret is reverse space at text start
    cl(&s, 5, 2, ' ', "inv");
    s.keys("hi");
    m(&s, 5, r"^│ hi +│$");
    cl(&s, 5, 2, 'h', "fg:#93a1a1 bg:#002b36 !inv");
    cl(&s, 5, 4, ' ', "inv");
}

/// F-COMMENT-01: selection editor - accent head, up to 5 dim quoted lines (tabs expanded), then `… +k more`.
#[test]
fn f_comment_01_quotes() {
    let mut s = sim(
        80,
        30,
        &[("f.txt", "alpha beta\n\tgamma delta\n\nepsilon zeta\nlast line here\nsix\nseven\neight\n")],
    );
    s.keys("VGa");
    m(&s, 10, r"^        8 \+▶eight");
    m(&s, 11, r"^╭─+╮$");
    m(&s, 12, r"^│ selection L1-8 +│$");
    m(&s, 13, r"^│ > alpha beta +│$");
    m(&s, 14, r"^│ >   gamma delta +│$");
    m(&s, 15, r"^│ > +│$");
    m(&s, 16, r"^│ > epsilon zeta +│$");
    m(&s, 17, r"^│ > last line here +│$");
    m(&s, 18, r"^│ … \+3 more +│$");
    m(&s, 19, r"^│  +│$");
    m(&s, 20, r"^│ \[save\] enter send");
    m(&s, 21, r"^╰─+╯$");
    s.assert_not_contains("> six");
    cl(&s, 12, 2, 's', "fg:#cb4b16 bg:#002b36");
    cl(&s, 13, 4, 'a', "fg:#586e75");
    s.keys("<Esc><Esc>");
    s.assert_not_contains("╭");
    // exactly 5 lines - no more-row
    s.keys("ggjVjjjja");
    m(&s, 7, r"^        5 \+▶last line here");
    m(&s, 8, r"^╭─+╮$");
    m(&s, 9, r"^│ selection L1-5 +│$");
    m(&s, 14, r"^│ > last line here +│$");
    m(&s, 15, r"^│  +│$");
    m(&s, 16, r"^│ \[save\] enter send");
    m(&s, 17, r"^╰─+╯$");
    s.assert_not_contains("more");
}

// ---------------------------------------------------------------------------------------------------------------
// F-COMMENT-02
// ---------------------------------------------------------------------------------------------------------------

/// F-COMMENT-02: editor keys - insert at caret, Left/Right, Backspace + Delete delete before caret, ctrl/meta/Up/
/// Down/Home/End ignored, ? typed.
#[test]
fn f_comment_02_editing() {
    let mut s = sim(80, 20, &[("f.txt", "alpha beta\ntwo words here\n")]);
    s.keys("a");
    s.keys("abc");
    m(&s, 5, r"^│ abc +│$");
    cl(&s, 5, 5, ' ', "inv");
    s.keys("<Left><Left>");
    cl(&s, 5, 3, 'b', "inv");
    cl(&s, 5, 5, ' ', "!inv");
    s.keys("X");
    m(&s, 5, r"^│ aXbc +│$");
    cl(&s, 5, 4, 'b', "inv");
    s.keys("<Right>");
    cl(&s, 5, 5, 'c', "inv");
    s.keys("<BS>");
    m(&s, 5, r"^│ aXc +│$");
    cl(&s, 5, 4, 'c', "inv");
    s.keys("<Del>");
    m(&s, 5, r"^│ ac +│$");
    cl(&s, 5, 3, 'c', "inv");
    // ignored keys - text, caret and cursor row unchanged, no help panel
    s.keys("<C-a><C-e><C-u><A-b><M-x><Up><Down><Home><End>");
    m(&s, 0, r"\[cursor L1:C1\] f\.txt ");
    m(&s, 3, r"^        1 \+▶alpha beta");
    m(&s, 5, r"^│ ac +│$");
    cl(&s, 5, 3, 'c', "inv");
    s.keys("?");
    m(&s, 5, r"^│ a\?c +│$");
    s.keys("<Right>");
    cl(&s, 5, 5, ' ', "inv");
    s.keys("<Right>");
    // Right at end stays
    cl(&s, 5, 5, ' ', "inv");
    // each CR/LF run in a paste (one chunk) becomes one space, inserted at the caret; CR does not submit
    s.paste("\r\nx\r\n\ny");
    m(&s, 5, r"^│ a\?c x y +│$");
    s.keys("<Enter>");
    m(&s, 5, r"^│ sent  line L1");
    m(&s, 6, r"^│ a\?c x y +│$");
    k(&s, -1, "question saved (1)");
}

/// F-COMMENT-02: Enter with empty or whitespace-only text does nothing (editor stays open).
#[test]
fn f_comment_02_enter_empty() {
    let mut s = sim(80, 20, &[("f.txt", "alpha beta\ntwo words here\n")]);
    s.keys("a<Enter>");
    m(&s, 4, r"^╭─+╮$");
    m(&s, 5, r"^│  +│$");
    m(&s, 6, r"^│ \[save\] enter send");
    s.assert_not_contains("question saved");
    s.keys("   <Enter>");
    m(&s, 4, r"^╭─+╮$");
    m(&s, 6, r"^│ \[save\] enter send");
    s.assert_not_contains("question saved");
    s.assert_not_contains("sent  line");
    cl(&s, 5, 5, ' ', "inv");
    s.keys("  ok  <Enter>");
    k(&s, -1, "question saved (1)");
    s.assert_contains("sent  line L1");
}

/// F-COMMENT-02: Esc closes the editor and discards the text (no comment created).
#[test]
fn f_comment_02_esc_discard() {
    let mut s = sim(80, 20, &[("f.txt", "alpha beta\ntwo words here\n")]);
    s.keys("a");
    s.keys("draft");
    m(&s, 5, r"^│ draft +│$");
    s.keys("<Esc>");
    m(&s, 0, r"\[cursor L1:C1\] f\.txt ");
    e(&s, 4, "        2 + two words here");
    m(&s, -1, r"\(1-3/3\) hjkl move  enter ask  J/K comments  \? help$");
    s.assert_not_contains("╭");
    s.assert_not_contains("draft");
    s.assert_not_contains("question saved");
    s.keys("J");
    k(&s, -1, "no comments");
    // reopening starts empty
    s.keys("a");
    m(&s, 5, r"^│  +│$");
    s.assert_not_contains("draft");
}

/// F-COMMENT-02: opening the editor keeps the cursor row visible with the box under it (follow margin 0).
/// H = 7, margin 2; 21 rows (hunk + 20 lines).
#[test]
fn f_comment_02_follow() {
    let mut text = String::new();
    for i in 1..=20 {
        text.push_str(&format!("l{i}\n"));
    }
    let mut s = sim(80, 10, &[("f.txt", &text)]);
    s.keys("5j");
    m(&s, 0, r"\[cursor L6:C1\] f\.txt ");
    e(&s, 2, "        2 + l2");
    m(&s, 6, r"^        6 \+▶l6");
    m(&s, -1, r"^\(3-9/21\) ");
    s.keys("a");
    // box of 4 rows must fit under cursor row - top moves to L4, cursor row, then box to viewport bottom
    e(&s, 2, "        4 + l4");
    m(&s, 4, r"^        6 \+▶l6");
    m(&s, 5, r"^╭─+╮$");
    m(&s, 8, r"^╰─+╯$");
    m(&s, -1, r"^\(5-11/21\) ");
}

/// F-COMMENT-02: Tab with MCP off - note `MCP is off (M to start)`, mode stays save.
#[test]
fn f_comment_02_tab_mcp_off() {
    let mut s = sim(80, 20, &[("f.txt", "alpha beta\n")]);
    s.keys("a");
    m(&s, 6, r"^│ \[save\] enter send");
    s.assert_not_contains("MCP is off");
    s.keys("<Tab>");
    m(&s, 6, r"^│ \[save\] enter send");
    k(&s, -1, "MCP is off (M to start)");
    s.assert_not_contains("[ask]");
    s.keys("hi<Enter>");
    k(&s, -1, "question saved (1)");
}

/// F-COMMENT-02: MCP running - editor defaults to ask, Tab toggles ask/save, choice kept, reset by MCP stop, ask
/// again after restart.
#[test]
fn f_comment_02_tab_mcp_on() {
    let mut s = sim_mcp(80, 30, &[("f.txt", "alpha beta\n")]);
    k(&s, 0, "[mcp: on]");
    s.keys("a");
    m(&s, 6, r"^│ \[ask\] enter send  tab save/ask  esc cancel +│$");
    s.keys("<Tab>");
    m(&s, 6, r"^│ \[save\] enter send  tab save/ask  esc cancel +│$");
    s.keys("<Tab>");
    m(&s, 6, r"^│ \[ask\] enter send");
    s.keys("<Tab><Esc>");
    // chosen mode kept for next editor
    s.keys("a");
    m(&s, 6, r"^│ \[save\] enter send");
    s.keys("<Esc>M");
    s.assert_contains("> ● on ");
    s.keys("<Space>");
    s.assert_contains("○ off");
    k(&s, 0, "[mcp: off]");
    s.keys("<Space>");
    s.assert_contains("● on ");
    k(&s, 0, "[mcp: on]");
    s.keys("<Esc>a");
    // MCP stop reset the chosen mode; running again - default ask
    m(&s, 6, r"^│ \[ask\] enter send");
}

// ---------------------------------------------------------------------------------------------------------------
// F-COMMENT-03
// ---------------------------------------------------------------------------------------------------------------

/// F-COMMENT-03: ask-mode submit - note `question sent to agent`; question carries file, side, line, raw line
/// text, context rows +-3.
#[test]
fn f_comment_03_ask_line() {
    let mut s = sim_mcp(80, 30, &[("f.txt", "one\ntwo\nthree\nfour\n\tfive\tx\nsix\nseven\neight\nnine\n")]);
    k(&s, 0, "[mcp: on]");
    let poll = s.http_start(Http::tool("next_question", json!({"wait_seconds": 60})));
    s.keys("4j");
    m(&s, 0, r"\[cursor L5:C1\] f\.txt ");
    s.keys("awhy tab<Enter>");
    k(&s, -1, "question sent to agent");
    s.assert_contains("sent  line L5");
    let r = s.http_await(&poll);
    let q = r.tool_result();
    assert_eq!(q["status"], "question");
    assert_eq!(q["turn"], 1);
    assert_eq!(q["follow_up"], false);
    re_ok(
        q["question"].as_str().unwrap_or_default(),
        r"^why tab\n\nFile: f\.txt\nSide: new \(after the change\)\nLines: 5\n\nCode at cursor line:\n```\n\tfive\tx\n```\n\nSurrounding context:\n```\ntwo\nthree\nfour\n\tfive\tx\nsix\nseven\neight\n```",
    );
}

/// F-COMMENT-03: ask-mode selection submit - Selected code (char cut at cols, tabs expanded), Lines a-b, context
/// from first-3 to last+3 (hunk row excluded).
#[test]
fn f_comment_03_ask_selection() {
    let mut s = sim_mcp(80, 30, &[("f.txt", "one\ntwo\n\tthree x\nfour five\nsix\nseven\neight\nnine\n")]);
    k(&s, 0, "[mcp: on]");
    let poll = s.http_start(Http::tool("next_question", json!({"wait_seconds": 60})));
    // anchor L3:C3 (tab expanded to 2 spaces -> `three x`), cursor L4:C4
    s.keys("jjllvjl");
    m(&s, 0, r"\[visual L4:C4\] f\.txt ");
    s.keys("asel<Enter>");
    k(&s, -1, "question sent to agent");
    let r = s.http_await(&poll);
    let q = r.tool_result();
    assert_eq!(q["status"], "question");
    re_ok(
        q["question"].as_str().unwrap_or_default(),
        r"^sel\n\nFile: f\.txt\nSide: new \(after the change\)\nLines: 3-4\n\nSelected code:\n```\nthree x\nfour\n```\n\nSurrounding context:\n```\none\ntwo\n\tthree x\nfour five\nsix\nseven\neight\n```",
    );
}

/// F-COMMENT-03: saved count k includes agent notes.
#[test]
fn f_comment_03_count_notes() {
    let mut s = sim_mcp(80, 30, &[("f.txt", "alpha beta\ntwo words here\n")]);
    k(&s, 0, "[mcp: on]");
    annotate_ok(&mut s, json!({"file": "f.txt", "line": 2, "text": "agent note text"}));
    s.assert_contains("agent note L2");
    s.keys("a<Tab>");
    s.assert_contains("[save] enter send");
    s.keys("mine<Enter>");
    k(&s, -1, "question saved (2)");
}

/// F-COMMENT-03: comment on hunk row (no line number) gets head `line r1` and sits under the hunk row.
#[test]
fn f_comment_03_hunk_row() {
    let mut s = sim(80, 20, &[("f.txt", "alpha beta\n")]);
    s.keys("g");
    m(&s, 0, r"\[cursor r1:C1\] f\.txt ");
    s.keys("ax<Enter>");
    m(&s, 2, r"^@@ -0,0 \+1 @@");
    m(&s, 3, r"^╭─+╮$");
    m(&s, 4, r"^│ sent  line r1  saved · not asked +│$");
    e(&s, 7, "        1 + alpha beta");
    k(&s, -1, "question saved (1)");
}

/// F-COMMENT-03: save-mode submit - editor closes, box under the row with head `line L<no>`, note
/// `question saved (<k>)` counting all comments.
#[test]
fn f_comment_03_save() {
    let mut s = sim(80, 30, &[("f.txt", "alpha beta\ntwo words here\nthree\n")]);
    s.keys("ahello<Enter>");
    m(&s, 3, r"^        1 \+▶alpha beta");
    m(&s, 4, r"^╭─+╮$");
    m(&s, 5, r"^│ sent  line L1  saved · not asked +│$");
    m(&s, 6, r"^│ hello +│$");
    m(&s, 7, r"^╰─+╯$");
    e(&s, 8, "        2 + two words here");
    m(&s, -1, r"^question saved \(1\) \| \(1-4/4\) hjkl move  enter ask  J/K comments  \? help$");
    s.assert_not_contains("enter send");
    s.assert_not_contains("[save]");
    s.keys("jaworld<Enter>");
    m(&s, 8, r"^        2 \+▶two words here");
    m(&s, 9, r"^╭─+╮$");
    m(&s, 10, r"^│ sent  line L2  saved · not asked +│$");
    m(&s, 11, r"^│ world +│$");
    k(&s, -1, "question saved (2)");
}

/// F-COMMENT-03: selection submit - box anchored under last selected row (also when selecting upward), head
/// `selection <tag>`, selection cleared.
#[test]
fn f_comment_03_selection() {
    let mut s = sim(80, 30, &[("f.txt", "alpha beta\ntwo words here\nthree\nfour\n")]);
    s.keys("Vjj");
    cl(&s, 3, 12, 'a', "bg:#6b4f00");
    s.keys("amsg<Enter>");
    m(&s, 0, r"\[cursor L3:C1\] f\.txt ");
    m(&s, 5, r"^        3 \+▶three");
    m(&s, 6, r"^╭─+╮$");
    m(&s, 7, r"^│ sent  selection L1-3  saved · not asked +│$");
    m(&s, 8, r"^│ > alpha beta +│$");
    m(&s, 11, r"^│ msg +│$");
    m(&s, 12, r"^╰─+╯$");
    e(&s, 13, "        4 + four");
    k(&s, -1, "question saved (1)");
    cl(&s, 3, 12, 'a', "bg:default");
    cl(&s, 4, 12, 't', "bg:default");
    // upward selection L4 -> L2 - box still under L4 (last selected row), cursor stays L2
    s.keys("jvkkl");
    m(&s, 0, r"\[visual L2:C2\] f\.txt ");
    s.keys("aup<Enter>");
    m(&s, 0, r"\[cursor L2:C2\] f\.txt ");
    m(&s, 4, r"^        2 \+▶two words here");
    e(&s, 5, "        3 + three");
    e(&s, 13, "        4 + four");
    m(&s, 14, r"^╭─+╮$");
    m(&s, 15, r"^│ sent  selection L2:C2-L4:C1  saved · not asked +│$");
    k(&s, -1, "question saved (2)");
}

// ---------------------------------------------------------------------------------------------------------------
// F-COMMENT-04
// ---------------------------------------------------------------------------------------------------------------

const B20: &str = "b1\nb2\nb3\nb4\nb5\nb6\nb7\nb8\nb9\nb10\nb11\nb12\nb13\nb14\nb15\nb16\nb17\nb18\nb19\nb20";

/// F-COMMENT-04: unfocused box body capped at 14 lines + ` … +k more`; agent note head has no `saved · not asked`.
#[test]
fn f_comment_04_body_limit() {
    let mut s = sim_mcp(80, 40, &[("f.txt", "alpha\nbeta\n")]);
    k(&s, 0, "[mcp: on]");
    annotate_ok(&mut s, json!({"file": "f.txt", "line": 1, "text": B20}));
    m(&s, 3, r"^        1 \+▶alpha");
    m(&s, 4, r"^╭─+╮$");
    m(&s, 5, r"^│ sent  agent note L1 +│$");
    m(&s, 6, r"^│ b1 +│$");
    m(&s, 19, r"^│ b14 +│$");
    m(&s, 20, r"^│ … \+6 more +│$");
    m(&s, 21, r"^╰─+╯$");
    e(&s, 22, "        2 + beta");
    s.assert_not_contains("b15");
    s.assert_not_contains("saved · not asked");
    // focused with room (window 36 >= 20) - all 20 lines, no scroll suffix, no j/k scroll hint
    s.keys("J");
    m(&s, 5, r"^┃ ▸ sent  agent note L1 +┃$");
    m(&s, 25, r"^┃ b20 +┃$");
    m(&s, 26, r"^┃ e edit  D delete  a follow up  esc back +┃$");
    m(&s, 27, r"^┗━+┛$");
    s.assert_not_contains("more");
    s.assert_not_contains("↕");
    s.assert_not_contains("j/k scroll  esc back ┃");
}

/// F-COMMENT-04: focused box with code blocks and overflow - hint gains `j/k scroll` and `↑/↓ code`.
/// H = 11, window = 11-1-4 = 6.
#[test]
fn f_comment_04_code_hint() {
    let mut s = sim_mcp(80, 14, &[("f.txt", "alpha\n")]);
    k(&s, 0, "[mcp: on]");
    annotate_ok(
        &mut s,
        json!({"file": "f.txt", "line": 1, "text": "see:\n```ts\nconst a = 1\nconst b = 2\nconst c = 3\n```\nt1\nt2\nt3\nt4"}),
    );
    s.keys("J");
    s.assert_contains("▸ sent  agent note L1  ↕ 1-6/");
    s.assert_matches(r". e edit  D delete  a follow up  j/k scroll  ↑/↓ code  esc back +.\n");
}

/// F-COMMENT-04: comment box - dim round border, accent head ` sent  <head>` + dim `saved · not asked`; focused -
/// accent border, ` ▸ ` bold head, hint row. (The oracle draws the focused border with heavy box chars; either
/// line style is accepted, accent fg only.)
#[test]
fn f_comment_04_focus_render() {
    let mut s = sim(80, 20, &[("f.txt", "alpha beta\ntwo words here\n")]);
    s.keys("ahello<Enter>");
    e(&s, 4, BOX80);
    m(&s, 5, r"^│ sent  line L1  saved · not asked +│$");
    m(&s, 6, r"^│ hello +│$");
    e(&s, 7, BOX80B);
    e(&s, 8, "        2 + two words here");
    cl(&s, 4, 0, '╭', "fg:#586e75 !bold");
    cl(&s, 4, 78, '╮', "fg:#586e75");
    cl(&s, 5, 0, '│', "fg:#586e75");
    cl(&s, 5, 2, 's', "fg:#cb4b16 !bold");
    ct(&s, "line L1", Some(5), 0, '\0', "fg:#cb4b16");
    ct(&s, "saved", Some(5), 0, '\0', "fg:#586e75");
    s.keys("J");
    // focused - accent border (heavier than the dim round one), head with ▸ bold, hint row adds one row
    m(&s, 4, r"^.[─━]+.$");
    m(&s, 5, r"^. ▸ sent  line L1  saved · not asked +.$");
    m(&s, 6, r"^. hello +.$");
    m(&s, 7, r"^. e edit  D delete  a ask  esc back +.$");
    m(&s, 8, r"^.[─━]+.$");
    e(&s, 9, "        2 + two words here");
    k(&s, -1, "e edit  D delete  a ask/follow up  j/k scroll");
    s.assert_not_contains("╭");
    cl(&s, 4, 0, '\0', "fg:#cb4b16");
    cl(&s, 8, 78, '\0', "fg:#cb4b16");
    cl(&s, 6, 0, '\0', "fg:#cb4b16");
    ct(&s, "▸ sent", Some(5), 0, '\0', "fg:#cb4b16 bold");
    ct(&s, "line L1", Some(5), 0, '\0', "fg:#cb4b16 bold");
    s.keys("<Esc>");
    m(&s, 5, r"^│ sent  line L1  saved · not asked +│$");
    m(&s, 7, r"^╰─+╯$");
    e(&s, 8, "        2 + two words here");
    s.assert_not_contains("e edit");
    cl(&s, 4, 0, '╭', "fg:#586e75 !bold");
}

/// F-COMMENT-04: focused body window = H-1-4-(other boxes on row); suffix `↕ from-to/total`, hint gains `j/k scroll`.
/// H = 13; window = 13-1-4 = 8 alone.
#[test]
fn f_comment_04_focus_window() {
    let mut s = sim_mcp(80, 16, &[("f.txt", "alpha\nbeta\n")]);
    k(&s, 0, "[mcp: on]");
    annotate_ok(&mut s, json!({"file": "f.txt", "line": 1, "text": B20}));
    s.assert_contains("sent  agent note L1");
    s.keys("J");
    s.assert_matches(
        r"        1 \+▶alpha.*\n.[─━]+.\n. ▸ sent  agent note L1  ↕ 1-8/20 +.\n. b1 +.\n. b2 +.\n. b3 +.\n. b4 +.\n. b5 +.\n. b6 +.\n. b7 +.\n. b8 +.\n. e edit  D delete  a follow up  j/k scroll  esc back +.\n.[─━]+.\n",
    );
    s.assert_not_contains("b9");
    s.assert_not_contains("more");
    ct(&s, "↕ 1-8/20", None, 0, '\0', "fg:#586e75");
    // second box on the same row shrinks the window by its height (4)
    s.keys("<Esc>a<Tab>other<Enter>");
    k(&s, -1, "question saved (2)");
    // K focuses last (human box, below the note, off screen), K again the agent note
    s.keys("KK");
    s.assert_contains("▸ sent  agent note L1  ↕ 1-4/20");
    s.assert_matches(r". b4 +.\n. e edit  D delete  a follow up  j/k scroll  esc back +.\n");
    s.assert_not_contains("b5");
}

/// F-COMMENT-04: focused hint at cols 19 - only `e edit`.
#[test]
fn f_comment_04_hint_19() {
    let mut s = sim(19, 16, &[("f.txt", "alpha\n")]);
    s.keys("ahi<Enter>J");
    m(&s, 5, r"^. ▸ sent");
    m(&s, 7, r"^. e edit +.$");
}

/// F-COMMENT-04: focused hint at cols 20 - `e edit  D delete` fits exactly.
#[test]
fn f_comment_04_hint_20() {
    let mut s = sim(20, 16, &[("f.txt", "alpha\n")]);
    s.keys("ahi<Enter>J");
    m(&s, 5, r"^. ▸ sent");
    m(&s, 7, r"^. e edit  D delete.$");
}

/// F-COMMENT-04: focused hint at cols 36 - drops `esc back`.
#[test]
fn f_comment_04_hint_36() {
    let mut s = sim(36, 16, &[("f.txt", "alpha\n")]);
    s.keys("ahi<Enter>J");
    m(&s, 5, r"^. ▸ sent");
    m(&s, 7, r"^. e edit  D delete  a ask +.$");
}

/// F-COMMENT-04: focused hint at cols 37 (width 36) - full `e edit  D delete  a ask  esc back` fits exactly.
#[test]
fn f_comment_04_hint_37() {
    let mut s = sim(37, 16, &[("f.txt", "alpha\n")]);
    s.keys("ahi<Enter>J");
    m(&s, 5, r"^. ▸ sent");
    m(&s, 7, r"^. e edit  D delete  a ask  esc back.$");
}

/// F-COMMENT-04: comment box shows up to 3 quoted selection lines then ` … +k more`.
#[test]
fn f_comment_04_quotes() {
    let mut s = sim(80, 30, &[("f.txt", "l1\nl2\nl3\nl4\nl5\nl6\n")]);
    s.keys("VjjjjaQ<Enter>");
    m(&s, 7, r"^        5 \+▶l5");
    m(&s, 8, r"^╭─+╮$");
    m(&s, 9, r"^│ sent  selection L1-5  saved · not asked +│$");
    m(&s, 10, r"^│ > l1 +│$");
    m(&s, 11, r"^│ > l2 +│$");
    m(&s, 12, r"^│ > l3 +│$");
    m(&s, 13, r"^│ … \+2 more +│$");
    m(&s, 14, r"^│ Q +│$");
    m(&s, 15, r"^╰─+╯$");
    e(&s, 16, "        6 + l6");
    s.assert_not_contains("> l4");
    s.keys("kkkkVjjaR<Enter>");
    // exactly 3 lines - no more row
    m(&s, 5, r"^        3 \+▶l3");
    m(&s, 6, r"^╭─+╮$");
    m(&s, 7, r"^│ sent  selection L1-3  saved · not asked +│$");
    m(&s, 10, r"^│ > l3 +│$");
    m(&s, 11, r"^│ R +│$");
    m(&s, 12, r"^╰─+╯$");
}

/// F-COMMENT-04: body wrapped to box width-3 at spaces. Box width 29, wrap width 26.
#[test]
fn f_comment_04_wrap() {
    let mut s = sim(30, 16, &[("f.txt", "alpha\n")]);
    s.keys("aaaaa bbbb cccc dddd eeee ffff gggg<Enter>");
    m(&s, 4, r"^╭─+╮$");
    m(&s, 5, r"^│ sent  line L1 ");
    m(&s, 6, r"^│ aaaa bbbb cccc dddd eeee +│$");
    m(&s, 7, r"^│ ffff gggg +│$");
    m(&s, 8, r"^╰─+╯$");
}

// ---------------------------------------------------------------------------------------------------------------
// F-COMMENT-05
// ---------------------------------------------------------------------------------------------------------------

/// F-COMMENT-05: box visible in browse of the same path, on the row with that line number.
#[test]
fn f_comment_05_browse() {
    let mut s = sim(80, 24, &[("f.txt", "one\ntwo\nthree\n")]);
    s.keys("jadiffnote<Enter>");
    m(&s, 4, r"^        2 \+▶two");
    m(&s, 7, r"^│ diffnote +│$");
    s.keys("F");
    s.keys("f.txt<Enter>");
    m(&s, 0, r"^\[browse\] .*\[cursor L1:C1\] f\.txt");
    m(&s, 2, r"^   1  ▶one");
    e(&s, 3, "   2   two");
    m(&s, 4, r"^╭─+╮$");
    m(&s, 5, r"^│ sent  line L2 ");
    m(&s, 6, r"^│ diffnote +│$");
    e(&s, 8, "   3   three");
}

/// F-COMMENT-05: comment on an unpaired unified del row shows in split under the row with empty right side and
/// left old no.
#[test]
fn f_comment_05_deleted_row() {
    let mut s = sim(120, 30, &[]);
    commit_base(&s, "a\nb\nc\n");
    s.write_file("f.txt", "a\nc\n");
    s.keys("r");
    // unified: 3 ctx a (1 1), 4 del b (old 2), 5 ctx c (old 3 new 2)
    m(&s, 0, r"\[cursor L2:C1\] ");
    m(&s, 4, r"^   2      -▶b");
    s.keys("adelnote<Enter>");
    m(&s, 5, r"^╭─+╮$");
    m(&s, 7, r"^│ delnote +│$");
    m(&s, 9, r"^   3    2   c");
    s.keys("s");
    k(&s, 0, "[split]");
    m(&s, 4, r"^   2 -.b\s+…?│\s*$");
    m(&s, 5, r"^╭─+╮$");
    m(&s, 6, r"^│ sent  line L2 ");
    m(&s, 7, r"^│ delnote +│$");
    m(&s, 9, r"^   3   c\s+│   2   c");
    s.keys("s");
    m(&s, 4, r"^   2      -.b");
    m(&s, 7, r"^│ delnote +│$");
    m(&s, 9, r"^   3    2   c");
}

/// F-COMMENT-05: hunk-row comment (no line number) anchored by row index - stays under hunk row across split toggle.
#[test]
fn f_comment_05_hunk_row() {
    let mut s = sim(120, 30, &[]);
    commit_base(&s, "a\nb\nc\n");
    s.write_file("f.txt", "a\nB\nc\n");
    s.keys("rg");
    m(&s, 0, r"\[cursor r1:C1\] ");
    s.keys("ahunk<Enter>");
    m(&s, 2, r"^@@ ");
    m(&s, 3, r"^╭─+╮$");
    m(&s, 4, r"^│ sent  line r1 ");
    m(&s, 7, r"^   1    1   a");
    s.keys("s");
    k(&s, 0, "[split]");
    m(&s, 2, r"^@@ ");
    m(&s, 3, r"^╭─+╮$");
    m(&s, 4, r"^│ sent  line r1 ");
    m(&s, 7, r"^   1   a\s+│   1   a");
}

/// F-COMMENT-05: box on new-side line survives unified/split toggle, scope toggle and reload; hidden while the
/// line is absent, back when present.
#[test]
fn f_comment_05_layout_scope_reload() {
    let mut s = sim(120, 30, &[]);
    commit_base(&s, "alpha\nbravo\ncharlie\n");
    s.write_file("f.txt", "alpha\nBRAVO two\ncharlie\ndelta\n");
    s.keys("r");
    // unified rows: 3 ctx L1, 4 del L2, 5 add L2, 6 ctx L3 (old 3 new 3), 7 add L4
    e(&s, 5, "        2 + BRAVO two");
    e(&s, 7, "        4 + delta");
    s.keys(":4<Enter>");
    m(&s, 0, r"\[cursor L4:C1\] f\.txt ");
    s.keys("anote4<Enter>");
    m(&s, 7, r"^        4 \+▶delta");
    m(&s, 9, r"^│ sent  line L4 ");
    s.keys("s");
    // split - box under the row whose right side is new L4
    k(&s, 0, "[split]");
    m(&s, 6, r"^\s+│   4 \+.delta");
    m(&s, 7, r"^╭─+╮$");
    m(&s, 8, r"^│ sent  line L4 ");
    m(&s, 9, r"^│ note4 +│$");
    s.keys("s");
    k(&s, 0, "[unified]");
    m(&s, 7, r"^        4 \+.delta");
    m(&s, 9, r"^│ sent  line L4 ");
    s.keys("c");
    k(&s, 0, "[changes]");
    s.assert_matches(r"        4 \+.delta.*\n╭─+╮\n│ sent  line L4 ");
    s.keys("cr");
    k(&s, 0, "[full]");
    s.assert_matches(r"        4 \+.delta.*\n╭─+╮\n│ sent  line L4 ");
    // line 4 gone from the new side - box hidden
    s.write_file("f.txt", "alpha\nBRAVO two\ncharlie\n");
    s.keys("r");
    for t in ["sent  line L4", "note4", "delta", "╭"] {
        s.assert_not_contains(t);
    }
    m(&s, 0, r"\+1 -1$");
    // line back - comment was kept, box shown again
    s.write_file("f.txt", "alpha\nBRAVO two\ncharlie\ndelta\n");
    s.keys("r");
    s.assert_matches(r"        4 \+.delta.*\n╭─+╮\n│ sent  line L4 .*\n│ note4 +│");
}

/// F-COMMENT-05: several boxes on one row are shown in creation order.
#[test]
fn f_comment_05_order() {
    let mut s = sim(80, 24, &[("f.txt", "one\ntwo\n")]);
    s.keys("afirst<Enter>asecond<Enter>athird<Enter>");
    m(&s, 3, r"^        1 \+▶one");
    m(&s, 6, r"^│ first +│$");
    m(&s, 10, r"^│ second +│$");
    m(&s, 14, r"^│ third +│$");
    e(&s, 16, "        2 + two");
    k(&s, -1, "question saved (3)");
}

/// F-COMMENT-05: old-pane comment in split anchors to the unified del row with that old number (not the paired
/// add row).
#[test]
fn f_comment_05_pane_old() {
    let mut s = sim_split(120, 30);
    commit_base(&s, "alpha\nbravo\ncharlie\n");
    s.write_file("f.txt", "alpha\nBRAVO two\ncharlie\ndelta\n");
    s.keys("r");
    m(&s, 0, r"\[cursor new L2:C1\] ");
    m(&s, 4, r"^   2 -.bravo");
    s.keys("p");
    m(&s, 0, r"\[cursor old L2:C1\] ");
    s.keys("aoldside<Enter>");
    m(&s, 4, r"^   2 -.bravo.*│   2 \+.BRAVO two");
    m(&s, 5, r"^╭─+╮$");
    m(&s, 7, r"^│ oldside +│$");
    m(&s, 9, r"^   3   charlie");
    s.keys("s");
    // unified - del row (old 2) carries the box; add row L2 comes after it
    k(&s, 0, "[unified]");
    m(&s, 4, r"^   2      -.bravo");
    m(&s, 5, r"^╭─+╮$");
    m(&s, 6, r"^│ sent  line L2 ");
    m(&s, 7, r"^│ oldside +│$");
    m(&s, 8, r"^╰─+╯$");
    m(&s, 9, r"^        2 \+.BRAVO two");
}

/// F-COMMENT-05: only boxes of the shown file are visible.
#[test]
fn f_comment_05_per_file() {
    let mut s = sim(80, 20, &[("a.txt", "one\ntwo\n"), ("b.txt", "one\ntwo\n")]);
    s.keys("aina<Enter>");
    k(&s, 0, "a.txt");
    m(&s, 6, r"^│ ina +│$");
    s.keys("<Tab>");
    m(&s, 0, r"\[2/2\] .*b\.txt ");
    e(&s, 4, "        2 + two");
    s.assert_not_contains("ina");
    s.assert_not_contains("╭");
    s.keys("<S-Tab>");
    k(&s, 0, "a.txt");
    m(&s, 6, r"^│ ina +│$");
}

// ---------------------------------------------------------------------------------------------------------------
// F-VISUAL-01
// ---------------------------------------------------------------------------------------------------------------

const VTXT: &str = "alpha beta\ntwo words here\n\nfourth line\nfifth\n";

/// F-VISUAL-01: file switch (Tab) ends the selection; coming back shows no selection.
#[test]
fn f_visual_01_ended_by_file_change() {
    let mut s = sim(80, 20, &[("a.txt", "one\ntwo\nthree\n"), ("b.txt", "four\nfive\n")]);
    m(&s, 0, r"\[1/2\] \[cursor L1:C1\] a\.txt ");
    s.keys("Vj");
    m(&s, 0, r"\[1/2\] \[visual L2:C1\] a\.txt ");
    cl(&s, 3, 12, 'o', "bg:#6b4f00");
    s.keys("<Tab>");
    m(&s, 0, r"\[2/2\] \[cursor L1:C1\] b\.txt ");
    m(&s, -1, r"hjkl move  enter ask  J/K comments  \? help$");
    s.keys("j");
    cl(&s, 3, 12, 'f', "bg:default");
    s.keys("<S-Tab>");
    m(&s, 0, r"\[1/2\] \[cursor L1:C1\] a\.txt ");
    cl(&s, 3, 13, 'n', "bg:#22586b");
    cl(&s, 4, 12, 't', "bg:default");
}

/// F-VISUAL-01: comment submit and J/K comment focus end an active selection.
#[test]
fn f_visual_01_ended_by_focus_submit() {
    let mut s = sim(80, 30, &[("f.txt", VTXT)]);
    s.keys("Vj");
    m(&s, 0, r"\[visual L2:C1\] f\.txt ");
    s.keys("a");
    s.assert_contains("selection L1-2");
    // submit closes editor and ends selection
    s.keys("x<Enter>");
    m(&s, 0, r"\[cursor L2:C1\] f\.txt ");
    k(&s, -1, "question saved (1)");
    cl(&s, 3, 12, 'a', "bg:default");
    s.keys("jjvj");
    m(&s, 0, r"\[visual L5:C1\] f\.txt ");
    s.keys("K");
    // focus moves cursor to the comment row and ends selection
    m(&s, 0, r"\[cursor L2:C1\] f\.txt ");
    k(&s, -1, "e edit  D delete");
    s.assert_contains("▸ sent  selection L1-2");
    ct(&s, "fourth", None, 0, '\0', "bg:default");
    ct(&s, "fifth", None, 0, '\0', "bg:default");
    s.keys("<Esc>vj");
    m(&s, 0, r"\[visual L3:C1\] f\.txt ");
    s.keys("J");
    m(&s, 0, r"\[cursor L2:C1\] f\.txt ");
    s.assert_contains("▸ sent  selection L1-2");
}

/// F-VISUAL-01: goto jump and find jump end an active selection.
#[test]
fn f_visual_01_ended_by_jumps() {
    let mut s = sim(80, 20, &[("f.txt", VTXT)]);
    s.keys("Vj");
    m(&s, 0, r"\[visual L2:C1\] f\.txt ");
    cl(&s, 3, 12, 'a', "bg:#6b4f00");
    s.keys(":4<Enter>");
    m(&s, 0, r"\[cursor L4:C1\] f\.txt ");
    cl(&s, 3, 12, 'a', "bg:default");
    cl(&s, 4, 12, 't', "bg:default");
    cl(&s, 6, 13, 'o', "bg:#22586b");
    s.keys("vk");
    m(&s, 0, r"\[visual L3:C1\] f\.txt ");
    cl(&s, 6, 12, 'f', "bg:#6b4f00");
    s.keys("/fifth<Enter>");
    m(&s, 0, r"\[cursor L5:C1\] f\.txt ");
    cl(&s, 6, 12, 'f', "bg:default");
    cl(&s, 5, 12, '\0', "bg:default");
}

/// F-VISUAL-01: numbered comment jump `)` ends an active selection.
#[test]
fn f_visual_01_ended_by_numbered_jump() {
    let mut s = sim_mcp(80, 30, &[("f.txt", VTXT)]);
    k(&s, 0, "[mcp: on]");
    annotate_ok(&mut s, json!({"file": "f.txt", "line": 4, "text": "look here", "number": 1}));
    s.assert_contains("#1 agent note L4");
    s.keys("Vj");
    m(&s, 0, r"\[visual L2:C1\] f\.txt ");
    cl(&s, 3, 12, 'a', "bg:#6b4f00");
    s.keys(")");
    m(&s, 0, r"\[cursor L4:C1\] f\.txt ");
    s.assert_contains("▸ sent  #1 agent note L4");
    cl(&s, 3, 12, 'a', "bg:default");
    cl(&s, 4, 12, 't', "bg:default");
}

/// F-VISUAL-01: i is a no-op in visual (selection kept, still extendable); Esc ends; Esc again no-op.
#[test]
fn f_visual_01_esc_i() {
    let mut s = sim(80, 20, &[("f.txt", VTXT)]);
    s.keys("vl");
    m(&s, 0, r"\[visual L1:C2\] f\.txt ");
    s.keys("i");
    m(&s, 0, r"\[visual L1:C2\] f\.txt ");
    m(&s, -1, r"v/esc end  enter ask  hjkl move  \? help$");
    s.assert_not_contains("╭");
    cl(&s, 3, 12, 'a', "bg:#6b4f00");
    s.keys("l");
    m(&s, 0, r"\[visual L1:C3\] f\.txt ");
    cl(&s, 3, 13, 'l', "bg:#6b4f00 !inv");
    cl(&s, 3, 14, 'p', "bg:#6b4f00 inv");
    s.keys("<Esc>");
    m(&s, 0, r"\[cursor L1:C3\] f\.txt \+5 -0$");
    m(&s, -1, r"\(1-6/6\) hjkl move  enter ask  J/K comments  \? help$");
    cl(&s, 3, 12, 'a', "bg:#22586b");
    cl(&s, 3, 13, 'l', "bg:#22586b");
    s.keys("<Esc>");
    m(&s, 0, r"\[cursor L1:C3\] f\.txt \+5 -0$");
    m(&s, 3, r"^        1 \+▶alpha beta");
    m(&s, -1, r"\(1-6/6\) hjkl move  enter ask  J/K comments  \? help$");
}

/// F-VISUAL-01: cursor motions (w e j $ G count k) extend an active char selection from the fixed anchor.
/// Rows: 3 L1 `alpha beta`, 4 L2 `two words here`, 5 L3 empty, 6 L4 `fourth line`, 7 L5 `fifth`; code col 12.
#[test]
fn f_visual_01_motions_extend() {
    let mut s = sim(80, 20, &[("f.txt", VTXT)]);
    s.keys("vw");
    m(&s, 0, r"\[visual L1:C7\] f\.txt ");
    cl(&s, 3, 17, ' ', "bg:#6b4f00");
    cl(&s, 3, 18, 'b', "bg:#6b4f00 inv");
    cl(&s, 3, 19, 'e', "bg:#22586b");
    s.keys("e");
    m(&s, 0, r"\[visual L1:C10\] f\.txt ");
    cl(&s, 3, 20, 't', "bg:#6b4f00 !inv");
    cl(&s, 3, 21, 'a', "bg:#6b4f00 inv");
    s.keys("j$");
    m(&s, 0, r"\[visual L2:C14\] f\.txt ");
    cl(&s, 3, 12, 'a', "bg:#6b4f00");
    cl(&s, 4, 24, 'r', "bg:#6b4f00");
    cl(&s, 4, 25, 'e', "bg:#6b4f00 inv");
    s.keys("G");
    // $ sticky - last char of L5; middle rows fully selected, empty row one cell
    m(&s, 0, r"\[visual L5:C5\] f\.txt ");
    cl(&s, 5, 12, '\0', "bg:#6b4f00");
    cl(&s, 5, 13, '\0', "bg:default");
    cl(&s, 6, 22, 'e', "bg:#6b4f00");
    cl(&s, 7, 16, 'h', "bg:#6b4f00 inv");
    s.keys("3k");
    // back up to L2; rows below cursor deselected
    m(&s, 0, r"\[visual L2:C14\] f\.txt ");
    cl(&s, 5, 12, '\0', "bg:default");
    cl(&s, 6, 12, 'f', "bg:default");
    cl(&s, 4, 12, 't', "bg:#6b4f00");
    s.keys("0");
    m(&s, 0, r"\[visual L2:C1\] f\.txt ");
    cl(&s, 4, 12, 't', "bg:#6b4f00 inv");
    cl(&s, 4, 13, 'w', "bg:#22586b");
}

/// F-VISUAL-01: p in split ends the selection and toggles the pane.
/// Split W=59: left pane code col 7, separator col 59, right pane code col 67; row 4 = bravo | BRAVO two.
#[test]
fn f_visual_01_p_ends() {
    let mut s = sim_split(120, 20);
    commit_base(&s, "alpha\nbravo\ncharlie\n");
    s.write_file("f.txt", "alpha\nBRAVO two\ncharlie\ndelta\n");
    s.keys("r");
    m(&s, 0, r"\[cursor new L2:C1\] f\.txt \+2 -1$");
    m(&s, 4, r"^   2 -▶bravo\s+…?│   2 \+▶BRAVO two");
    m(&s, -1, r"hjkl move  enter ask  J/K comments  p pane  \? help$");
    s.keys("vl");
    m(&s, 0, r"\[visual new L2:C2\] f\.txt ");
    cl(&s, 4, 67, 'B', "bg:#6b4f00");
    s.keys("p");
    m(&s, 0, r"\[cursor old L2:C2\] f\.txt ");
    m(&s, -1, r"hjkl move  enter ask  J/K comments  p pane  \? help$");
    cl(&s, 4, 67, 'B', "bg:#22586b");
    cl(&s, 4, 68, 'R', "bg:#22586b !inv");
    cl(&s, 4, 7, 'b', "bg:#22586b !inv");
    cl(&s, 4, 8, 'r', "inv");
}

/// F-VISUAL-01: V starts line selection; v/V while active switches kind with anchor kept; same key again ends.
#[test]
fn f_visual_01_switch_kind() {
    let mut s = sim(80, 20, &[("f.txt", VTXT)]);
    s.keys("llvj");
    // char selection L1:C3 .. L2:C3
    m(&s, 0, r"\[visual L2:C3\] f\.txt ");
    cl(&s, 3, 13, 'l', "bg:default");
    cl(&s, 3, 14, 'p', "bg:#6b4f00");
    cl(&s, 4, 15, ' ', "bg:#22586b");
    s.keys("V");
    // switched to line kind - whole text of both rows selected, cursor unchanged
    m(&s, 0, r"\[visual L2:C3\] f\.txt ");
    cl(&s, 3, 12, 'a', "bg:#6b4f00 fg:#fdf6e3");
    cl(&s, 3, 21, 'a', "bg:#6b4f00");
    cl(&s, 4, 25, 'e', "bg:#6b4f00");
    s.keys("v");
    // back to char kind - anchor still L1:C3
    m(&s, 0, r"\[visual L2:C3\] f\.txt ");
    cl(&s, 3, 13, 'l', "bg:default");
    cl(&s, 3, 14, 'p', "bg:#6b4f00");
    cl(&s, 4, 15, ' ', "bg:#22586b");
    s.keys("V");
    cl(&s, 3, 12, 'a', "bg:#6b4f00");
    s.keys("V");
    // same key again ends
    m(&s, 0, r"\[cursor L2:C3\] f\.txt ");
    cl(&s, 3, 12, 'a', "bg:default");
    cl(&s, 4, 12, 't', "bg:#22586b");
    s.keys("V");
    m(&s, 0, r"\[visual L2:C3\] ");
    // V anchored on L2 only - L1 not selected
    cl(&s, 3, 12, 'a', "bg:default");
    cl(&s, 4, 12, 't', "bg:#6b4f00");
}

/// F-VISUAL-01: v starts char selection at cursor (visual tag + footer), moves extend it, v again ends it.
/// Untracked f.txt shown fully added: row 2 hunk, rows 3.. = L1..; code starts col 12; cursor starts L1:C1.
#[test]
fn f_visual_01_v_toggle() {
    let mut s = sim(80, 20, &[("f.txt", VTXT)]);
    m(&s, 0, r"\[cursor L1:C1\] f\.txt \+5 -0$");
    m(&s, 3, r"^        1 \+▶alpha beta");
    m(&s, -1, r"\(1-6/6\) hjkl move  enter ask  J/K comments  \? help$");
    // no selection yet - code cells on cursor row have curBg only
    cl(&s, 3, 13, 'l', "bg:#22586b !inv");
    s.keys("v");
    m(&s, 0, r"\[visual L1:C1\] f\.txt \+5 -0$");
    m(&s, -1, r"\(1-6/6\) v/esc end  enter ask  hjkl move  \? help$");
    ct(&s, "[visual", Some(0), 1, '\0', "fg:#cb4b16 bold");
    // anchor = cursor char, reverse inside selection
    cl(&s, 3, 12, 'a', "fg:#fdf6e3 bg:#6b4f00 inv");
    cl(&s, 3, 13, 'l', "bg:#22586b !inv");
    s.keys("ll");
    m(&s, 0, r"\[visual L1:C3\] f\.txt ");
    cl(&s, 3, 12, 'a', "fg:#fdf6e3 bg:#6b4f00 !inv");
    cl(&s, 3, 13, 'l', "fg:#fdf6e3 bg:#6b4f00 !inv");
    cl(&s, 3, 14, 'p', "fg:#fdf6e3 bg:#6b4f00 inv");
    cl(&s, 3, 15, 'h', "bg:#22586b");
    s.keys("v");
    m(&s, 0, r"\[cursor L1:C3\] f\.txt \+5 -0$");
    m(&s, -1, r"\(1-6/6\) hjkl move  enter ask  J/K comments  \? help$");
    cl(&s, 3, 12, 'a', "bg:#22586b !inv");
    cl(&s, 3, 13, 'l', "bg:#22586b !inv");
    // moves after ending do not select
    s.keys("h");
    m(&s, 0, r"\[cursor L1:C2\] ");
    cl(&s, 3, 12, 'a', "bg:#22586b !inv");
}

// ---------------------------------------------------------------------------------------------------------------
// F-VISUAL-02
// ---------------------------------------------------------------------------------------------------------------

/// F-VISUAL-02: backward char selection - from cursor col on first row to anchor col on last row.
#[test]
fn f_visual_02_char_backward() {
    let mut s = sim(80, 20, &[("f.txt", VTXT)]);
    s.keys("jlllvkh");
    // anchor L2:C4, cursor L1:C3
    m(&s, 0, r"\[visual L1:C3\] f\.txt ");
    cl(&s, 3, 13, 'l', "bg:#22586b");
    cl(&s, 3, 14, 'p', "bg:#6b4f00 inv");
    cl(&s, 3, 21, 'a', "bg:#6b4f00");
    cl(&s, 4, 12, 't', "bg:#6b4f00");
    cl(&s, 4, 15, ' ', "bg:#6b4f00");
    cl(&s, 4, 16, 'w', "bg:default");
}

/// F-VISUAL-02: char selection over rows - first row from anchor col, middle rows full, empty row one cell, last
/// row to cursor col.
#[test]
fn f_visual_02_char_multirow() {
    let mut s = sim(80, 20, &[("f.txt", VTXT)]);
    s.keys("llvjjjh");
    m(&s, 0, r"\[visual L4:C2\] f\.txt ");
    // first row - before anchor col not selected, anchor col through end selected, past end not
    cl(&s, 3, 13, 'l', "bg:default");
    cl(&s, 3, 14, 'p', "fg:#fdf6e3 bg:#6b4f00");
    cl(&s, 3, 21, 'a', "fg:#fdf6e3 bg:#6b4f00");
    cl(&s, 3, 22, '\0', "bg:default");
    // middle row full text only
    cl(&s, 4, 12, 't', "bg:#6b4f00");
    cl(&s, 4, 25, 'e', "bg:#6b4f00");
    cl(&s, 4, 26, '\0', "bg:default");
    // empty row - exactly one highlighted cell
    cl(&s, 5, 12, ' ', "fg:#fdf6e3 bg:#6b4f00");
    cl(&s, 5, 13, '\0', "bg:default");
    // last row to cursor col, cursor char reverse with same colors
    cl(&s, 6, 12, 'f', "fg:#fdf6e3 bg:#6b4f00 !inv");
    cl(&s, 6, 13, 'o', "fg:#fdf6e3 bg:#6b4f00 inv");
    cl(&s, 6, 14, 'u', "bg:#22586b !inv");
    // rows outside range untouched
    cl(&s, 7, 12, 'f', "bg:default");
}

/// F-VISUAL-02: line selection highlights whole text of each row (not past it); empty row one cell.
#[test]
fn f_visual_02_line() {
    let mut s = sim(80, 20, &[("f.txt", VTXT)]);
    s.keys("jlVj");
    m(&s, 0, r"\[visual L3:C1\] f\.txt ");
    cl(&s, 3, 12, 'a', "bg:default");
    cl(&s, 4, 12, 't', "fg:#fdf6e3 bg:#6b4f00");
    cl(&s, 4, 13, 'w', "fg:#fdf6e3 bg:#6b4f00 !inv");
    cl(&s, 4, 25, 'e', "bg:#6b4f00");
    cl(&s, 4, 26, '\0', "bg:default");
    // empty cursor row - one selected cell, reverse (cursor)
    cl(&s, 5, 12, ' ', "fg:#fdf6e3 bg:#6b4f00 inv");
    cl(&s, 5, 13, '\0', "bg:#22586b");
    s.keys("j");
    m(&s, 0, r"\[visual L4:C2\] f\.txt ");
    cl(&s, 5, 12, ' ', "bg:#6b4f00 !inv");
    cl(&s, 5, 13, '\0', "bg:default");
    cl(&s, 6, 12, 'f', "bg:#6b4f00 !inv");
    cl(&s, 6, 13, 'o', "bg:#6b4f00 inv");
    cl(&s, 6, 22, 'e', "bg:#6b4f00");
    cl(&s, 6, 23, '\0', "bg:#22586b");
    cl(&s, 7, 12, 'f', "bg:default");
}

/// F-VISUAL-02: split - selection drawn only in the active pane (new, then old after p).
/// Rows: 3 alpha|alpha, 4 bravo|BRAVO two (cursor), 5 charlie|charlie, 6 (empty)|delta; left code col 7, right 67.
#[test]
fn f_visual_02_split_pane() {
    let mut s = sim_split(120, 20);
    commit_base(&s, "alpha\nbravo\ncharlie\n");
    s.write_file("f.txt", "alpha\nBRAVO two\ncharlie\ndelta\n");
    s.keys("r");
    m(&s, 0, r"\[cursor new L2:C1\] f\.txt ");
    m(&s, 5, r"^   3   charlie\s+│   3   charlie");
    s.keys("Vj");
    m(&s, 0, r"\[visual new L3:C1\] f\.txt ");
    cl(&s, 4, 67, 'B', "bg:#6b4f00 fg:#fdf6e3");
    cl(&s, 4, 75, 'o', "bg:#6b4f00");
    cl(&s, 5, 73, 'e', "bg:#6b4f00");
    // old pane cells of the same rows not selected
    cl(&s, 4, 7, 'b', "bg:default");
    cl(&s, 5, 7, 'c', "bg:#22586b");
    cl(&s, 5, 8, 'h', "bg:#22586b");
    s.keys("kpvlj");
    m(&s, 0, r"\[visual old L3:C2\] f\.txt ");
    cl(&s, 4, 7, 'b', "bg:#6b4f00");
    cl(&s, 4, 11, 'o', "bg:#6b4f00");
    cl(&s, 5, 7, 'c', "bg:#6b4f00");
    cl(&s, 5, 8, 'h', "bg:#6b4f00 inv");
    cl(&s, 5, 9, 'a', "bg:#22586b");
    // new pane cells not selected; char cursor only in active pane
    cl(&s, 4, 68, 'R', "bg:default");
    cl(&s, 5, 67, 'c', "bg:#22586b !inv");
    cl(&s, 5, 68, 'h', "bg:#22586b !inv");
}

// ---------------------------------------------------------------------------------------------------------------
// F-VISUAL-03
// ---------------------------------------------------------------------------------------------------------------

/// F-VISUAL-03: char selection tags `L<a>:C<x>-C<y>` (same row) and `L<a>:C<x>-L<b>:C<y>` (rows differ).
#[test]
fn f_visual_03_char_tags() {
    let mut s = sim(80, 30, &[("f.txt", VTXT)]);
    s.keys("llvllll");
    m(&s, 0, r"\[visual L1:C7\] f\.txt ");
    s.keys("<Enter>");
    s.assert_contains("│ selection L1:C3-C7 ");
    s.assert_matches(r"│ > pha b\s+│");
    s.keys("<Esc><Esc>");
    m(&s, 0, r"\[cursor L1:C7\] f\.txt ");
    s.assert_not_contains("╭");
    s.keys("0llvjh");
    m(&s, 0, r"\[visual L2:C2\] f\.txt ");
    s.keys("a");
    s.assert_contains("│ selection L1:C3-L2:C2 ");
    s.assert_matches(r"│ > pha beta\s+│");
    s.assert_matches(r"│ > tw\s+│");
    s.keys("<Esc><Esc>");
    // backward selection - tag goes from first to last position
    s.keys(":4<Enter>lvkkkh");
    m(&s, 0, r"\[visual L1:C1\] f\.txt ");
    s.keys("a");
    s.assert_contains("│ selection L1:C1-L4:C2 ");
}

/// F-VISUAL-03: Enter / a on line selection open editor with head `selection L<a>` or `L<a>-<b>` (end label minus
/// first char).
#[test]
fn f_visual_03_line_tags() {
    let mut s = sim(80, 40, &[("f.txt", "l1\nl2\nl3\nl4\nl5\nl6\nl7\nl8\nl9\nl10\nl11\nl12\n")]);
    s.keys("V");
    s.assert_not_contains("selection L");
    s.keys("<Enter>");
    s.assert_contains("│ selection L1 ");
    s.assert_not_contains("selection L1-");
    m(&s, -1, r"enter send  tab save/ask  esc cancel$");
    ct(&s, "selection L1", None, 0, '\0', "fg:#cb4b16");
    s.keys("<Esc>");
    s.assert_not_contains("selection L");
    s.assert_not_contains("╭");
    s.keys("<Esc>9GVj");
    m(&s, 0, r"\[visual L9:C1\] f\.txt ");
    s.keys("a");
    s.assert_contains("│ selection L8-9 ");
    s.keys("<Esc><Esc>");
    s.keys("10GVjjj");
    m(&s, 0, r"\[visual L12:C1\] f\.txt ");
    s.keys("a");
    s.assert_contains("│ selection L9-12 ");
    s.assert_not_contains("selection L9-L12");
}

/// F-VISUAL-03: rows without number use `r<row+1>` labels (hunk row = r1); line and char forms.
#[test]
fn f_visual_03_row_labels() {
    let mut s = sim(80, 30, &[("f.txt", "alpha beta\ntwo words here\n")]);
    s.keys("g");
    m(&s, 0, r"\[cursor r1:C1\] f\.txt ");
    s.keys("Vj");
    m(&s, 0, r"\[visual L1:C1\] f\.txt ");
    s.keys("<Enter>");
    s.assert_contains("│ selection r1-1 ");
    s.keys("<Esc><Esc>gvjll");
    m(&s, 0, r"\[visual L1:C3\] f\.txt ");
    s.keys("<Enter>");
    s.assert_contains("│ selection r1:C1-L1:C3 ");
    s.keys("<Esc><Esc>gV");
    s.keys("<Enter>");
    s.assert_contains("│ selection r1 ");
    s.assert_not_contains("selection r1-");
}

/// F-VISUAL-03: del + add row both numbered L2 in unified give the single label `L2` / `L2:C1-C3`.
#[test]
fn f_visual_03_same_number() {
    let mut s = sim(80, 30, &[]);
    commit_base(&s, "# Title\nhello\n");
    s.write_file("f.txt", "# Title\nhello world\nmore\n");
    s.keys("r");
    m(&s, 0, r"\[cursor L2:C1\] f\.txt \+2 -1$");
    m(&s, 4, r"^   2      -▶hello");
    e(&s, 5, "        2 + hello world");
    s.keys("Vj");
    m(&s, 0, r"\[visual L2:C1\] f\.txt ");
    s.keys("a");
    s.assert_contains("│ selection L2 ");
    s.assert_not_contains("selection L2-");
    s.keys("<Esc><Esc>kvjll");
    m(&s, 0, r"\[visual L2:C3\] f\.txt ");
    s.keys("<Enter>");
    s.assert_contains("│ selection L2:C1-C3 ");
    s.assert_not_contains("selection L2:C1-L2");
}
