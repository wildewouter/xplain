//! Ported from `e2e/scenarios/u11-comment` (F-COMMENT-06..10, F-EXPORT-01/02).

#![allow(clippy::expect_used, clippy::panic)]

use regex::{Regex, RegexBuilder};
use serde_json::{Value, json};
use xplain_sim::{CellExpect as C, Http, Sim};

// ---- helpers --------------------------------------------------------------------------------------------------

fn rx(p: &str) -> Regex {
    RegexBuilder::new(p).multi_line(true).build().unwrap_or_else(|e| panic!("bad regex {p:?}: {e}"))
}

#[track_caller]
fn assert_re(text: &str, p: &str) {
    assert!(rx(p).is_match(text), "text does not match /{p}/\n{text}");
}

#[track_caller]
fn assert_has(text: &str, needle: &str) {
    assert!(text.contains(needle), "text does not contain {needle:?}\n{text}");
}

#[track_caller]
fn assert_lacks(text: &str, needle: &str) {
    assert!(!text.contains(needle), "text unexpectedly contains {needle:?}\n{text}");
}

fn mcp_sim() -> Sim {
    Sim::builder().config_json(json!({"mcp": {"autostart": true}})).build()
}

fn mcp_sim_size(w: u16, h: u16) -> Sim {
    Sim::builder().config_json(json!({"mcp": {"autostart": true}})).size(w, h).build()
}

#[track_caller]
fn annotate(s: &mut Sim, args: Value) {
    let r = s.mcp_call("annotate", args);
    assert_eq!(r.status, 200);
    assert_eq!(r.tool_result()["ok"], true);
}

/// Screen after `keys`: no focused comment and the cursor tag in row 0.
#[track_caller]
fn unfocused_at(s: &mut Sim, keys: &str, cursor: &str) {
    s.keys(keys);
    s.assert_not_contains("▸");
    s.assert_row_contains(0, cursor);
}

/// Names of the review exports in `dir`.
fn exports(s: &Sim, dir: &str) -> Vec<String> {
    s.list_dir(dir).into_iter().filter(|n| n.starts_with("xplain-review-") && n.ends_with(".md")).collect()
}

/// The one export in `dir` as (relative path, content).
#[track_caller]
fn only_export(s: &Sim, dir: &str) -> (String, String) {
    let v = exports(s, dir);
    assert_eq!(v.len(), 1, "exports in {dir}: {v:?}");
    let rel = if dir == "." { v[0].clone() } else { format!("{dir}/{}", v[0]) };
    let text = s.file(&rel);
    (rel, text)
}

fn repo_re(s: &Sim) -> String {
    regex::escape(&s.repo().to_string_lossy())
}

// ---- F-COMMENT-06 ---------------------------------------------------------------------------------------------

/// digits while focused start a count and keep focus; the next motion unfocuses and uses the count.
#[test]
fn f_comment_06_count() {
    let mut s = Sim::builder().build();
    s.keys("<Tab>");
    s.keys(":20<Enter>ac<Enter>");
    s.keys("J");
    s.assert_contains("▸ sent  line L20");
    s.keys("3");
    s.assert_contains("▸ sent  line L20");
    s.assert_row_contains(0, "[cursor L20:C1] src/big.ts");
    s.assert_row_contains(-1, "e edit  D delete");
    s.keys("j");
    s.assert_not_contains("▸");
    s.assert_row_contains(0, "[cursor L23:C1] src/big.ts");
}

/// motion keys unfocus the comment and then act (move cursor / start selection).
#[test]
fn f_comment_06_motion_unfocus() {
    let mut s = Sim::builder().build();
    s.keys("<Tab>");
    s.keys(":20<Enter>ac<Enter>");
    s.keys("J");
    s.assert_contains("▸ sent  line L20");
    s.assert_row_contains(0, "[cursor L20:C1] src/big.ts");
    let cases = [
        ("j", "[cursor L21:C1] src/big.ts"),
        ("Jk", "[cursor L19:C1] src/big.ts"),
        ("J<Down>", "[cursor L21:C1] src/big.ts"),
        ("J<Up>", "[cursor L19:C1] src/big.ts"),
        ("Jl", "[cursor L20:C2] src/big.ts"),
        ("J<Right>", "[cursor L20:C3] src/big.ts"),
        ("Jh", "[cursor L20:C2] src/big.ts"),
        ("J<Left>", "[cursor L20:C1] src/big.ts"),
        ("Jw", "[cursor L20:C7] src/big.ts"),
        ("Jb", "[cursor L20:C1] src/big.ts"),
        ("J$", "[cursor L20:C14] src/big.ts"),
        ("J0", "[cursor L20:C1] src/big.ts"),
        ("lllJ^", "[cursor L20:C1] src/big.ts"),
        ("Jv", "[visual L20:C1] src/big.ts"),
        ("<Esc>JV", "[visual L20:C1] src/big.ts"),
        // next change start after L20 is the del row of L30
        ("<Esc>J]", "[cursor L30:C1] src/big.ts"),
        // no change start before L20 - no move, but unfocused
        ("J[", "[cursor L20:C1] src/big.ts"),
        // H=37, d = 18 rows (row 38 = L37, L30 has del+add rows)
        ("Jd", "[cursor L37:C1] src/big.ts"),
        ("Ju", "[cursor L2:C1] src/big.ts"),
        ("JG", "[cursor L60:C1] src/big.ts"),
        // row 1 is the hunk row
        ("Jg", "[cursor r1:C1] src/big.ts"),
        // page = H-1 = 36 rows from row 20 (row 56 = L55)
        ("J<Space>", "[cursor L55:C1] src/big.ts"),
        ("J<PageUp>", "[cursor r1:C1] src/big.ts"),
        ("J<PageDown>", "[cursor L55:C1] src/big.ts"),
        // unified - p changes no pane, but still unfocuses
        ("Jp", "[cursor L20:C1] src/big.ts"),
    ];
    for (keys, cursor) in cases {
        unfocused_at(&mut s, keys, cursor);
    }
}

