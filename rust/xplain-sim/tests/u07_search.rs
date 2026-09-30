//! Ported from `e2e/scenarios/u07-search`.

mod search_support;

use search_support::*;
use serde_json::json;
use xplain_sim::{CellExpect as C, Fixture, Http, Sim};

/// F-BROWSE-01: NUL within first 8000 bytes -> 'binary file, not shown'; NUL only after byte 8000 -> text shown
#[test]
fn f_browse_01_binary() {
    let mut s = Sim::builder()
        .fixture(Fixture::Empty)
        .size(80, 24)
        .args(["--unstaged"])
        .file("small.bin", "ab\0cd\n")
        .file("edge.bin", &[&"a".repeat(7999), "\0\n"].concat())
        .file("late.txt", &["first line\n", &"b".repeat(7989), "\0\n"].concat())
        .build();
    s.keys("Fsmall<Enter>");
    s.assert_not_contains("ab");
    s.assert_row_matches(0, r#"^\[browse\] .*\[cursor L1:C1\] small\.bin$"#);
    s.assert_row_matches(2, "binary file, not shown");
    s.keys("Fedge<Enter>");
    // NUL at byte 8000 (last byte of first 8000)
    s.assert_not_contains("aaaa");
    s.assert_row_matches(0, r#"^\[browse\] .*\[cursor L1:C1\] edge\.bin$"#);
    s.assert_row_matches(2, "binary file, not shown");
    s.keys("Flate<Enter>");
    // NUL at byte 8001 -> text (rows after row 1 not asserted, NUL rendering UNSPEC-26)
    s.assert_not_contains("binary file");
    s.assert_row_matches(0, r#"^\[browse\] .*\[cursor L1:C1\] late\.txt$"#);
    s.assert_row_matches(2, "^   1  ▶first line");
    s.assert_row_matches(-1, r#"^\(1-2/2\) "#);
}

/// F-BROWSE-01: read error with --cwd names <cwd>/<path> in the note; browse not opened
#[test]
fn f_browse_01_error_cwd() {
    let mut s = Sim::builder().size(200, 24).args(["--cwd", "${REPO}"]).build();
    s.remove_file("package.json");
    s.keys("Fjson<Enter>");
    s.assert_not_contains("Search (");
    s.assert_not_contains("[browse]");
    s.assert_row_matches(0, r#"^\[all\] .*README\.md "#);
    s.assert_row_contains(-1, &s.expand("cannot read ${REPO}/package.json: not found | (1-5/5) "));
}

/// F-BROWSE-01: read error (file not readable): note "cannot read <p>: permission denied", browse not opened
#[test]
fn f_browse_01_error_denied() {
    let mut s = Sim::builder().size(120, 24).build();
    chmod(&s, "package.json", 0o000);
    s.keys("Fjson<Enter>");
    s.assert_not_contains("Search (");
    s.assert_not_contains("[browse]");
    s.assert_row_matches(0, r#"^\[all\] .*README\.md "#);
    s.assert_row_matches(-1, r#"^cannot read package\.json: permission denied \| \(1-5/5\) "#);
    chmod(&s, "package.json", 0o644);
}

/// F-BROWSE-01: read error (tracked file deleted from worktree) -> note with error message, browse not opened
#[test]
fn f_browse_01_error() {
    let mut s = Sim::builder().size(120, 24).build();
    s.remove_file("package.json");
    s.keys("Fjson");
    s.assert_row_matches(7, r#"│> package\.json +│"#);
    s.keys("<Enter>");
    s.assert_not_contains("Search (");
    s.assert_row_matches(0, r#"^\[all\] \[full\] \[unified\] .*\[1/4\] \[cursor L2:C1\] README\.md "#);
    s.assert_row_matches(
        -1,
        r#"^cannot read package\.json: not found \| \(1-5/5\) hjkl move  enter ask  J/K comments  \? help$"#,
    );
}

/// F-BROWSE-01: line splitting: empty file = one empty row; only one trailing newline dropped; no final newline keeps last line
#[test]
fn f_browse_01_lines() {
    let mut s = Sim::builder()
        .fixture(Fixture::Empty)
        .size(80, 24)
        .args(["--unstaged"])
        .file("empty.txt", "")
        .file("blank.txt", "a\n\n")
        .file("nonl.txt", "a\nb")
        .file("multi.txt", "x\n\ny\n")
        .build();
    s.assert_row_contains(0, "No changes");
    s.keys("Fempty<Enter>");
    s.assert_row_matches(0, r#"^\[browse\] .*\[cursor L1:C1\] empty\.txt$"#);
    s.assert_row_matches(2, "^   1  ▶[ …]*$");
    s.assert_row_matches(3, "^$");
    s.assert_row_matches(-1, r#"^\(1-1/1\) "#);
    s.keys("Fblank<Enter>");
    // "a\n\n" -> rows a, empty
    s.assert_row_matches(0, r#"^\[browse\] .*\[cursor L1:C1\] blank\.txt$"#);
    s.assert_row_matches(2, "^   1  ▶a");
    s.assert_row_matches(3, "^   2 *$");
    s.assert_row_matches(4, "^$");
    s.assert_row_matches(-1, r#"^\(1-2/2\) "#);
    s.keys("Fnonl<Enter>");
    s.assert_row_matches(0, r#"^\[browse\] .*\[cursor L1:C1\] nonl\.txt$"#);
    s.assert_row_matches(2, "^   1  ▶a");
    s.assert_row_matches(3, "^   2   b$");
    s.assert_row_matches(4, "^$");
    s.assert_row_matches(-1, r#"^\(1-2/2\) "#);
    s.keys("Fmulti<Enter>");
    s.assert_row_matches(0, r#"^\[browse\] .*\[cursor L1:C1\] multi\.txt$"#);
    s.assert_row_matches(2, "^   1  ▶x");
    s.assert_row_matches(3, "^   2 *$");
    s.assert_row_matches(4, "^   3   y$");
    s.assert_row_matches(5, "^$");
    s.assert_row_matches(-1, r#"^\(1-3/3\) "#);
}

/// F-BROWSE-01: browse opens file read-only; browse header with cursor tag; cursor row 1 col 1, top 0; trailing newline dropped
#[test]
fn f_browse_01_open() {
    let mut s = Sim::builder().size(80, 24).build();
    s.keys("<Tab>G");
    // diff view on src/big.ts, cursor moved to the end
    s.assert_row_matches(0, r#"^\[all\] .*\[2/4\] \[cursor L60:C1\] src/big\.ts "#);
    s.assert_row_matches(-1, r#"^\(42-62/62\) "#);
    s.keys("Fsbt<Enter>");
    // same file in browse starts at row 1, top 0 (60 lines, H=21)
    s.assert_not_contains("@@");
    s.assert_not_contains("+");
    s.assert_not_contains("Search (");
    s.assert_row_matches(0, r#"^\[browse\] \[solarized\] \[mcp: off\] \[cursor L1:C1\] src/big\.ts$"#);
    s.assert_row_matches(1, "^─{79}$");
    s.assert_row_matches(2, "^   1  ▶const v1 = 1;");
    s.assert_row_matches(3, "^   2   const v2 = 1;$");
    s.assert_row_matches(22, "^  21   const v21 = 1;$");
    s.assert_row_matches(-1, r#"^\(1-21/60\) hjkl move  enter ask  J/K comments  \? help$"#);
    s.keys("Fjson<Enter>");
    // file not in diff; single trailing newline dropped -> 1 row
    s.assert_row_matches(0, r#"^\[browse\] \[solarized\] \[mcp: off\] \[cursor L1:C1\] package\.json$"#);
    s.assert_row_matches(2, r#"^   1  ▶\{"name":"demo","v":2\}"#);
    s.assert_row_matches(3, "^$");
    s.assert_row_matches(-1, r#"^\(1-1/1\) hjkl move  enter ask  J/K comments  \? help$"#);
    // char cursor on col 1 (reverse video)
    s.assert_cell(7, 2, C::new().ch('{').reverse(true));
}

/// F-BROWSE-02: question asked from browse of a file not in the diff carries the browse path and line text to the agent
#[test]
fn f_browse_02_ask() {
    let mut s = Sim::builder().size(80, 24).config_json(json!({"mcp":{"autostart":true}})).build();
    s.assert_row_contains(0, "[mcp: on]");
    s.keys("Fjson<Enter>");
    s.assert_row_matches(0, r#"^\[browse\] \[solarized\] \[mcp: on\] \[cursor L1:C1\] package\.json$"#);
    let poll = s.http_start(Http::tool("next_question", json!({"wait_seconds": 60})));
    s.keys("a");
    s.keys("what is v?<Enter>");
    s.assert_row_contains(-1, "question sent to agent");
    let r = s.http_await(&poll);
    assert_eq!(r.status, 200);
    let q = r.tool_result();
    assert_eq!(q["status"], "question");
    let text = q["question"].as_str().unwrap_or_default();
    let re = regex::Regex::new(r#"^what is v\?\n\nFile: package\.json\nSide: new \(after the change\)\nLines: 1\n[\s\S]*\{"name":"demo","v":2\}"#)
        .unwrap();
    assert!(re.is_match(text), "question: {text:?}");
}

/// F-BROWSE-02: comment made in browse belongs to the browse path (shown on that file's diff line, not on other files); E exports it
#[test]
fn f_browse_02_comment() {
    let mut s = Sim::builder().size(80, 24).build();
    s.keys("Fsbt<Enter>2j");
    s.assert_row_matches(0, r#"^\[browse\] .*\[cursor L3:C1\] src/big\.ts$"#);
    s.keys("a");
    s.keys("why v3<Enter>");
    s.assert_row_matches(4, "^   3  ▶const v3 = 1;");
    s.assert_row_matches(6, "sent  line L3");
    s.assert_row_matches(7, "why v3");
    s.assert_row_contains(-1, "question saved (1) | ");
    s.keys("<Esc>");
    // back to diff README.md; comment of src/big.ts not shown there
    s.assert_not_contains("why v3");
    s.assert_row_matches(0, r#"^\[all\] .*\[1/4\] \[cursor L2:C1\] README\.md "#);
    s.keys("<Tab>g");
    // diff of src/big.ts shows the browse comment on new line 3
    s.assert_row_matches(0, r#"\[2/4\] \[cursor r1:C1\] src/big\.ts "#);
    s.assert_row_matches(5, "^   3    3   const v3 = 1;");
    s.assert_row_matches(7, "sent  line L3");
    s.assert_row_matches(8, "why v3");
    s.keys("E");
    s.assert_row_matches(-1, "^exported 1 comment -> ");
    // the export names the browse file
    let exports: Vec<String> = s
        .list_dir(".")
        .into_iter()
        .filter(|n| n.starts_with("xplain-review-") && n.ends_with(".md"))
        .collect();
    assert_eq!(exports.len(), 1, "exports: {exports:?}");
    let md = s.file(&exports[0]);
    assert!(md.contains("## `src/big.ts`\n"), "export: {md}");
    assert!(md.contains("> why v3"), "export: {md}");
}

/// F-BROWSE-02: cursor keys, counts and visual work in browse (single pane)
#[test]
fn f_browse_02_cursor() {
    let mut s = Sim::builder().size(80, 24).build();
    s.keys("Fsbt<Enter>");
    s.assert_row_matches(0, r#"\[cursor L1:C1\] src/big\.ts$"#);
    s.assert_row_matches(-1, r#"^\(1-21/60\) "#);
    s.keys("j");
    s.assert_row_matches(0, r#"\[cursor L2:C1\] src/big\.ts$"#);
    s.assert_row_matches(3, "^   2  ▶const v2");
    s.keys("3j");
    s.assert_row_matches(0, r#"\[cursor L5:C1\] src/big\.ts$"#);
    s.keys("<Up>");
    s.assert_row_matches(0, r#"\[cursor L4:C1\] src/big\.ts$"#);
    s.keys("G");
    s.assert_row_matches(0, r#"\[cursor L60:C1\] src/big\.ts$"#);
    s.assert_row_matches(-1, r#"^\(40-60/60\) "#);
    s.keys("g");
    s.assert_row_matches(0, r#"\[cursor L1:C1\] src/big\.ts$"#);
    s.assert_row_matches(-1, r#"^\(1-21/60\) "#);
    s.keys("10G");
    // no hunk row in browse -> row 10 = L10
    s.assert_row_matches(0, r#"\[cursor L10:C1\] src/big\.ts$"#);
    s.keys("d");
    // half page = floor(21/2) = 10 rows
    s.assert_row_matches(0, r#"\[cursor L20:C1\] src/big\.ts$"#);
    s.keys("u");
    s.assert_row_matches(0, r#"\[cursor L10:C1\] src/big\.ts$"#);
    s.keys("<Space>");
    // page = H-1 = 20 rows
    s.assert_row_matches(0, r#"\[cursor L30:C1\] src/big\.ts$"#);
    s.keys("<PageUp>");
    s.assert_row_matches(0, r#"\[cursor L10:C1\] src/big\.ts$"#);
    s.keys("l");
    s.assert_row_matches(0, r#"\[cursor L10:C2\] src/big\.ts$"#);
    s.keys("$");
    // "const v10 = 1;" last char col 14
    s.assert_row_matches(0, r#"\[cursor L10:C14\] src/big\.ts$"#);
    s.keys("0");
    s.assert_row_matches(0, r#"\[cursor L10:C1\] src/big\.ts$"#);
    s.keys("w");
    s.assert_row_matches(0, r#"\[cursor L10:C7\] src/big\.ts$"#);
    s.keys("2h");
    s.assert_row_matches(0, r#"\[cursor L10:C5\] src/big\.ts$"#);
    s.keys("V");
    s.assert_row_matches(0, r#"^\[browse\] .*\[visual L10:C5\] src/big\.ts$"#);
    s.assert_row_matches(-1, r#"^\(8-28/60\) v/esc end  enter ask  hjkl move  \? help$"#);
    s.keys("j");
    s.assert_row_matches(0, r#"\[visual L11:C5\] src/big\.ts$"#);
    // line selection covers row L10 text (visBg)
    let (x, y) = cell_pos(&s, "const v10", 0, 0, None);
    s.assert_cell(x, y, C::new().bg("#6b4f00"));
    s.keys("V");
    s.assert_row_matches(0, r#"^\[browse\] .*\[cursor L11:C5\] src/big\.ts$"#);
}

/// F-BROWSE-02: Esc in browse first ends selection, then unfocuses comment, only then returns to diff
#[test]
fn f_browse_02_esc_chain() {
    let mut s = Sim::builder().size(80, 24).build();
    s.keys("Fjson<Enter>");
    s.assert_row_matches(0, r#"^\[browse\] .*\[cursor L1:C1\] package\.json$"#);
    s.keys("vl");
    s.assert_row_matches(0, r#"^\[browse\] .*\[visual L1:C2\] package\.json$"#);
    s.assert_row_matches(-1, r#"^\(1-1/1\) v/esc end  enter ask  hjkl move  \? help$"#);
    s.keys("<Esc>");
    // first Esc only ends the selection
    s.assert_row_matches(0, r#"^\[browse\] .*\[cursor L1:C2\] package\.json$"#);
    s.assert_row_matches(-1, r#"^\(1-1/1\) hjkl move  enter ask  J/K comments  \? help$"#);
    s.keys("a");
    s.keys("note here<Enter>");
    s.assert_contains("note here");
    s.keys("J");
    s.assert_matches("▸ sent  line L1");
    s.keys("<Esc>");
    // Esc unfocuses the comment, stays in browse
    s.assert_contains("note here");
    s.assert_not_contains("▸");
    s.assert_row_matches(0, r#"^\[browse\] .*package\.json$"#);
    s.keys("<Esc>");
    // now back to diff
    s.assert_not_contains("note here");
    s.assert_row_matches(0, r#"^\[all\] .*\[1/4\] \[cursor L2:C1\] README\.md "#);
}

/// F-BROWSE-02: Esc in browse returns to the same diff file with initial position (F-NAV-08); one Esc, no extra step
#[test]
fn f_browse_02_esc() {
    let mut s = Sim::builder().size(80, 24).build();
    s.keys("<Tab>20j");
    s.assert_row_matches(0, r#"^\[all\] \[full\] \[unified\] .*\[2/4\] \[cursor L49:C1\] src/big\.ts "#);
    s.keys("FREADME<Enter>");
    s.assert_row_matches(0, r#"^\[browse\] .*\[cursor L1:C1\] README\.md$"#);
    s.assert_row_matches(-1, r#"^\(1-3/3\) "#);
    s.keys("jl");
    s.assert_row_matches(0, r#"\[cursor L2:C2\] README\.md$"#);
    s.keys("<Esc>");
    // back on src/big.ts (not README.md), cursor at first change (L30), top = firstChange-3
    s.assert_not_contains("[browse]");
    s.assert_row_matches(0, r#"^\[all\] \[full\] \[unified\] .*\[2/4\] \[cursor L30:C1\] src/big\.ts "#);
    s.assert_row_matches(-1, r#"^\(28-48/62\) hjkl move  enter ask  J/K comments  \? help$"#);
    s.keys("<Esc>");
    // Esc in diff with nothing to undo is a no-op
    s.assert_row_matches(0, r#"\[2/4\] \[cursor L30:C1\] src/big\.ts "#);
    s.assert_row_matches(-1, r#"^\(28-48/62\) "#);
}

/// F-BROWSE-02: t, C, M, E, F, /, n, N, :, r, q, ? work in browse
#[test]
fn f_browse_02_global() {
    let mut s = Sim::builder().size(120, 30).config_json(json!({"app":{"confirmQuit":true}})).build();
    s.keys("Fsbt<Enter>");
    s.assert_row_matches(0, r#"^\[browse\] \[solarized\] \[mcp: off\] \[cursor L1:C1\] src/big\.ts$"#);
    s.keys("t");
    // theme cycles, browse header chip updates
    s.assert_row_matches(0, r#"^\[browse\] \[vibrant\] \[mcp: off\] \[cursor L1:C1\] src/big\.ts$"#);
    s.keys("/v42<Enter>");
    // find jumps to the match in browse text
    s.assert_row_matches(0, r#"\[cursor L42:C7\] src/big\.ts$"#);
    s.assert_row_matches(-1, r#"^/v42 \| "#);
    s.keys("/v5<Enter>");
    // Enter jumps to first match row >= cursor, wrapping (v5 at L5 and L50-L59)
    s.assert_row_matches(0, r#"\[cursor L50:C7\] src/big\.ts$"#);
    s.keys("n");
    s.assert_row_matches(0, r#"\[cursor L51:C7\] src/big\.ts$"#);
    s.keys("N");
    s.assert_row_matches(0, r#"\[cursor L50:C7\] src/big\.ts$"#);
    s.keys(":7<Enter>");
    // goto by browse line number
    s.assert_row_matches(0, r#"\[cursor L7:C1\] src/big\.ts$"#);
    s.keys(":99<Enter>");
    s.assert_row_matches(0, r#"\[cursor L7:C1\] src/big\.ts$"#);
    s.assert_row_contains(-1, "line 99 out of range (1-60) | ");
    s.keys("r");
    s.assert_row_matches(0, r#"^\[browse\] .*\[cursor L7:C1\] src/big\.ts$"#);
    s.assert_row_contains(-1, "reloaded | ");
    s.keys("E");
    s.assert_row_contains(-1, "no comments to export | ");
    s.keys("?");
    s.assert_contains("Help · File viewer");
    s.keys("??");
    s.assert_not_contains("Help ·");
    s.keys("C");
    s.assert_contains(" Config");
    s.keys("<Esc>");
    s.assert_not_contains(" Config");
    s.keys("M");
    s.assert_contains("> ○ off");
    s.assert_matches("│ MCP +│");
    s.keys("<Esc>");
    s.assert_not_contains("○ off");
    s.assert_row_matches(0, r#"^\[browse\] .*\[cursor L7:C1\] src/big\.ts$"#);
    s.keys("q");
    s.assert_contains("Quit xplain? (y/n)");
    s.keys("n");
    s.assert_not_contains("Quit xplain?");
    s.keys("FREADME<Enter>");
    // F from browse opens another file in browse
    s.assert_row_matches(0, r#"^\[browse\] \[vibrant\] \[mcp: off\] \[cursor L1:C1\] README\.md$"#);
    s.assert_row_matches(2, "^   1  ▶# Title");
    s.keys("q");
    s.keys("y");
    assert_eq!(s.exit_code(), Some(0));
}

/// F-BROWSE-02: in browse n (no term), f, p, c, s, m, Tab, S-Tab (and [ ]) are ignored; diff state untouched on return
#[test]
fn f_browse_02_ignored() {
    let mut s = Sim::builder().size(120, 24).build();
    s.keys("Fsbt<Enter>5j2l");
    s.assert_row_matches(0, r#"^\[browse\] \[solarized\] \[mcp: off\] \[cursor L6:C3\] src/big\.ts$"#);
    s.assert_row_matches(-1, r#"^\(1-21/60\) hjkl move  enter ask  J/K comments  \? help$"#);
    s.keys("n");
    // n without find term ignored
    s.assert_not_contains(" Files (");
    s.assert_not_contains("too narrow");
    s.assert_not_contains("Search (");
    s.assert_row_matches(0, r#"^\[browse\] \[solarized\] \[mcp: off\] \[cursor L6:C3\] src/big\.ts$"#);
    s.assert_row_matches(7, "^   6  ▶const v6");
    s.assert_row_matches(-1, r#"^\(1-21/60\) hjkl move  enter ask  J/K comments  \? help$"#);
    s.keys("f");
    // f (no picker) ignored
    s.assert_not_contains(" Files (");
    s.assert_not_contains("too narrow");
    s.assert_not_contains("Search (");
    s.assert_row_matches(0, r#"^\[browse\] \[solarized\] \[mcp: off\] \[cursor L6:C3\] src/big\.ts$"#);
    s.assert_row_matches(7, "^   6  ▶const v6");
    s.assert_row_matches(-1, r#"^\(1-21/60\) hjkl move  enter ask  J/K comments  \? help$"#);
    s.keys("p");
    // p (no pane) ignored
    s.assert_not_contains(" Files (");
    s.assert_not_contains("too narrow");
    s.assert_not_contains("Search (");
    s.assert_row_matches(0, r#"^\[browse\] \[solarized\] \[mcp: off\] \[cursor L6:C3\] src/big\.ts$"#);
    s.assert_row_matches(7, "^   6  ▶const v6");
    s.assert_row_matches(-1, r#"^\(1-21/60\) hjkl move  enter ask  J/K comments  \? help$"#);
    s.keys("c");
    // c (no scope toggle) ignored
    s.assert_not_contains(" Files (");
    s.assert_not_contains("too narrow");
    s.assert_not_contains("Search (");
    s.assert_row_matches(0, r#"^\[browse\] \[solarized\] \[mcp: off\] \[cursor L6:C3\] src/big\.ts$"#);
    s.assert_row_matches(7, "^   6  ▶const v6");
    s.assert_row_matches(-1, r#"^\(1-21/60\) hjkl move  enter ask  J/K comments  \? help$"#);
    s.keys("s");
    // s (no split) ignored
    s.assert_not_contains(" Files (");
    s.assert_not_contains("too narrow");
    s.assert_not_contains("Search (");
    s.assert_row_matches(0, r#"^\[browse\] \[solarized\] \[mcp: off\] \[cursor L6:C3\] src/big\.ts$"#);
    s.assert_row_matches(7, "^   6  ▶const v6");
    s.assert_row_matches(-1, r#"^\(1-21/60\) hjkl move  enter ask  J/K comments  \? help$"#);
    s.keys("m");
    // m (no mode cycle) ignored
    s.assert_not_contains(" Files (");
    s.assert_not_contains("too narrow");
    s.assert_not_contains("Search (");
    s.assert_row_matches(0, r#"^\[browse\] \[solarized\] \[mcp: off\] \[cursor L6:C3\] src/big\.ts$"#);
    s.assert_row_matches(7, "^   6  ▶const v6");
    s.assert_row_matches(-1, r#"^\(1-21/60\) hjkl move  enter ask  J/K comments  \? help$"#);
    s.keys("<Tab>");
    // Tab (no file switch) ignored
    s.assert_not_contains(" Files (");
    s.assert_not_contains("too narrow");
    s.assert_not_contains("Search (");
    s.assert_row_matches(0, r#"^\[browse\] \[solarized\] \[mcp: off\] \[cursor L6:C3\] src/big\.ts$"#);
    s.assert_row_matches(7, "^   6  ▶const v6");
    s.assert_row_matches(-1, r#"^\(1-21/60\) hjkl move  enter ask  J/K comments  \? help$"#);
    s.keys("<S-Tab>");
    // S-Tab ignored
    s.assert_not_contains(" Files (");
    s.assert_not_contains("too narrow");
    s.assert_not_contains("Search (");
    s.assert_row_matches(0, r#"^\[browse\] \[solarized\] \[mcp: off\] \[cursor L6:C3\] src/big\.ts$"#);
    s.assert_row_matches(7, "^   6  ▶const v6");
    s.assert_row_matches(-1, r#"^\(1-21/60\) hjkl move  enter ask  J/K comments  \? help$"#);
    s.keys("]");
    // ] (no change starts in browse) ignored
    s.assert_not_contains(" Files (");
    s.assert_not_contains("too narrow");
    s.assert_not_contains("Search (");
    s.assert_row_matches(0, r#"^\[browse\] \[solarized\] \[mcp: off\] \[cursor L6:C3\] src/big\.ts$"#);
    s.assert_row_matches(7, "^   6  ▶const v6");
    s.assert_row_matches(-1, r#"^\(1-21/60\) hjkl move  enter ask  J/K comments  \? help$"#);
    s.keys("[");
    // [ ignored
    s.assert_not_contains(" Files (");
    s.assert_not_contains("too narrow");
    s.assert_not_contains("Search (");
    s.assert_row_matches(0, r#"^\[browse\] \[solarized\] \[mcp: off\] \[cursor L6:C3\] src/big\.ts$"#);
    s.assert_row_matches(7, "^   6  ▶const v6");
    s.assert_row_matches(-1, r#"^\(1-21/60\) hjkl move  enter ask  J/K comments  \? help$"#);
    s.keys("<Esc>");
    // diff state unchanged by the ignored keys (mode, scope, layout, file index)
    s.assert_row_matches(0, r#"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/4\] \[cursor L2:C1\] README\.md \+2 -1$"#);
    s.assert_row_matches(-1, r#"^\(1-5/5\) hjkl move  enter ask  J/K comments  \? help$"#);
}

/// F-BROWSE-02: ) and ( work in browse; numbered note in a diff file switches from browse to that diff file
#[test]
fn f_browse_02_numbered() {
    let mut s = Sim::builder().size(80, 24).config_json(json!({"mcp":{"autostart":true}})).build();
    s.assert_row_contains(0, "[mcp: on]");
    s.keys("Fjson<Enter>");
    s.keys(")");
    // none yet
    s.assert_row_matches(0, r#"^\[browse\] .*package\.json$"#);
    s.assert_row_contains(-1, "no numbered comments | ");
    let r =
        s.mcp_call("annotate", json!({"file": "src/c.tsx", "line": 1, "text": "step one here", "number": 1}));
    assert_eq!(r.tool_result()["ok"], true);
    s.keys("(");
    // leaves browse for diff file src/c.tsx, focuses the note
    s.assert_contains("step one here");
    s.assert_not_contains("[browse]");
    s.assert_matches("▸ sent  #1 agent note L1");
    s.assert_row_matches(0, r#"^\[all\] .*\[3/4\] \[cursor L1:C1\] src/c\.tsx "#);
}

/// F-SEARCH-01: git ls-files failing (not a repo) leaves the search list empty (0/0); Enter does nothing
#[test]
fn f_search_01_list_error() {
    let mut s = Sim::builder()
        .fixture(Fixture::NoGit)
        .size(80, 24)
        .args(["--no-index", "a.txt", "b.txt"])
        .file("a.txt", "same\n")
        .file("b.txt", "same\n")
        .build();
    s.assert_row_matches(0, r#"^\[all\] \[mcp: off\] No changes "#);
    s.keys("F");
    s.assert_row_matches(5, r#"│ Search \(0/0\) +│"#);
    s.assert_row_matches(6, "│ > +│");
    s.assert_row_matches(7, "│ +│");
    s.keys("<Enter>");
    s.assert_row_matches(0, r#"^\[all\] \[mcp: off\] No changes "#);
    s.assert_row_matches(5, r#"│ Search \(0/0\) +│"#);
}

/// F-SEARCH-01: list = tracked + untracked non-ignored files (incl. subdirs, deleted-in-worktree tracked), sorted
#[test]
fn f_search_01_list() {
    let mut s = Sim::builder()
        .size(80, 24)
        .file(".gitignore", "*.log\nbuild/\n")
        .file("debug.log", "x\n")
        .file("build/out.js", "x\n")
        .file("docs/guide.md", "x\n")
        .build();
    s.remove_file("src/c.tsx");
    s.keys("F");
    // ignored debug.log and build/out.js absent; tracked-but-deleted src/c.tsx still listed (--cached)
    s.assert_row_matches(5, r#"│ Search \(1/8\) +│"#);
    s.assert_row_matches(7, r#"│> \.gitignore +│"#);
    s.assert_row_matches(8, r#"│  README\.md +│"#);
    s.assert_row_matches(9, r#"│  docs/guide\.md +│"#);
    s.assert_row_matches(10, r#"│  package\.json +│"#);
    s.assert_row_matches(11, r#"│  src/a\.ts +│"#);
    s.assert_row_matches(12, r#"│  src/big\.ts +│"#);
    s.assert_row_matches(13, r#"│  src/c\.tsx +│"#);
    s.assert_row_matches(14, r#"│  x\.bin +│"#);
    s.assert_row_matches(15, "│ +│");
    assert_region_not_contains(&s, (4, 19), (12, 67), "debug.log");
    assert_region_not_contains(&s, (4, 19), (12, 67), "build/");
    assert_region_not_contains(&s, (4, 19), (12, 67), "out.js");
}

/// F-SEARCH-01: query filters to in-order subsequence hits (gaps allowed); exact path is first hit; matched chars bold accent
#[test]
fn f_search_01_match() {
    let mut s = Sim::builder().size(80, 24).file("big", "b\n").file("src/a.tsx", "x\n").build();
    s.keys("F");
    s.assert_row_matches(5, r#"│ Search \(1/8\) +│"#);
    s.keys("big");
    // hits = {big (exact, first), src/big.ts}
    s.assert_row_matches(5, r#"│ Search \(1/2\) +│"#);
    s.assert_row_matches(6, "│ > big +│");
    s.assert_row_matches(7, "│> big +│");
    s.assert_row_matches(8, r#"│  src/big\.ts +│"#);
    s.assert_row_matches(9, "│ +│");
    assert_region_not_contains(&s, (7, 17), (12, 67), "README.md");
    assert_region_not_contains(&s, (7, 17), (12, 67), "package.json");
    assert_region_not_contains(&s, (7, 17), (12, 67), "src/a.ts");
    assert_region_not_contains(&s, (7, 17), (12, 67), "x.bin");
    // matched chars of unselected row bold + accent; unmatched not bold
    let (x, y) = cell_pos(&s, "src/big.ts", 0, 4, Some(8));
    s.assert_cell(x, y, C::new().ch('b').fg("#cb4b16").bold(true));
    let (x, y) = cell_pos(&s, "src/big.ts", 0, 5, Some(8));
    s.assert_cell(x, y, C::new().ch('i').fg("#cb4b16").bold(true));
    let (x, y) = cell_pos(&s, "src/big.ts", 0, 6, Some(8));
    s.assert_cell(x, y, C::new().ch('g').fg("#cb4b16").bold(true));
    let (x, y) = cell_pos(&s, "src/big.ts", 0, 0, Some(8));
    s.assert_cell(x, y, C::new().ch('s').bold(false));
    let (x, y) = cell_pos(&s, "src/big.ts", 0, 8, Some(8));
    s.assert_cell(x, y, C::new().ch('t').bold(false));
    // selected row keeps selection colors; matched chars still bold
    let (x, y) = cell_pos(&s, "> big", 0, 2, Some(7));
    s.assert_cell(x, y, C::new().ch('b').bg("#073642").bold(true));
    s.keys("<BS><BS><BS>sbt");
    // in-order subsequence s..b..t only in src/big.ts
    s.assert_row_matches(5, r#"│ Search \(1/1\) +│"#);
    s.assert_row_matches(6, "│ > sbt +│");
    s.assert_row_matches(7, r#"│> src/big\.ts +│"#);
    s.assert_row_matches(8, "│ +│");
    s.keys("<BS><BS><BS>tbs");
    // same chars out of order -> no hit
    s.assert_row_matches(5, r#"│ Search \(0/0\) +│"#);
    s.assert_row_matches(6, "│ > tbs +│");
    s.assert_row_matches(7, "│ +│");
    s.keys("<BS><BS><BS>rdm");
    // scattered subsequence r..d..m (lowercase query, case-insensitive) hits README.md only
    s.assert_row_matches(5, r#"│ Search \(1/1\) +│"#);
    s.assert_row_matches(7, r#"│> README\.md +│"#);
    s.assert_row_matches(8, "│ +│");
    s.keys("<BS><BS><BS>pkg");
    s.assert_row_matches(5, r#"│ Search \(1/1\) +│"#);
    s.assert_row_matches(7, r#"│> package\.json +│"#);
    s.assert_row_matches(8, "│ +│");
    s.keys("<BS><BS><BS>ReAdMe");
    // uppercase in query -> case-sensitive; README.md has no lowercase e/d -> no hit
    s.assert_row_matches(5, r#"│ Search \(0/0\) +│"#);
    s.assert_row_matches(7, "│ +│");
    s.keys("<BS><BS><BS><BS><BS><BS>BIG");
    // upper-case query does not hit lower-case paths
    s.assert_row_matches(5, r#"│ Search \(0/0\) +│"#);
    s.assert_row_matches(7, "│ +│");
    s.keys("<BS><BS><BS>");
    s.keys("src/a.ts");
    // exact path src/a.ts first; src/a.tsx also a hit
    s.assert_row_matches(5, r#"│ Search \(1/2\) +│"#);
    s.assert_row_matches(7, r#"│> src/a\.ts +│"#);
    s.assert_row_matches(8, r#"│  src/a\.tsx +│"#);
    s.assert_row_matches(9, "│ +│");
    s.keys("<BS><BS><BS><BS><BS><BS><BS><BS>");
    // empty query again -> all files sorted
    s.assert_row_matches(5, r#"│ Search \(1/8\) +│"#);
    s.assert_row_matches(6, "│ > +│");
    s.assert_row_matches(7, r#"│> README\.md +│"#);
    s.assert_row_matches(8, "│  big +│");
    s.assert_row_matches(9, r#"│  package\.json +│"#);
    s.assert_row_matches(10, r#"│  src/a\.ts +│"#);
    s.assert_row_matches(11, r#"│  src/a\.tsx +│"#);
    s.assert_row_matches(12, r#"│  src/big\.ts +│"#);
    s.assert_row_matches(13, r#"│  src/c\.tsx +│"#);
    s.assert_row_matches(14, r#"│  x\.bin +│"#);
}

/// F-SEARCH-01: F works on the no-changes screen; Enter opens the hit in browse
#[test]
fn f_search_01_nochanges() {
    let mut s = Sim::builder()
        .fixture(Fixture::Empty)
        .size(80, 24)
        .args(["--unstaged"])
        .file("notes.txt", "first\nsecond\n")
        .build();
    s.assert_row_matches(0, r#"^\[unstaged\] \[mcp: off\] No changes \(m cycles mode, F search, q quits\)"#);
    s.assert_row_matches(-1, r#"^\(0-0/0\) "#);
    s.keys("F");
    s.assert_contains("│> notes.txt");
    s.assert_matches(r#"│ Search \(1/1\) +│"#);
    s.keys("<Enter>");
    s.assert_not_contains("Search (");
    s.assert_not_contains("No changes");
    s.assert_row_matches(0, r#"^\[browse\] \[solarized\] \[mcp: off\] \[cursor L1:C1\] notes\.txt$"#);
    s.assert_row_matches(2, "^   1  ▶first[ …]*$");
    s.assert_row_matches(3, "^   2   second *$");
    s.assert_row_matches(-1, r#"^\(1-2/2\) hjkl move  enter ask  J/K comments  \? help$"#);
}

/// F-SEARCH-01: F opens file search over git ls-files (tracked + untracked), sorted; box geometry, title, query row, hint, colors
#[test]
fn f_search_01_open() {
    let mut s = Sim::builder().size(80, 24).build();
    s.assert_not_contains("Search (");
    s.assert_row_matches(0, r#"^\[all\] \[full\] \[unified\] .*\[1/4\] \[cursor L2:C1\] README\.md "#);
    s.keys("F");
    s.assert_row_matches(0, r#"^\[all\] \[full\] \[unified\] .*\[1/4\] \[cursor L2:C1\] README\.md "#);
    s.assert_row_matches(-1, r#"^\(1-5/5\) hjkl move  enter ask  J/K comments  \? help$"#);
    assert_region_not_contains(&s, (3, 3), (12, 67), "╭");
    assert_region_not_contains(&s, (3, 3), (12, 67), "─");
    assert_eq!(
        region(&s, 4, 4, 12, 67),
        "╭──────────────────────────────────────────────────────╮",
        "region rows 4-4 cols 12-67"
    );
    assert_region_matches(&s, (5, 5), (12, 67), r#"^│ Search \(1/6\) +│$"#);
    assert_region_matches(&s, (6, 6), (12, 67), "^│ > +│$");
    assert_region_matches(&s, (7, 7), (12, 67), r#"^│> README\.md +│$"#);
    assert_region_matches(&s, (8, 8), (12, 67), r#"^│  package\.json +│$"#);
    assert_region_matches(&s, (9, 9), (12, 67), r#"^│  src/a\.ts +│$"#);
    assert_region_matches(&s, (10, 10), (12, 67), r#"^│  src/big\.ts +│$"#);
    assert_region_matches(&s, (11, 11), (12, 67), r#"^│  src/c\.tsx +│$"#);
    assert_region_matches(&s, (12, 12), (12, 67), r#"^│  x\.bin +│$"#);
    assert_region_matches(&s, (13, 17), (13, 66), r#"^ *\n *\n *\n *\n *$"#);
    assert_region_matches(&s, (18, 18), (12, 67), r#"^│ ↑↓/\^n\^p move enter open esc close +│$"#);
    assert_eq!(
        region(&s, 19, 19, 12, 67),
        "╰──────────────────────────────────────────────────────╯",
        "region rows 19-19 cols 12-67"
    );
    assert_region_not_contains(&s, (20, 20), (12, 67), "╰");
    assert_region_not_contains(&s, (20, 20), (12, 67), "─");
    // `> ` of the query row in accent, then inverse-space caret
    let (x, y) = cell_pos(&s, "│ > ", 0, 2, Some(6));
    s.assert_cell(x, y, C::new().fg("#cb4b16"));
    let (x, y) = cell_pos(&s, "│ > ", 0, 4, Some(6));
    s.assert_cell(x, y, C::new().ch(' ').reverse(true));
    // selected row selBg/selFg
    let (x, y) = cell_pos(&s, "> README.md", 0, 2, Some(7));
    s.assert_cell(x, y, C::new().fg("#93a1a1").bg("#073642"));
    // unselected row not in selection colors (modal bg)
    let (x, y) = cell_pos(&s, "package.json", 0, 0, Some(8));
    s.assert_cell(x, y, C::new().bg("#002b36"));
}

/// F-SEARCH-01: F does not open search while picker, find, goto or comment editor is open (typed into text inputs)
#[test]
fn f_search_01_pre() {
    let mut s = Sim::builder().size(80, 24).build();
    s.keys("f");
    s.assert_contains(" Files (1/4)");
    s.assert_not_contains("Search (");
    s.keys("F");
    // picker ignores F
    s.assert_contains(" Files (1/4)");
    s.assert_not_contains("Search (");
    s.keys("<Esc>");
    s.assert_not_contains(" Files (");
    s.assert_not_contains("Search (");
    s.keys("/");
    s.assert_row_matches(-1, "^/█");
    s.keys("F");
    s.assert_not_contains("Search (");
    s.assert_row_matches(-1, "^/F█");
    s.keys("<Esc>");
    s.keys(":");
    s.assert_row_matches(-1, "^:█");
    s.keys("F");
    s.assert_not_contains("Search (");
    s.assert_row_matches(-1, "^:F█");
    s.keys("<Esc>");
    s.assert_row_matches(-1, r#"^\(1-5/5\) "#);
    s.keys("a");
    s.assert_contains("enter send");
    s.assert_not_contains("Search (");
    s.keys("F");
    // editor input row shows typed F
    s.assert_not_contains("Search (");
    s.assert_matches("│ F +│");
    s.keys("<Esc>");
    s.assert_not_contains("enter send");
    s.assert_not_contains("Search (");
    s.keys("F");
    // with nothing open, F opens search
    s.assert_contains(" Search (1/6)");
}

/// F-SEARCH-01: search box height floors at 8 on short terminals; list window height-5 = 3 rows centered on selection
#[test]
fn f_search_01_size_min() {
    let mut s = Sim::builder().size(80, 10).build();
    s.keys("F");
    assert_region_matches(&s, (1, 1), (12, 67), "^╭─{54}╮$");
    assert_region_matches(&s, (2, 2), (12, 67), r#"^│ Search \(1/6\) +│$"#);
    assert_region_matches(&s, (3, 3), (12, 67), "^│ > +│$");
    assert_region_matches(&s, (4, 4), (12, 67), r#"^│> README\.md +│$"#);
    assert_region_matches(&s, (5, 5), (12, 67), r#"^│  package\.json +│$"#);
    assert_region_matches(&s, (6, 6), (12, 67), r#"^│  src/a\.ts +│$"#);
    assert_region_matches(&s, (7, 7), (12, 67), r#"^│ ↑↓/\^n\^p move enter open esc close +│$"#);
    assert_region_matches(&s, (8, 8), (12, 67), "^╰─{54}╯$");
    s.keys("<Down><Down><Down>");
    // selection 4th of 6 -> window centered on it
    assert_region_matches(&s, (2, 2), (12, 67), r#"^│ Search \(4/6\) +│$"#);
    assert_region_matches(&s, (4, 4), (12, 67), r#"^│  src/a\.ts +│$"#);
    assert_region_matches(&s, (5, 5), (12, 67), r#"^│> src/big\.ts +│$"#);
    assert_region_matches(&s, (6, 6), (12, 67), r#"^│  src/c\.tsx +│$"#);
    s.keys("<Up><Up>");
    assert_region_matches(&s, (2, 2), (12, 67), r#"^│ Search \(2/6\) +│$"#);
    assert_region_matches(&s, (4, 4), (12, 67), r#"^│  README\.md +│$"#);
    assert_region_matches(&s, (5, 5), (12, 67), r#"^│> package\.json +│$"#);
    assert_region_matches(&s, (6, 6), (12, 67), r#"^│  src/a\.ts +│$"#);
    s.keys("<Down><Down><Down><Down>");
    // last hit selected and visible
    assert_region_matches(&s, (2, 2), (12, 67), r#"^│ Search \(6/6\) +│$"#);
    assert_region_contains(&s, (4, 6), (12, 67), "│> x.bin");
}

/// F-SEARCH-01: search box at 120x40 is floor(cols*0.7)=84 wide and floor(R*0.7)=28 high, centered; empty rows below short list
#[test]
fn f_search_01_size_wide() {
    let mut s = Sim::builder().size(120, 40).build();
    s.assert_not_contains("Search (");
    s.keys("F");
    assert_region_not_contains(&s, (5, 5), (18, 101), "╭");
    assert_region_not_contains(&s, (5, 5), (18, 101), "─");
    assert_region_matches(&s, (6, 6), (18, 101), "^╭─{82}╮$");
    assert_region_matches(&s, (7, 7), (18, 101), r#"^│ Search \(1/6\) +│$"#);
    assert_region_matches(&s, (8, 8), (18, 101), "^│ > +│$");
    assert_region_matches(&s, (9, 9), (18, 101), r#"^│> README\.md +│$"#);
    assert_region_matches(&s, (14, 14), (18, 101), r#"^│  x\.bin +│$"#);
    assert_region_matches(&s, (32, 32), (18, 101), r#"^│ ↑↓/\^n\^p move enter open esc close +│$"#);
    assert_region_matches(&s, (33, 33), (18, 101), "^╰─{82}╯$");
    assert_region_not_contains(&s, (34, 34), (18, 101), "╰");
    assert_region_not_contains(&s, (34, 34), (18, 101), "─");
}

/// F-SEARCH-01: smart case, every query char literal (space ' ^ $ ! |), exact path first under the same case rule
#[test]
fn f_search_01_smart_case() {
    let mut s = Sim::builder()
        .size(80, 24)
        .file("notes/readme.md", "x\n")
        .file("doc/a b.md", "x\n")
        .file("odd/x^y$z!w|v'u.txt", "x\n")
        .build();
    s.keys("F");
    s.assert_row_matches(5, r#"│ Search \(1/9\) +│"#);
    s.keys("rm");
    // all-lowercase query -> case-insensitive; README.md and notes/readme.md
    s.assert_row_matches(5, r#"│ Search \(1/2\) +│"#);
    s.assert_row_matches(9, "│ +│");
    assert_region_matches(&s, (7, 8), (12, 67), r#"│[> ] README\.md +│"#);
    assert_region_matches(&s, (7, 8), (12, 67), r#"│[> ] notes/readme\.md +│"#);
    s.keys("<BS><BS>RM");
    // uppercase in query -> case-sensitive; README.md only
    s.assert_row_matches(5, r#"│ Search \(1/1\) +│"#);
    s.assert_row_matches(7, r#"│> README\.md +│"#);
    s.assert_row_matches(8, "│ +│");
    s.keys("<BS><BS>rM");
    // r then M, case-sensitive -> neither
    s.assert_row_matches(5, r#"│ Search \(0/0\) +│"#);
    s.assert_row_matches(7, "│ +│");
    s.keys("<BS><BS>readme.md");
    // exact path under the case-insensitive rule (README.md) is the first hit
    s.assert_row_matches(5, r#"│ Search \(1/2\) +│"#);
    s.assert_row_matches(7, r#"│> README\.md +│"#);
    s.assert_row_matches(8, r#"│  notes/readme\.md +│"#);
    s.assert_row_matches(9, "│ +│");
    s.keys("<BS><BS><BS><BS><BS><BS><BS><BS><BS>notes/readme.md");
    // lowercase exact path first too
    s.assert_row_matches(5, r#"│ Search \(1/1\) +│"#);
    s.assert_row_matches(7, r#"│> notes/readme\.md +│"#);
    s.keys("<BS><BS><BS><BS><BS><BS><BS><BS><BS><BS><BS><BS><BS><BS><BS>README.md");
    // case-sensitive query equal to README.md -> only that path
    s.assert_row_matches(5, r#"│ Search \(1/1\) +│"#);
    s.assert_row_matches(7, r#"│> README\.md +│"#);
    s.assert_row_matches(8, "│ +│");
    s.keys("<BS><BS><BS><BS><BS><BS><BS><BS><BS>a b");
    // space is literal (no AND of terms) -> only the path with a, space, b in order
    s.assert_row_matches(5, r#"│ Search \(1/1\) +│"#);
    s.assert_row_matches(6, "│ > a b +│");
    s.assert_row_matches(7, r#"│> doc/a b\.md +│"#);
    s.assert_row_matches(8, "│ +│");
    s.keys("<BS><BS><BS>^s");
    // ^ is literal (no prefix anchor), no path has ^ followed by s
    s.assert_row_matches(5, r#"│ Search \(0/0\) +│"#);
    s.assert_row_matches(7, "│ +│");
    s.keys("<BS><BS>^y$");
    // ^ and $ literal -> only odd/x^y$z!w|v'u.txt
    s.assert_row_matches(5, r#"│ Search \(1/1\) +│"#);
    s.assert_row_matches(7, r#"│> odd/x\^y\$z!w\|v'u\.txt +│"#);
    s.assert_row_matches(8, "│ +│");
    s.keys("<BS><BS><BS>.md$");
    // $ literal (no suffix anchor) -> no hit
    s.assert_row_matches(5, r#"│ Search \(0/0\) +│"#);
    s.assert_row_matches(7, "│ +│");
    s.keys("<BS><BS><BS><BS>!y");
    // literal (no negation) -> no path has ! followed by y
    s.assert_row_matches(5, r#"│ Search \(0/0\) +│"#);
    s.keys("<BS><BS>'big");
    // ' literal (no exact-match prefix) -> no hit
    s.assert_row_matches(5, r#"│ Search \(0/0\) +│"#);
    s.keys("<BS><BS><BS><BS>w|v");
    // | literal (no OR) -> only the odd path
    s.assert_row_matches(5, r#"│ Search \(1/1\) +│"#);
    s.assert_row_matches(7, "│> odd/");
}

/// F-SEARCH-01: list is unique; conflicted path (3 index stages, listed thrice by git ls-files) shows once
#[test]
fn f_search_01_unique() {
    let mut s = Sim::builder().fixture(Fixture::Empty).size(80, 24).build();
    s.write_file("f.txt", "a\n");
    s.git(&["add", "f.txt"]);
    s.git(&["commit", "-qm", "a"]);
    s.git(&["checkout", "-qb", "other"]);
    s.write_file("f.txt", "b\n");
    s.git(&["commit", "-qam", "b"]);
    s.git(&["checkout", "-q", "main"]);
    s.write_file("f.txt", "c\n");
    s.git(&["commit", "-qam", "c"]);
    assert_eq!(s.git_code(&["merge", "-q", "other"]), 1);
    s.keys("F");
    s.assert_row_matches(5, r#"│ Search \(1/1\) +│"#);
    s.assert_row_matches(7, r#"│> f\.txt +│"#);
    s.assert_row_matches(8, "│ +│");
}

/// F-SEARCH-02: Backspace/Delete drop last query char and reset selection; Left/Right do nothing
#[test]
fn f_search_02_edit() {
    let mut s = Sim::builder().size(80, 24).build();
    s.keys("Fr<Down><Down>");
    s.assert_row_matches(5, r#"│ Search \(3/4\) +│"#);
    s.assert_row_matches(6, "│ > r +│");
    s.keys("<Left>");
    // Left changes neither query nor selection
    s.assert_row_matches(5, r#"│ Search \(3/4\) +│"#);
    s.assert_row_matches(6, "│ > r +│");
    s.keys("<Right>");
    s.assert_row_matches(5, r#"│ Search \(3/4\) +│"#);
    s.assert_row_matches(6, "│ > r +│");
    s.keys("<BS>");
    // query empty -> all 6 files, selection first
    s.assert_row_matches(5, r#"│ Search \(1/6\) +│"#);
    s.assert_row_matches(6, "│ > +│");
    s.assert_row_matches(7, r#"│> README\.md +│"#);
    s.keys("<BS>");
    // Backspace on empty query is harmless
    s.assert_row_matches(5, r#"│ Search \(1/6\) +│"#);
    s.assert_row_matches(6, "│ > +│");
    s.keys("r.<Down>");
    s.assert_row_matches(6, r#"│ > r\. +│"#);
    s.keys("<Del>");
    // Delete also removes the last char (not char under caret) and resets selection
    s.assert_row_matches(5, r#"│ Search \(1/4\) +│"#);
    s.assert_row_matches(6, "│ > r +│");
}

/// F-SEARCH-02: Enter opens selected hit in browse and closes; Enter with no hits does nothing; Esc closes without change
#[test]
fn f_search_02_enter_esc() {
    let mut s = Sim::builder().size(80, 24).build();
    s.keys("Fzzz");
    s.assert_row_matches(5, r#"│ Search \(0/0\) +│"#);
    s.keys("<Enter>");
    // no hits -> search stays open, still diff view
    s.assert_row_matches(0, r#"^\[all\] .*\[1/4\] \[cursor L2:C1\] README\.md "#);
    s.assert_row_matches(5, r#"│ Search \(0/0\) +│"#);
    s.assert_row_matches(6, "│ > zzz +│");
    s.keys("<Esc>");
    // Esc closes; diff view unchanged
    s.assert_not_contains("Search (");
    s.assert_row_matches(0, r#"^\[all\] \[full\] \[unified\] .*\[1/4\] \[cursor L2:C1\] README\.md "#);
    s.assert_row_matches(4, "^   2      -▶hello");
    s.assert_row_matches(-1, r#"^\(1-5/5\) hjkl move  enter ask  J/K comments  \? help$"#);
    s.keys("F");
    // reopened search starts with empty query
    s.assert_row_matches(5, r#"│ Search \(1/6\) +│"#);
    s.assert_row_matches(6, "│ > +│");
    s.keys("<Down><Down><Down>");
    s.assert_row_matches(10, r#"│> src/big\.ts +│"#);
    s.keys("<Enter>");
    // selected hit (not first) opened in browse, search closed
    s.assert_not_contains("Search (");
    s.assert_row_matches(0, r#"^\[browse\] \[solarized\] \[mcp: off\] \[cursor L1:C1\] src/big\.ts$"#);
    s.assert_row_matches(2, "^   1  ▶const v1 = 1;");
    s.assert_row_matches(-1, r#"^\(1-21/60\) hjkl move  enter ask  J/K comments  \? help$"#);
}

/// F-SEARCH-02: Down/Ctrl+N next hit, Up/Ctrl+P previous, clamped at both ends
#[test]
fn f_search_02_move() {
    let mut s = Sim::builder().size(80, 24).build();
    s.keys("F<Up>");
    // Up on first hit stays
    s.assert_row_matches(5, r#"│ Search \(1/6\) +│"#);
    s.assert_row_matches(7, r#"│> README\.md +│"#);
    s.keys("<C-p>");
    s.assert_row_matches(5, r#"│ Search \(1/6\) +│"#);
    s.keys("<C-n>");
    s.assert_row_matches(5, r#"│ Search \(2/6\) +│"#);
    s.assert_row_matches(7, r#"│  README\.md +│"#);
    s.assert_row_matches(8, r#"│> package\.json +│"#);
    s.keys("<C-n><Down>");
    s.assert_row_matches(5, r#"│ Search \(4/6\) +│"#);
    s.assert_row_matches(10, r#"│> src/big\.ts +│"#);
    s.keys("<C-p>");
    s.assert_row_matches(5, r#"│ Search \(3/6\) +│"#);
    s.assert_row_matches(9, r#"│> src/a\.ts +│"#);
    s.assert_row_matches(10, r#"│  src/big\.ts +│"#);
    s.keys("<Up>");
    s.assert_row_matches(5, r#"│ Search \(2/6\) +│"#);
    s.assert_row_matches(8, r#"│> package\.json +│"#);
    s.keys("<Down><Down><Down><Down><Down><Down>");
    // clamped at last hit
    s.assert_row_matches(5, r#"│ Search \(6/6\) +│"#);
    s.assert_row_matches(12, r#"│> x\.bin +│"#);
    s.keys("<C-n>");
    s.assert_row_matches(5, r#"│ Search \(6/6\) +│"#);
    s.assert_row_matches(12, r#"│> x\.bin +│"#);
    // query untouched by moves
    s.assert_row_matches(6, "│ > +│");
}

/// F-SEARCH-02: printable chars incl. j k q ? space go to the query (not bound keys); typing resets selection to first hit
#[test]
fn f_search_02_type() {
    let mut s = Sim::builder().size(80, 24).build();
    s.keys("F<Down><Down>");
    s.assert_row_matches(5, r#"│ Search \(3/6\) +│"#);
    s.assert_row_matches(9, r#"│> src/a\.ts +│"#);
    s.keys("s");
    // hits containing s = package.json + 3 src files; selection back to first
    s.assert_row_matches(5, r#"│ Search \(1/4\) +│"#);
    s.assert_row_matches(6, "│ > s +│");
    s.assert_row_matches(7, "│> ");
    s.assert_row_matches(11, "│ +│");
    s.keys("<BS>");
    s.keys("j");
    // j typed, not a move
    s.assert_row_matches(5, r#"│ Search \(1/1\) +│"#);
    s.assert_row_matches(6, "│ > j +│");
    s.assert_row_matches(7, r#"│> package\.json +│"#);
    s.keys("k");
    s.assert_row_matches(5, r#"│ Search \(0/0\) +│"#);
    s.assert_row_matches(6, "│ > jk +│");
    s.assert_row_matches(7, "│ +│");
    s.keys("q");
    // q typed, no quit confirm
    s.assert_not_contains("Quit xplain?");
    s.assert_row_matches(6, "│ > jkq +│");
    s.keys("?");
    // ? typed, no help panel
    s.assert_not_contains("Help ·");
    s.assert_row_matches(6, r#"│ > jkq\? +│"#);
    s.keys("<Space>x");
    // space typed (hits for queries with spaces UNSPEC-30, title not asserted)
    s.assert_row_matches(6, r#"│ > jkq\? x +│"#);
    s.keys("F");
    // F typed too (search stays open)
    s.assert_row_matches(6, r#"│ > jkq\? xF +│"#);
}