/// J/K with no comments in the shown file: note no comments, cursor unchanged (comments of other files ignored).
#[test]
fn f_comment_06_no_comments() {
    let mut s = Sim::builder().size(120, 20).build();
    s.assert_row_contains(0, "[cursor L2:C1] README.md");
    s.keys("J");
    s.assert_not_contains("▸");
    s.assert_row_contains(0, "[cursor L2:C1] README.md");
    s.assert_row_matches(-1, r"^no comments \| ");
    s.keys("jac<Enter>");
    s.assert_row_matches(-1, r"^question saved \(1\) \| ");
    s.keys("<Tab>");
    s.assert_row_contains(0, "[cursor L30:C1] src/big.ts");
    s.keys("K");
    s.assert_not_contains("▸");
    s.assert_row_contains(0, "[cursor L30:C1] src/big.ts");
    s.assert_row_matches(-1, r"^no comments \| ");
    // back on README the comment is found
    s.keys("<S-Tab>J");
    s.assert_contains("▸ sent  line L2");
    s.assert_row_contains(-1, "e edit  D delete");
}

/// J/K walk file comments by row then creation; unfocused J = first, K = last; clamped, no wrap; cursor follows.
#[test]
fn f_comment_06_order() {
    let mut s = Sim::builder().build();
    s.keys("<Tab>");
    s.keys(":40<Enter>ac1<Enter>");
    s.keys(":10<Enter>ac2<Enter>");
    s.keys(":40<Enter>ac3<Enter>");
    s.keys(":20<Enter>ac4<Enter>");
    s.assert_not_contains("▸");
    s.assert_row_contains(0, "[cursor L20:C1] src/big.ts");
    // unfocused J = first by row (L10), not first created (L40)
    s.keys("J");
    s.assert_matches("▸ sent  line L10[^\n]*\n┃ c2 ");
    s.assert_row_contains(0, "[cursor L10:C1] src/big.ts");
    s.assert_row_contains(-1, "e edit  D delete");
    s.keys("J");
    s.assert_matches("▸ sent  line L20[^\n]*\n┃ c4 ");
    s.assert_row_contains(0, "[cursor L20:C1] src/big.ts");
    // two comments on L40, creation order - c1 first
    s.keys("J");
    s.assert_matches("▸ sent  line L40[^\n]*\n┃ c1 ");
    s.assert_row_contains(0, "[cursor L40:C1] src/big.ts");
    s.keys("J");
    s.assert_matches("▸ sent  line L40[^\n]*\n┃ c3 ");
    s.assert_row_contains(0, "[cursor L40:C1] src/big.ts");
    // clamped at last, no wrap to c2
    s.keys("J");
    s.assert_matches("▸ sent  line L40[^\n]*\n┃ c3 ");
    s.assert_not_contains("▸ sent  line L10");
    s.assert_row_contains(0, "[cursor L40:C1] src/big.ts");
    s.keys("K");
    s.assert_matches("▸ sent  line L40[^\n]*\n┃ c1 ");
    s.keys("K");
    s.assert_matches("▸ sent  line L20[^\n]*\n┃ c4 ");
    s.assert_row_contains(0, "[cursor L20:C1] src/big.ts");
    // clamped at first, no wrap to c3
    s.keys("KK");
    s.assert_matches("▸ sent  line L10[^\n]*\n┃ c2 ");
    s.assert_row_contains(0, "[cursor L10:C1] src/big.ts");
    s.keys("<Esc>");
    s.assert_not_contains("▸");
    // unfocused K = last (c3 on L40)
    s.keys("K");
    s.assert_matches("▸ sent  line L40[^\n]*\n┃ c3 ");
    s.assert_row_contains(0, "[cursor L40:C1] src/big.ts");
}

/// J/K end an active selection and move the cursor to the comment row.
#[test]
fn f_comment_06_selection_ends() {
    let mut s = Sim::builder().build();
    s.keys("ahi<Enter>");
    s.keys("vj");
    s.assert_not_contains("▸");
    s.assert_row_contains(0, "[visual L2:C1] README.md");
    s.assert_row_contains(-1, "v/esc end  enter ask");
    // comment on del row L2; selection ended, cursor back on the del row
    s.keys("K");
    s.assert_contains("▸ sent  line L2");
    s.assert_not_contains("[visual");
    s.assert_row_contains(0, "[cursor L2:C1] README.md");
    s.assert_row_matches(4, r"^   2      -▶hello");
    s.assert_row_contains(-1, "e edit  D delete");
    s.keys("<Esc>G");
    s.assert_row_contains(0, "[cursor L3:C1] README.md");
    s.keys("V");
    s.assert_row_contains(0, "[visual L3:C1] README.md");
    s.keys("J");
    s.assert_contains("▸ sent  line L2");
    s.assert_not_contains("[visual");
    s.assert_row_contains(0, "[cursor L2:C1] README.md");
    s.assert_row_matches(4, r"^   2      -▶hello");
}

// ---- F-COMMENT-07 ---------------------------------------------------------------------------------------------

/// agent note with a single turn is editable: e prefills the note text, Enter changes it.
#[test]
fn f_comment_07_agent_note() {
    let mut s = mcp_sim_size(120, 30);
    s.assert_row_contains(0, "[mcp: on]");
    annotate(&mut s, json!({"file": "README.md", "line": 1, "text": "agent said"}));
    s.assert_matches("sent  agent note L1[^\n]*\n│ agent said ");
    s.keys("J");
    s.assert_contains("▸ sent  agent note L1");
    s.keys("e");
    s.assert_matches("│ edit agent note L1 +│\n│ agent said  +│\n│ enter send  esc cancel +│");
    s.keys(" twice<Enter>");
    s.assert_matches("sent  agent note L1[^\n]*\n[│┃] agent said twice ");
    s.assert_row_matches(-1, r"^comment updated \| ");
}

/// comment with an answer (single turn) is editable; the answer is kept.
#[test]
fn f_comment_07_answered() {
    let mut s = mcp_sim_size(120, 30);
    s.assert_row_contains(0, "[mcp: on]");
    s.keys("awhy this<Enter>");
    let q = s.mcp_call("next_question", json!({"wait_seconds": 1})).tool_result();
    assert_eq!(q["status"], "question");
    assert_eq!(q["turn"], 1);
    let t = q["thread_id"].as_str().unwrap_or_default().to_string();
    let r = s.mcp_call("answer", json!({"thread_id": t, "text": "because reasons"}));
    assert_eq!(r.tool_result()["ok"], true);
    s.assert_contains("because reasons");
    s.keys("J");
    s.keys("e");
    s.assert_matches("│ edit line L2 +│\n│ why this  +│");
    s.keys("<BS><BS><BS><BS>that<Enter>");
    s.assert_matches(
        "sent  line L2[^\n]*\n[│┃] why that +[│┃]\n[│┃]─ answer · [^\n]*done[^\n]*\n[│┃] because reasons ",
    );
    s.assert_not_contains("why this");
    s.assert_row_matches(-1, r"^comment updated \| ");
}

/// e on focused comment opens editor prefilled (caret at end, head edit <head>); Enter replaces message.
#[test]
fn f_comment_07_edit_e() {
    let mut s = Sim::builder().size(120, 30).build();
    s.keys("<Tab>");
    s.keys(":20<Enter>afirst text<Enter>");
    s.keys("J");
    s.assert_matches("▸ sent  line L20[^\n]*\n┃ first text ");
    s.keys("e");
    s.assert_matches("│ edit line L20 +│\n│ first text  +│\n│ enter send  esc cancel +│");
    s.assert_row_contains(-1, "enter send  esc cancel");
    // typed text lands at the caret, which started at the end
    s.keys(" more");
    s.assert_matches("│ first text more  +│");
    s.keys("<Enter>");
    s.assert_matches("sent  line L20[^\n]*\n[│┃] first text more ");
    s.assert_not_contains("edit line L20");
    s.assert_not_contains("enter send");
    s.assert_row_matches(-1, r"^comment updated \| ");
}

/// Enter on focused comment also opens the edit editor; selection comment edit shows its quoted lines.
#[test]
fn f_comment_07_edit_enter() {
    let mut s = Sim::builder().size(120, 30).build();
    s.keys("Vj<Enter>");
    s.keys("sel note<Enter>");
    s.assert_contains("sent  selection L2 ");
    s.keys("J");
    s.assert_contains("▸ sent  selection L2 ");
    s.keys("<Enter>");
    s.assert_matches(
        "│ edit selection L2 +│\n│ > hello +│\n│ > hello world +│\n│ sel note  +│\n│ enter send  esc cancel +│",
    );
    s.keys("<BS><BS><BS><BS>text<Enter>");
    s.assert_matches(
        "sent  selection L2[^\n]*\n[│┃] > hello +[│┃]\n[│┃] > hello world +[│┃]\n[│┃] sel text ",
    );
    s.assert_not_contains("sel note");
    s.assert_not_contains("edit selection");
    s.assert_row_matches(-1, r"^comment updated \| ");
}

/// Esc in the edit editor discards the change; message kept.
#[test]
fn f_comment_07_esc() {
    let mut s = Sim::builder().size(120, 30).build();
    s.keys("aoriginal<Enter>");
    s.keys("Je");
    s.assert_contains("edit line L2");
    s.keys(" changed");
    s.assert_contains("original changed");
    s.keys("<Esc>");
    s.assert_matches("sent  line L2[^\n]*\n[│┃] original ");
    s.assert_not_contains("original changed");
    s.assert_not_contains("edit line L2");
    s.assert_not_contains("comment updated");
}

/// thread with follow-ups: e and Enter note can't edit after follow-ups, no editor.
#[test]
fn f_comment_07_follow_ups() {
    let mut s = mcp_sim_size(120, 30);
    s.assert_row_contains(0, "[mcp: on]");
    s.keys("afirst<Enter>");
    let q = s.mcp_call("next_question", json!({"wait_seconds": 1})).tool_result();
    let t = q["thread_id"].as_str().unwrap_or_default().to_string();
    let r = s.mcp_call("answer", json!({"thread_id": t, "text": "an answer"}));
    assert_eq!(r.tool_result()["ok"], true);
    s.keys("Ja");
    s.keys("again<Enter>");
    s.assert_contains("follow-up: again");
    s.assert_row_matches(-1, r"^follow-up queued \| ");
    s.keys("J");
    s.assert_contains("▸ sent  line L2");
    s.keys("e");
    s.assert_contains("▸ sent  line L2");
    s.assert_not_contains("edit line L2");
    s.assert_not_contains("enter send");
    s.assert_row_matches(-1, r"^can't edit after follow-ups \| ");
    s.keys("<Esc>J<Enter>");
    s.assert_contains("▸ sent  line L2");
    s.assert_not_contains("edit line L2");
    s.assert_not_contains("enter send");
    s.assert_row_matches(-1, r"^can't edit after follow-ups \| ");
}

// ---- F-COMMENT-08 ---------------------------------------------------------------------------------------------

/// delete modal: n and Esc close keeping the comment; other keys ignored; ? toggles help.
#[test]
fn f_comment_08_cancel() {
    let mut s = Sim::builder().build();
    s.keys("akeep me<Enter>J");
    s.assert_contains("▸ sent  line L2");
    s.keys("D");
    s.assert_contains("Delete comment? (y/n)");
    s.keys("n");
    s.assert_not_contains("Delete comment?");
    s.assert_matches("sent  line L2[^\n]*\n[│┃] keep me ");
    s.keys("D");
    s.assert_contains("Delete comment? (y/n)");
    s.keys("<Esc>");
    s.assert_not_contains("Delete comment?");
    s.assert_matches("sent  line L2[^\n]*\n[│┃] keep me ");
    s.keys("D");
    // keys ignored - modal still open, no editor, cursor not moved
    s.keys("jxJKe");
    s.assert_contains("Delete comment? (y/n)");
    s.assert_not_contains("edit line");
    s.assert_row_contains(0, "[cursor L2:C1] README.md");
    s.keys("?");
    s.assert_contains("Delete comment? (y/n)");
    s.assert_contains("Help · Confirm");
    s.keys("?");
    s.assert_contains("Delete comment? (y/n)");
    s.assert_not_contains("Help · Confirm");
    s.keys("n");
    s.assert_not_contains("Delete comment?");
    s.assert_matches("sent  line L2[^\n]*\n[│┃] keep me ");
}

/// Enter confirms delete too; deleting the last comment in file unfocuses, cursor stays.
#[test]
fn f_comment_08_delete_enter_last() {
    let mut s = Sim::builder().build();
    s.keys("<Tab>");
    s.keys(":10<Enter>ac1<Enter>");
    s.keys(":20<Enter>ac2<Enter>");
    s.keys("K");
    s.assert_matches("▸ sent  line L20[^\n]*\n┃ c2 ");
    s.keys("D");
    s.assert_contains("Delete comment? (y/n)");
    s.keys("<Enter>");
    // no comment after c2 - unfocused, cursor stays on L20
    s.assert_not_contains("Delete comment?");
    s.assert_not_contains("line L20");
    s.assert_not_contains("▸");
    s.assert_contains("sent  line L10");
    s.assert_row_contains(0, "[cursor L20:C1] src/big.ts");
    s.assert_row_matches(-1, r"^comment deleted \| \(\d+-\d+/62\) hjkl move");
    s.keys("JD<Enter>");
    s.assert_not_contains("sent  line");
    s.assert_not_contains("▸");
    s.assert_row_contains(0, "[cursor L10:C1] src/big.ts");
    s.assert_row_matches(-1, r"^comment deleted \| ");
    s.keys("J");
    s.assert_row_matches(-1, r"^no comments \| ");
}

/// D opens Delete comment? (y/n); y removes it, focus moves to next comment in file.
#[test]
fn f_comment_08_delete_y() {
    let mut s = Sim::builder().build();
    s.keys("<Tab>");
    s.keys(":10<Enter>ac1<Enter>");
    s.keys(":20<Enter>ac2<Enter>");
    s.keys(":25<Enter>ac3<Enter>");
    s.keys("JJ");
    s.assert_matches("▸ sent  line L20[^\n]*\n┃ c2 ");
    s.keys("D");
    // round border, paddingX 1 (21 chars + 2 padding)
    s.assert_matches(r"│ Delete comment\? \(y/n\) │");
    s.assert_matches("╭─{23}╮");
    s.assert_matches("╰─{23}╯");
    s.keys("y");
    s.assert_not_contains("Delete comment?");
    s.assert_not_contains("line L20");
    s.assert_matches("▸ sent  line L25[^\n]*\n┃ c3 ");
    s.assert_matches("sent  line L10[^\n]*\n│ c1 ");
    s.assert_row_contains(0, "[cursor L25:C1] src/big.ts");
    s.assert_row_matches(-1, r"^comment deleted \| ");
}

// ---- F-COMMENT-09 ---------------------------------------------------------------------------------------------

/// numbered note on a file that cannot be opened: note shows the read error, view unchanged.
#[test]
fn f_comment_09_browse_error() {
    let mut s = mcp_sim_size(120, 30);
    s.assert_row_contains(0, "[mcp: on]");
    annotate(&mut s, json!({"file": "nope.txt", "line": 1, "text": "ghost", "number": 1}));
    s.keys(")");
    s.assert_not_contains("[browse]");
    s.assert_row_matches(0, r"^\[all\] .*\[1/4\] \[cursor L2:C1\] README\.md");
    s.assert_row_matches(-1, r"^cannot read nope\.txt: not found \| ");
}

/// target file not in diff: opens it in browse; target in diff list: leaves browse and switches to it.
#[test]
fn f_comment_09_browse() {
    let mut s = mcp_sim_size(120, 30);
    s.assert_row_contains(0, "[mcp: on]");
    annotate(&mut s, json!({"file": "package.json", "line": 1, "text": "pkg note", "number": 1}));
    annotate(&mut s, json!({"file": "src/c.tsx", "line": 1, "text": "tsx note", "number": 2}));
    s.keys(")");
    // package.json is unchanged (not in diff) - browse
    s.assert_matches("▸ sent  #1 agent note L1[^\n]*\n┃ pkg note ");
    s.assert_row_matches(0, r"^\[browse\] .*\[cursor L1:C1\] package\.json");
    s.keys(")");
    s.assert_matches("▸ sent  #2 agent note L1[^\n]*\n┃ tsx note ");
    s.assert_not_contains("[browse]");
    s.assert_row_matches(0, r"^\[all\] .*\[3/4\] \[cursor L1:C1\] src/c\.tsx");
    s.keys(")");
    s.assert_matches("▸ sent  #1 agent note L1[^\n]*\n┃ pkg note ");
    s.assert_row_matches(0, r"^\[browse\] .*package\.json");
    // Esc unfocuses, second Esc leaves browse back to the diff file shown before (src/c.tsx)
    s.keys("<Esc><Esc>");
    s.assert_row_matches(0, r"^\[all\] .*\[3/4\] .*src/c\.tsx");
}

/// ) / ( walk numbered agent notes by number across files, wrapping; switches diff file and focuses the note.
#[test]
fn f_comment_09_cycle() {
    let mut s = mcp_sim();
    s.assert_row_contains(0, "[mcp: on]");
    annotate(&mut s, json!({"file": "README.md", "line": 3, "text": "note two", "number": 2}));
    annotate(&mut s, json!({"file": "src/c.tsx", "line": 1, "text": "note one", "number": 1}));
    annotate(&mut s, json!({"file": "README.md", "line": 1, "text": "note three", "number": 3}));
    annotate(&mut s, json!({"file": "README.md", "line": 2, "text": "not numbered"}));
    let one = (r"▸ sent  #1 agent note L1[^\n]*\n┃ note one ", r"\[3/4\] \[cursor L1:C1\] src/c\.tsx");
    let two = (r"▸ sent  #2 agent note L3[^\n]*\n┃ note two ", r"\[1/4\] \[cursor L3:C1\] README\.md");
    let three = (r"▸ sent  #3 agent note L1[^\n]*\n┃ note three ", r"\[1/4\] \[cursor L1:C1\] README\.md");
    let seq = [(")", one), (")", two), (")", three), (")", one), ("(", three), ("(", two), ("(", one)];
    // fresh start goes to the first by number; then walks, wrapping both ways
    for (k, (body, row0)) in seq {
        s.keys(k);
        s.assert_matches(body);
        s.assert_row_matches(0, row0);
    }
}

/// ( and ) without numbered comments note no numbered comments.
#[test]
fn f_comment_09_none() {
    let mut s = mcp_sim_size(120, 30);
    s.assert_row_contains(0, "[mcp: on]");
    s.keys(")");
    s.assert_not_contains("▸");
    s.assert_row_contains(0, "[cursor L2:C1] README.md");
    s.assert_row_matches(-1, r"^no numbered comments \| ");
    s.keys("a<Tab>savedq<Enter>");
    annotate(&mut s, json!({"file": "README.md", "line": 1, "text": "plain note"}));
    s.assert_contains("agent note L1");
    s.assert_contains("savedq");
    s.assert_row_matches(-1, r"^question saved \(\d\) \| ");
    s.keys("(");
    s.assert_not_contains("▸");
    s.assert_row_contains(0, "[cursor L2:C1] README.md");
    s.assert_row_matches(-1, r"^no numbered comments \| ");
}

/// start point: fresh ( = last; focused numbered comment beats last jumped; else last jumped.
#[test]
fn f_comment_09_start() {
    let mut s = mcp_sim();
    s.assert_row_contains(0, "[mcp: on]");
    annotate(&mut s, json!({"file": "README.md", "line": 3, "text": "note two", "number": 2}));
    annotate(&mut s, json!({"file": "src/c.tsx", "line": 1, "text": "note one", "number": 1}));
    annotate(&mut s, json!({"file": "README.md", "line": 1, "text": "note three", "number": 3}));
    annotate(&mut s, json!({"file": "README.md", "line": 2, "text": "not numbered"}));
    // fresh start - ( goes to the last by number (#3)
    s.keys("(");
    s.assert_matches("▸ sent  #3 agent note L1[^\n]*\n┃ note three ");
    s.assert_row_matches(0, r"\[1/4\] \[cursor L1:C1\] README\.md");
    // last jumped = #1 (src/c.tsx)
    s.keys("((");
    s.assert_matches("▸ sent  #1 agent note L1[^\n]*\n┃ note one ");
    s.assert_row_contains(0, "src/c.tsx");
    s.keys("<S-Tab><S-Tab>");
    s.assert_not_contains("▸");
    s.assert_row_matches(0, r"\[1/4\] .*README\.md");
    s.keys("J");
    s.assert_matches("▸ sent  #3 agent note L1[^\n]*\n┃ note three ");
    // starts from the focused #3 (wraps to #1), not from last jumped #1 (would give #2)
    s.keys(")");
    s.assert_matches("▸ sent  #1 agent note L1[^\n]*\n┃ note one ");
    s.assert_row_matches(0, r"\[3/4\] \[cursor L1:C1\] src/c\.tsx");
    s.keys("<S-Tab><S-Tab>JJ");
    s.assert_matches("▸ sent  agent note L2[^\n]*\n┃ not numbered ");
    // focused note has no number - starts from last jumped (#1), so #2
    s.keys(")");
    s.assert_matches("▸ sent  #2 agent note L3[^\n]*\n┃ note two ");
    s.assert_row_matches(0, r"\[1/4\] \[cursor L3:C1\] README\.md");
    s.keys("<Esc>j");
    s.assert_not_contains("▸");
    // unfocused - starts from last jumped (#2), so #1
    s.keys("(");
    s.assert_matches("▸ sent  #1 agent note L1[^\n]*\n┃ note one ");
    s.assert_row_contains(0, "src/c.tsx");
}

// ---- F-COMMENT-10 ---------------------------------------------------------------------------------------------

/// agent note boxes: heads, no saved · not asked, body = note text, old side on del row, code blocks.
#[test]
fn f_comment_10_render() {
    let mut s = mcp_sim();
    s.assert_row_contains(0, "[mcp: on]");
    annotate(
        &mut s,
        json!({"file": "README.md", "line": 1, "number": 4, "text": "see code\n```ts\nlet a = 1;\n```"}),
    );
    annotate(&mut s, json!({"file": "README.md", "line": 2, "side": "old", "text": "old side note"}));
    annotate(&mut s, json!({"file": "README.md", "line": 2, "text": "new side note"}));
    s.assert_not_contains("saved · not asked");
    s.assert_not_contains("```");
    s.assert_matches(
        "   1    1   # Title\n╭─+╮\n│ sent  #4 agent note L1 +│\n│ see code +│\n│ \\[ copy \\] ts +│\n│ │ let a = 1; +│\n╰─+╯",
    );
    s.assert_matches("   2      -[▶ ]hello *…?\n╭─+╮\n│ sent  agent note L2 +│\n│ old side note +│\n╰─+╯");
    s.assert_matches(
        r"        2 \+[▶ ]hello world *…?\n╭─+╮\n│ sent  agent note L2 +│\n│ new side note +│\n╰─+╯",
    );
    s.keys("J");
    // focused agent note - still no saved · not asked suffix
    s.assert_not_contains("saved · not asked");
    s.assert_matches("┃ ▸ sent  #4 agent note L1 +┃");
    // head in accent (solarized #cb4b16), copy button accent
    s.assert_text_cell("sent  #4 agent note", 0, C::new().fg("#cb4b16"));
    s.assert_text_cell("[ copy ]", 0, C::new().fg("#cb4b16"));
    // MCP stopped - agent notes still never get the saved · not asked suffix
    s.keys("M<Enter><Esc>");
    s.assert_row_contains(0, "[mcp: off]");
    s.assert_not_contains("saved · not asked");
    s.assert_matches(" #4 agent note L1 +┃\n┃ see code +┃");
    s.assert_matches(" agent note L2 +│\n│ old side note +│");
    s.assert_matches(" agent note L2 +│\n│ new side note +│");
}

// ---- F-EXPORT-01 ----------------------------------------------------------------------------------------------

/// export includes agent notes (count and content).
#[test]
fn f_export_01_agent_notes() {
    let mut s = mcp_sim_size(250, 20);
    s.assert_row_contains(0, "[mcp: on]");
    annotate(&mut s, json!({"file": "README.md", "line": 1, "text": "agent says hi"}));
    s.keys("E");
    // only an agent note - still exported
    let pat = format!(r"^exported 1 comment -> {}/xplain-review-\d{{8}}-\d{{6}}\.md ", repo_re(&s));
    s.assert_row_matches(-1, &pat);
    let (rel, text) = only_export(&s, ".");
    assert_has(&text, "- comments: 1\n");
    assert_has(&text, "- origin: agent\n");
    assert_has(&text, "> agent says hi\n");
    s.remove_file(&rel);
    s.keys("a<Tab>mine<Enter>E");
    s.assert_row_matches(-1, r"^exported 2 comments -> ");
    let (_, text) = only_export(&s, ".");
    assert_has(&text, "- comments: 2\n");
    assert_has(&text, "> agent says hi\n");
    assert_has(&text, "> mine\n");
}

/// write error (cwd not writable): note export failed: <path>: permission denied.
#[test]
fn f_export_01_error() {
    use std::os::unix::fs::PermissionsExt;
    let mut s = Sim::builder().size(200, 20).build();
    s.keys("ac<Enter>");
    // cwd (the repo) made read-only
    let repo = s.repo().to_path_buf();
    let chmod = |mode: u32| {
        std::fs::set_permissions(&repo, std::fs::Permissions::from_mode(mode)).expect("chmod repo")
    };
    chmod(0o555);
    s.keys("E");
    let row = s.row(-1);
    chmod(0o755);
    let pat =
        format!(r"^export failed: {}/xplain-review-\d{{8}}-\d{{6}}\.md: permission denied \| ", repo_re(&s));
    assert_re(&row, &pat);
    assert_eq!(s.git_code(&["ls-files", "--others", "--error-unmatch", "xplain-review-*"]), 1);
}

/// file name timestamp is local time: with a -12h offset its hour differs from the UTC date line.
#[test]
fn f_export_01_local_time_offset() {
    let mut s = Sim::builder().size(250, 20).utc_offset_secs(-12 * 3600).build();
    s.keys("ac<Enter>E");
    let pat = format!(r"^exported 1 comment -> {}/xplain-review-\d{{8}}-\d{{6}}\.md ", repo_re(&s));
    s.assert_row_matches(-1, &pat);
    let g = s.capture_row(-1, r"-> (\S+/xplain-review-\d{8}-(\d{2})\d{4}\.md) ");
    let (path, hour) = (g[1].clone(), g[2].clone());
    assert_eq!(exports(&s, ".").len(), 1);
    let text = s.file(&path);
    // local hour = UTC hour - 12, never equal; the date line itself stays UTC (Z)
    let re = rx(r"^- date: \d{4}-\d{2}-\d{2}T(\d{2}):\d{2}:\d{2}\.\d{3}Z$");
    let caps = re.captures(&text).unwrap_or_else(|| panic!("no date line\n{text}"));
    assert_ne!(&caps[1], hour, "hour must differ from the UTC date line\n{text}");
}

/// file name timestamp is local time: equals the UTC date line hour with offset 0.
#[test]
fn f_export_01_local_time() {
    let mut s = Sim::builder().size(250, 20).build();
    s.keys("ac<Enter>E");
    let pat = format!(r"^exported 1 comment -> {}/xplain-review-\d{{8}}-\d{{6}}\.md ", repo_re(&s));
    s.assert_row_matches(-1, &pat);
    let g = s.capture_row(-1, r"-> (\S+/xplain-review-(\d{4})(\d{2})(\d{2})-(\d{2})\d{4}\.md) ");
    assert_eq!(exports(&s, ".").len(), 1);
    let text = s.file(&g[1]);
    // file name YYYYMMDD-HH equals the `- date:` line (ISO UTC) YYYY-MM-DDTHH
    let want = format!(r"^- date: {}-{}-{}T{}:\d{{2}}:\d{{2}}\.\d{{3}}Z$", g[2], g[3], g[4], g[5]);
    assert_re(&text, &want);
}

/// E without comments: note no comments to export, no file written.
#[test]
fn f_export_01_none() {
    let mut s = Sim::builder().size(120, 20).build();
    s.keys("E");
    s.assert_row_matches(-1, r"^no comments to export \| \(1-5/5\) ");
    // no untracked xplain-review-* file in the repo
    assert_eq!(s.git_code(&["ls-files", "--others", "--error-unmatch", "xplain-review-*"]), 1);
}

/// export lands in the process cwd (app started in a repo subdirectory).
#[test]
fn f_export_01_process_cwd() {
    let mut s = Sim::builder().size(250, 20).cwd("src").build();
    s.keys("ac<Enter>E");
    let pat = format!(r"^exported 1 comment -> {}/src/xplain-review-\d{{8}}-\d{{6}}\.md ", repo_re(&s));
    s.assert_row_matches(-1, &pat);
    let (_, text) = only_export(&s, "src");
    assert_has(&text, "- comments: 1\n");
    assert_eq!(exports(&s, ".").len(), 0);
}

/// E writes <abs cwd>/xplain-review-YYYYMMDD-HHMMSS.md; note exported 1 comment, then 2 comments (plural).
#[test]
fn f_export_01_write() {
    let mut s = Sim::builder().size(250, 20).build();
    s.keys("aone<Enter>");
    s.keys("E");
    let pat =
        format!(r"^exported 1 comment -> {}/xplain-review-\d{{8}}-\d{{6}}\.md \| \(1-5/5\) ", repo_re(&s));
    s.assert_row_matches(-1, &pat);
    assert_eq!(s.git_code(&["ls-files", "--others", "--error-unmatch", "xplain-review-*.md"]), 0);
    // the path in the note is the one export file
    let out1 = s.capture_row(-1, r"^exported 1 comment -> (\S+) ")[1].clone();
    assert_eq!(exports(&s, ".").len(), 1);
    let text = s.file(&out1);
    assert_has(&text, &format!("- repo: `{}`\n", s.repo().display()));
    assert_has(&text, "- comments: 1\n");
    assert_has(&text, "> one\n");
    s.remove_file(&out1);
    s.keys("jatwo<Enter>");
    s.keys("E");
    let pat = format!(r"^exported 2 comments -> {}/xplain-review-\d{{8}}-\d{{6}}\.md \| ", repo_re(&s));
    s.assert_row_matches(-1, &pat);
    let (rel, text) = only_export(&s, ".");
    assert_re(&rel, r"^xplain-review-\d{8}-\d{6}\.md$");
    assert_has(&text, "- comments: 2\n");
    assert_has(&text, "> one\n");
    assert_has(&text, "> two\n");
}

// ---- F-EXPORT-02 ----------------------------------------------------------------------------------------------

/// agent note: origin agent, no Line / Context blocks, **Note:** quote, where <side> side, line <l>.
#[test]
fn f_export_02_agent_note() {
    let mut s = mcp_sim_size(250, 20);
    s.assert_row_contains(0, "[mcp: on]");
    annotate(
        &mut s,
        json!({"file": "README.md", "line": 2, "side": "old", "number": 7, "text": "first\n\nthird"}),
    );
    s.keys("E");
    s.assert_row_matches(-1, r"^exported 1 comment -> ");
    let (_, text) = only_export(&s, ".");
    assert_re(
        &text,
        r"- comments: 1

## `README\.md`

### 1\. old side, line 2

- origin: agent
- state: saved

\*\*Note:\*\*

> first
>
> third\n\z",
    );
    for n in ["Line:", "Context:", "**Comment:**", "Selected text:"] {
        assert_lacks(&text, n);
    }
}

/// diff line: unstaged mode, git args with backticks -> inline code with longer delimiters, padded.
#[test]
fn f_export_02_diff_args_ticks() {
    let mut s = Sim::builder().size(250, 20).args(["--unstaged", "--", "README.md", ":!q```"]).build();
    s.assert_row_matches(0, r"^\[unstaged\] .*README\.md");
    s.keys("ac<Enter>E");
    s.assert_row_matches(-1, r"^exported 1 comment -> ");
    // content ends with a run of 3 backticks -> fence 4 -> inline 2, padded with spaces
    let (_, text) = only_export(&s, ".");
    assert_has(&text, "\n- diff: `` unstaged -- README.md :!q``` ``\n");
}

/// diff line: mode plus git args (all with args).
#[test]
fn f_export_02_diff_args() {
    let mut s = Sim::builder().size(250, 20).args(["HEAD", "--", "README.md"]).build();
    s.keys("ac<Enter>E");
    s.assert_row_matches(-1, r"^exported 1 comment -> ");
    let (_, text) = only_export(&s, ".");
    assert_has(
        &text,
        &format!("- repo: `{}`\n- diff: `all HEAD -- README.md`\n- date: ", s.repo().display()),
    );
}

/// code fence length = max(3, longest backtick run + 1); inline code backticks = fence-2, padded.
#[test]
fn f_export_02_fences() {
    let mut s = Sim::builder().size(250, 20).file("`q```.txt", "one\ntwo ```` two\nthree\n").build();
    s.keys("<S-Tab><S-Tab>");
    s.assert_row_contains(0, "`q```.txt");
    s.keys(":2<Enter>");
    s.assert_row_contains(0, "[cursor L2:C1] `q```.txt");
    s.keys("aticks<Enter>E");
    s.assert_row_matches(-1, r"^exported 1 comment -> ");
    // path has a run of 3 -> fence 4 -> inline 2 backticks, padded; code has a run of 4 -> fence 5
    let (_, text) = only_export(&s, ".");
    assert_has(
        &text,
        "## `` `q```.txt ``\n\n### 1. new side, line 2\n\n- origin: human\n- state: saved\n\nLine:\n\n`````\ntwo ```` two\n`````\n\nContext:\n\n`````\none\ntwo ```` two\nthree\n`````\n\n**Comment:**\n\n> ticks\n",
    );
}

/// comment on a hunk row: where = <side> side, row <row+1>; Line block = hunk text; context = line rows only.
#[test]
fn f_export_02_hunk_row() {
    let mut s = Sim::builder().size(250, 20).build();
    s.keys("g");
    s.assert_row_contains(0, "[cursor r1:C1] README.md");
    s.keys("aon hunk<Enter>E");
    s.assert_row_matches(-1, r"^exported 1 comment -> ");
    let (_, text) = only_export(&s, ".");
    assert_has(
        &text,
        "### 1. new side, row 1

- origin: human
- state: saved

Line:

```
@@ -1,2 +1,3 @@
```

Context:

```
# Title
hello
hello world
```

**Comment:**

> on hunk
",
    );
}

/// markdown of one human line comment: whole document exact.
#[test]
fn f_export_02_line() {
    let mut s = Sim::builder().size(250, 20).build();
    s.keys("kalooks fine<Enter>");
    s.assert_row_contains(0, "[cursor L1:C1] README.md");
    s.keys("E");
    s.assert_row_matches(-1, r"^exported 1 comment -> ");
    let (_, text) = only_export(&s, ".");
    // \A / \z anchor start / end of file (matches is multiline)
    let pat = format!(
        r"\A# xplain review

- repo: `{}`
- diff: `all`
- date: \d{{4}}-\d\d-\d\dT\d\d:\d\d:\d\d\.\d{{3}}Z
- comments: 1

## `README\.md`

### 1\. new side, line 1

- origin: human
- state: saved

Line:

```
# Title
```

Context:

```
# Title
hello
hello world
more
```

\*\*Comment:\*\*

> looks fine\n\z",
        repo_re(&s)
    );
    assert_re(&text, &pat);
}

/// comment in the old pane (split): where = old side, line <l>, Line block = old text.
#[test]
fn f_export_02_old_side() {
    let mut s = Sim::builder().size(250, 20).args(["--split"]).build();
    s.keys("p");
    s.assert_row_contains(0, "[cursor old L2:C1] README.md");
    s.keys("aold one<Enter>E");
    s.assert_row_matches(-1, r"^exported 1 comment -> ");
    let (_, text) = only_export(&s, ".");
    assert_has(
        &text,
        "### 1. old side, line 2

- origin: human
- state: saved

Line:

```
hello
```
",
    );
    assert_has(&text, "**Comment:**\n\n> old one\n");
}

/// selection comments: where = selection line <a>, cols / selection lines <a>-<b>, cols; Selected text block.
#[test]
fn f_export_02_selection() {
    let mut s = Sim::builder().size(250, 30).build();
    s.keys("kvllll");
    s.assert_row_contains(0, "[visual L1:C5] README.md");
    s.keys("<Enter>chars<Enter>");
    // line mode over L1 .. L3 (del row hello has old no 2); endCol = len("more") = 4
    s.keys("Vjjj<Enter>lines<Enter>");
    s.keys("E");
    s.assert_row_matches(-1, r"^exported 2 comments -> ");
    let (_, text) = only_export(&s, ".");
    assert_has(
        &text,
        "### 1. new side, selection line 1, cols 1-5

- origin: human
- state: saved

Selected text:

```
# Tit
```

Context:

```
# Title
hello
hello world
more
```

**Comment:**

> chars
",
    );
    assert_has(
        &text,
        "### 2. new side, selection lines 1-3, cols 1-4

- origin: human
- state: saved

Selected text:

```
# Title
hello
hello world
more
```
",
    );
    assert_lacks(&text, "Line:");
    assert_lacks(&text, "\n\n\n");
}

/// files sorted by path in code-unit order; comments by line numerically, ties in creation order.
#[test]
fn f_export_02_sort() {
    let mut s = Sim::builder().size(250, 40).build();
    // browse package.json via file search and comment on its line 1
    s.keys("Fpackage.json<Enter>");
    s.assert_row_matches(0, r"^\[browse\] .*package\.json");
    s.keys("ap1<Enter><Esc>");
    s.assert_row_matches(0, r"^\[all\] .*README\.md");
    s.keys("Gar3<Enter>");
    s.keys("gjar1<Enter>");
    // row 1 is the hunk row (row index 0)
    s.keys("gah0<Enter>");
    s.keys("<Tab>");
    s.assert_row_contains(0, "src/big.ts");
    s.keys(":10<Enter>ab10<Enter>");
    s.keys(":9<Enter>ab9<Enter>");
    s.keys(":30<Enter>ab30<Enter>");
    s.keys(":10<Enter>ab10z<Enter>");
    s.keys("E");
    s.assert_row_matches(-1, r"^exported 8 comments -> ");
    // README.md < package.json < src/big.ts by code unit (uppercase first); 9 < 10 < 30 numerically
    let (_, text) = only_export(&s, ".");
    assert_has(&text, "- comments: 8\n");
    assert_re(
        &text,
        "## `README\\.md`\n\n### 1\\. new side, row 1\n[\\s\\S]*\n> h0\n\n### 2\\. new side, line 1\n[\\s\\S]*\n> r1\n\n### 3\\. new side, line 3\n[\\s\\S]*\n> r3\n\n## `package\\.json`\n\n### 4\\. new side, line 1\n[\\s\\S]*\n> p1\n\n## `src/big\\.ts`\n\n### 5\\. new side, line 9\n[\\s\\S]*\n> b9\n\n### 6\\. new side, line 10\n[\\s\\S]*\n> b10\n\n### 7\\. new side, line 10\n[\\s\\S]*\n> b10z\n\n### 8\\. new side, line 30\n[\\s\\S]*\n> b30\n\\z",
    );
}

/// state and answer blocks: answered (+ follow-up and its answer), streaming, pending, saved; cancelled after MCP stop.
#[test]
fn f_export_02_states() {
    let mut s = mcp_sim_size(250, 50);
    s.assert_row_contains(0, "[mcp: on]");
    // an initialized MCP session, so delivered answers carry its client name
    let r = s.mcp_rpc(
        "initialize",
        json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "statebot", "version": "1"}}),
    );
    assert_eq!(r.status, 200);
    let sid = r.header("mcp-session-id").expect("session id").to_string();
    let next = |s: &mut Sim| {
        s.http(Http::tool("next_question", json!({"wait_seconds": 1})).session(&sid)).tool_result()
    };

    // c1 on L1, asked and answered, then a follow-up answered
    s.keys("kaq one<Enter>");
    let q = next(&mut s);
    assert_eq!(q["status"], "question");
    assert_eq!(q["turn"], 1);
    let t1 = q["thread_id"].as_str().unwrap_or_default().to_string();
    let r =
        s.http(Http::tool("answer", json!({"thread_id": t1, "text": "ans one\nsecond line"})).session(&sid));
    assert_eq!(r.tool_result()["ok"], true);
    s.keys("Ja");
    s.keys("more please<Enter>");
    s.assert_row_matches(-1, r"^follow-up queued \| ");
    let q = next(&mut s);
    assert_eq!(q["status"], "question");
    assert_eq!(q["turn"], 2);
    assert_eq!(q["follow_up"], true);
    let r = s.http(Http::tool("answer", json!({"thread_id": t1, "text": "ans two"})).session(&sid));
    assert_eq!(r.tool_result()["ok"], true);
    // c2 on the del row L2, delivered, no answer (streaming)
    s.keys("<Esc>jaq two<Enter>");
    let q = next(&mut s);
    assert_eq!(q["status"], "question");
    assert_eq!(q["turn"], 1);
    assert!(q["question"].as_str().is_some_and(|t| t.contains("q two")), "{q}");
    // c3 on L3 queued, not delivered (pending); c4 on L3 saved
    s.keys("Gaq three<Enter>");
    s.keys("a<Tab>q four<Enter>");
    s.assert_row_matches(-1, r"^question saved \(4\) \| ");
    s.keys("E");
    s.assert_row_matches(-1, r"^exported 4 comments -> ");
    let (rel, text) = only_export(&s, ".");
    assert_lacks(&text, "\n\n\n");
    assert_re(
        &text,
        "### 1\\. new side, line 1\n\n- origin: human\n- state: answered\n\nLine:[\\s\\S]*?\\*\\*Comment:\\*\\*\n\n> q one\n\n\\*\\*Answer \\(statebot\\) - done\\*\\*\n\n> ans one\n> second line\n\n\\*\\*Follow-up 1:\\*\\*\n\n> more please\n\n\\*\\*Answer \\(statebot\\) - done\\*\\*\n\n> ans two\n\n### 2\\. ",
    );
    assert_re(
        &text,
        "### 2\\. new side, line 2\n\n- origin: human\n- state: streaming\n\nLine:\n\n```\nhello\n```\n[\\s\\S]*?\\*\\*Comment:\\*\\*\n\n> q two\n\n\\*\\*Answer \\(statebot\\) - streaming\\*\\*\n",
    );
    assert_re(
        &text,
        "### 3\\. new side, line 3\n\n- origin: human\n- state: pending\n\n[\\s\\S]*?\\*\\*Comment:\\*\\*\n\n> q three\n\n\\*\\*Answer - pending\\*\\*\n",
    );
    assert_re(
        &text,
        "### 4\\. new side, line 3\n\n- origin: human\n- state: saved\n\n[\\s\\S]*?\\*\\*Comment:\\*\\*\n\n> q four\n\\z",
    );
    s.remove_file(&rel);
    // stop MCP - the delivered question is cancelled (agent name after stop not asserted)
    s.keys("M<Space><Esc>");
    s.assert_row_contains(0, "[mcp: off]");
    s.keys("E");
    s.assert_row_matches(-1, r"^exported 4 comments -> ");
    let (_, text) = only_export(&s, ".");
    assert_re(&text, "### 1\\. new side, line 1\n\n- origin: human\n- state: answered\n");
    assert_re(
        &text,
        "### 2\\. new side, line 2\n\n- origin: human\n- state: cancelled\n[\\s\\S]*?\n> q two\n\n\\*\\*Answer( \\(statebot\\))? - cancelled\\*\\*\n\n> MCP stopped\n\n### 3\\. ",
    );
}

/// Line block keeps tabs raw; Selected text has tabs expanded; context raw.
#[test]
fn f_export_02_tabs() {
    let mut s = Sim::builder().size(250, 20).file("tab.txt", "a\tb\n").build();
    s.keys("<S-Tab><S-Tab>");
    s.assert_row_contains(0, "tab.txt");
    s.keys("jaline c<Enter>");
    s.assert_row_contains(0, "[cursor L1:C1] tab.txt");
    s.keys("Vasel c<Enter>E");
    s.assert_row_matches(-1, r"^exported 2 comments -> ");
    let (_, text) = only_export(&s, ".");
    assert_has(&text, "Line:\n\n```\na\tb\n```\n\nContext:\n\n```\na\tb\n```\n\n**Comment:**\n\n> line c\n");
    assert_re(
        &text,
        "Selected text:\n\n```\na +b\n```\n\nContext:\n\n```\na\tb\n```\n\n\\*\\*Comment:\\*\\*\n\n> sel c\n",
    );
}
