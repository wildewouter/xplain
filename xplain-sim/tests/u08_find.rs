//! Scenario tests: find.

mod search_support;

use search_support::*;
use xplain_sim::{CellExpect as C, Fixture, Sim};

/// F-FIND-01: / inside the comment editor is typed text, does not open find
#[test]
fn f_find_01_editor() {
    let mut s =
        Sim::builder().fixture(Fixture::Empty).size(80, 12).file("f.txt", "alpha beta\ngamma\n").build();
    s.keys("a");
    s.assert_row_contains(-1, "enter send");
    s.keys("/x");
    s.assert_contains(" /x");
    s.assert_row_contains(-1, "enter send");
    assert_row_not_contains(&s, -1, "█");
    s.keys("<Enter>");
    s.assert_row_matches(-1, r#"^question saved \(1\) \| \(1-3/3\) "#);
}

/// F-FIND-01: / Enter sets term (footer /term | ), Esc keeps old term, empty Enter clears term + highlights, no-match note
#[test]
fn f_find_01_enter() {
    let mut s = Sim::builder()
        .fixture(Fixture::Empty)
        .size(120, 12)
        .file("f.txt", "alpha beta\ngamma Alpha\ndelta\naaa\n\tbeta alpha\n")
        .build();
    s.assert_row_contains(0, "[cursor L1:C1] f.txt");
    s.assert_row_matches(-1, r#"^\(1-6/6\) hjkl move"#);
    s.keys("/delta<Enter>");
    // term set, cursor jumped to match
    s.assert_row_contains(0, "[cursor L3:C1] f.txt");
    s.assert_row_matches(-1, r#"^/delta \| \(1-6/6\) hjkl move  enter ask  J/K comments  \? help$"#);
    s.keys("/gam");
    // while open the find line replaces the term part
    s.assert_row_matches(-1, r#"^/gam█\(1-6/6\) hjkl move"#);
    s.keys("<Esc>");
    // Esc keeps the earlier term, no move
    s.assert_row_contains(0, "[cursor L3:C1] f.txt");
    s.assert_row_matches(-1, r#"^/delta \| \(1-6/6\) hjkl move"#);
    s.keys("k");
    // term active, delta on L3 highlighted
    let (x, y) = cell_pos(&s, "delta", 0, 0, Some(5));
    s.assert_cell(x, y, C::new().fg("0").bg("3"));
    s.keys("/<Enter>");
    // empty Enter clears term, no move
    s.assert_row_contains(0, "[cursor L2:C1] f.txt");
    s.assert_row_matches(-1, r#"^\(1-6/6\) hjkl move  enter ask  J/K comments  \? help$"#);
    let (x, y) = cell_pos(&s, "delta", 0, 0, Some(5));
    s.assert_cell(x, y, C::new().bg("default"));
    s.keys("/zzz<Enter>");
    // no match anywhere; term still set, note shown, no move
    s.assert_row_contains(0, "[cursor L2:C1] f.txt");
    s.assert_row_matches(
        -1,
        r#"^/zzz \| pattern not found: zzz \| \(1-6/6\) hjkl move  enter ask  J/K comments  \? help$"#,
    );
    s.keys("/");
    // open find line replaces both term and note parts
    s.assert_row_matches(-1, r#"^/█\(1-6/6\) hjkl move  enter ask  J/K comments  \? help$"#);
}

/// F-FIND-01: / opens find line: typing, ? typed, BS/Del, ctrl/meta/tab/arrows ignored, Esc closes, count prefix dropped
#[test]
fn f_find_01_input() {
    let mut s = Sim::builder()
        .fixture(Fixture::Empty)
        .size(80, 12)
        .file("f.txt", "alpha beta\ngamma Alpha\ndelta\naaa\n\tbeta alpha\n")
        .build();
    s.assert_row_contains(0, "[1/1] [cursor L1:C1] f.txt +5 -0");
    s.assert_row_matches(3, r#"^        1 \+▶alpha beta"#);
    s.assert_row_matches(-1, r#"^\(1-6/6\) hjkl move  enter ask  J/K comments  \? help$"#);
    s.keys("3/");
    // find line open, empty text, rest of footer kept
    s.assert_row_matches(-1, r#"^/█\(1-6/6\) hjkl move  enter ask  J/K comments  \? help$"#);
    s.keys("al?");
    // ? is typed, help panel not opened
    s.assert_not_contains("Help ·");
    s.assert_row_matches(-1, r#"^/al\?█\(1-6/6\) "#);
    s.keys("<BS>");
    s.assert_row_matches(-1, r#"^/al█\(1-6/6\) "#);
    s.keys("<Del>");
    s.assert_row_matches(-1, r#"^/a█\(1-6/6\) "#);
    s.keys("<Tab><Left><Right><Up><Down><C-a><C-x><A-b><M-z><Home><End><PageDown>");
    // ctrl, meta, tab, arrows ignored (not typed, cursor unmoved)
    s.assert_row_contains(0, "[cursor L1:C1] f.txt");
    s.assert_row_matches(-1, r#"^/a█\(1-6/6\) "#);
    s.keys("<Space>b");
    s.assert_row_matches(-1, r#"^/a b█\(1-6/6\) "#);
    s.keys("<Esc>");
    // Esc closes; no term was set before, so none now; no move
    s.assert_row_contains(0, "[cursor L1:C1] f.txt");
    s.assert_row_matches(-1, r#"^\(1-6/6\) hjkl move  enter ask  J/K comments  \? help$"#);
    s.keys("j");
    // count 3 typed before / was dropped, j moves one row
    s.assert_row_contains(0, "[cursor L2:C1] f.txt");
    s.assert_row_matches(4, r#"^        2 \+▶gamma Alpha"#);
    // each CR/LF run in a paste (one chunk) becomes one space
    s.keys("/");
    s.paste("a\r\n\r\nb\nc");
    s.assert_row_matches(-1, "^/a b c█");
}

/// F-FIND-01: find line keeps the rest of the footer (too narrow for split part stays)
#[test]
fn f_find_01_narrow() {
    let mut s = Sim::builder()
        .fixture(Fixture::Empty)
        .size(80, 12)
        .args(["--split"])
        .file("f.txt", "alpha beta\ngamma\n")
        .build();
    s.assert_row_matches(-1, r#"^too narrow for split \| \(1-3/3\) hjkl move"#);
    s.keys("/be");
    s.assert_row_matches(-1, r#"^/be█too narrow for split \| \(1-3/3\) hjkl move"#);
    s.keys("<Enter>");
    s.assert_row_contains(0, "[cursor L1:C7] f.txt");
    s.assert_row_matches(-1, r#"^/be \| too narrow for split \| \(1-3/3\) hjkl move"#);
}

/// F-FIND-02: hunk header rows and note rows (binary, no textual changes) never match
#[test]
fn f_find_02_nonmatch_rows() {
    let mut s = Sim::builder()
        .fixture(Fixture::Empty)
        .size(120, 12)
        .file("a.txt", "hello\n")
        .file("b.bin", "a\0b\n")
        .file("e.txt", "")
        .build();
    s.assert_row_contains(0, "[1/3] [cursor L1:C1] a.txt");
    s.assert_row_matches(2, r#"^@@ -0,0 \+1 @@"#);
    s.keys("/@@<Enter>");
    s.assert_row_contains(0, "[cursor L1:C1] a.txt");
    s.assert_row_matches(-1, r#"^/@@ \| pattern not found: @@ \| "#);
    s.keys("/-0,0<Enter>");
    s.assert_row_matches(-1, r#"^/-0,0 \| pattern not found: -0,0 \| "#);
    s.keys("<Tab>");
    s.assert_row_contains(0, "[2/3] [cursor r1:C1] b.bin");
    s.assert_row_contains(2, "Binary file");
    s.keys("/Binary<Enter>");
    s.assert_row_contains(0, "[cursor r1:C1] b.bin");
    s.assert_row_matches(-1, r#"^/Binary \| pattern not found: Binary \| "#);
    s.keys("<Tab>");
    s.assert_row_contains(0, "[3/3] [cursor r1:C1] e.txt");
    s.assert_row_contains(2, "No textual changes");
    s.keys("/textual<Enter>");
    s.assert_row_contains(0, "[cursor r1:C1] e.txt");
    s.assert_row_matches(-1, r#"^/textual \| pattern not found: textual \| "#);
}

/// F-FIND-02: smartcase: lowercase term case-insensitive, any uppercase case-sensitive; all hits highlighted yellow/black
#[test]
fn f_find_02_smartcase() {
    let mut s = Sim::builder()
        .fixture(Fixture::Empty)
        .size(120, 12)
        .file("f.txt", "alpha beta\ngamma Alpha\ndelta\naaa\n\tbeta alpha\n")
        .build();
    s.keys("jj");
    s.assert_row_contains(0, "[cursor L3:C1] f.txt");
    s.assert_row_matches(-1, r#"^\(1-6/6\) hjkl move"#);
    // no term yet, no highlight
    s.assert_cell(12, 3, C::new().bg("default"));
    s.keys("/alpha<Enter>");
    s.assert_row_contains(0, "[cursor L5:C8] f.txt");
    s.assert_row_matches(-1, r#"^/alpha \| \(1-6/6\) hjkl move"#);
    // L1 "alpha" (cols 12-16) highlighted, following space not
    s.assert_cell(12, 3, C::new().fg("0").bg("3"));
    s.assert_cell(16, 3, C::new().fg("0").bg("3"));
    s.assert_cell(17, 3, C::new().bg("default"));
    // L2 "Alpha" matches case-insensitively; "gamma" not
    let (x, y) = cell_pos(&s, "Alpha", 0, 0, Some(4));
    s.assert_cell(x, y, C::new().fg("0").bg("3"));
    let (x, y) = cell_pos(&s, "Alpha", 0, 4, Some(4));
    s.assert_cell(x, y, C::new().fg("0").bg("3"));
    let (x, y) = cell_pos(&s, "gamma", 0, 0, Some(4));
    s.assert_cell(x, y, C::new().bg("default"));
    let (x, y) = cell_pos(&s, "delta", 0, 0, Some(5));
    s.assert_cell(x, y, C::new().bg("default"));
    s.keys("/Alpha<Enter>");
    // uppercase term is case-sensitive; from L5 wraps to the only hit L2
    s.assert_row_contains(0, "[cursor L2:C7] f.txt");
    s.assert_row_matches(-1, r#"^/Alpha \| \(1-6/6\) hjkl move"#);
    s.assert_cell(12, 3, C::new().bg("default"));
    let (x, y) = cell_pos(&s, "alpha", 0, 0, Some(7));
    s.assert_cell(x, y, C::new().bg("default"));
    s.keys("/ALPHA<Enter>");
    s.assert_row_contains(0, "[cursor L2:C7] f.txt");
    s.assert_row_matches(-1, r#"^/ALPHA \| pattern not found: ALPHA \| \(1-6/6\) "#);
}

/// F-FIND-02: split: row matches if either side matches, hits highlighted in both panes; jump column from cursor pane text, else first hit in row
#[test]
fn f_find_02_split() {
    let mut s = Sim::builder()
        .fixture(Fixture::Empty)
        .size(120, 12)
        .args(["--split"])
        .file("g.txt", "ab foo\nold xyz\nkeep\n")
        .build();
    s.git(&["add", "-A"]);
    s.git(&["commit", "-qm", "base"]);
    s.write_file("g.txt", "c foo\nnew line text\nkeep\n");
    s.keys("r");
    s.keys("g");
    s.assert_row_contains(0, "[split] [solarized] [mcp: off] [1/1] [cursor new r1:C1] g.txt +2 -2");
    s.assert_row_matches(3, "ab foo.*│.*c foo");
    s.assert_row_matches(4, "old xyz.*│.*new line text");
    s.assert_row_matches(-1, r#"\(1-4/4\) hjkl move  enter ask  J/K comments  p pane  \? help$"#);
    s.keys("/foo<Enter>");
    // pane new, first hit in right text "c foo" is col 3
    s.assert_row_contains(0, "[cursor new L1:C3] g.txt");
    s.assert_row_matches(-1, r#"^/foo \| "#);
    s.keys("j");
    // both panes of row L1 highlighted
    let (x, y) = cell_pos(&s, "foo", 0, 0, Some(3));
    s.assert_cell(x, y, C::new().fg("0").bg("3"));
    let (x, y) = cell_pos(&s, "foo", 1, 0, Some(3));
    s.assert_cell(x, y, C::new().fg("0").bg("3"));
    let (x, y) = cell_pos(&s, "ab", 0, 0, Some(3));
    s.assert_cell(x, y, C::new().bg("default"));
    s.keys("g0p");
    s.assert_row_contains(0, "[cursor old r1:C1] g.txt");
    s.keys("/foo<Enter>");
    // pane old, first hit in left text "ab foo" is col 4
    s.assert_row_contains(0, "[cursor old L1:C4] g.txt");
    s.keys("g0p");
    s.assert_row_contains(0, "[cursor new r1:C1] g.txt");
    s.keys("/xyz<Enter>");
    // only the left (old) side has xyz; row matches, col from first hit in row texts
    s.assert_row_contains(0, "[cursor new L2:C5] g.txt");
    s.assert_row_matches(-1, r#"^/xyz \| "#);
    s.keys("k");
    let (x, y) = cell_pos(&s, "xyz", 0, 0, Some(4));
    s.assert_cell(x, y, C::new().fg("0").bg("3"));
    let (x, y) = cell_pos(&s, "old", 0, 0, Some(4));
    s.assert_cell(x, y, C::new().bg("default"));
}

/// F-FIND-02: plain substring (no regex); non-overlapping hits left to right (aa in aaa: one hit at col 1)
#[test]
fn f_find_02_substring() {
    let mut s = Sim::builder()
        .fixture(Fixture::Empty)
        .size(120, 12)
        .file("f.txt", "alpha beta\ngamma Alpha\ndelta\naaa\n\tbeta alpha\n")
        .build();
    s.assert_row_contains(0, "[cursor L1:C1] f.txt");
    s.keys("/a.p<Enter>");
    // regex a.p would match alp; plain substring does not
    s.assert_row_contains(0, "[cursor L1:C1] f.txt");
    s.assert_row_matches(-1, r#"^/a\.p \| pattern not found: a\.p \| "#);
    s.keys("/a+<Enter>");
    s.assert_row_contains(0, "[cursor L1:C1] f.txt");
    s.assert_row_matches(-1, r#"^/a\+ \| pattern not found: a\+ \| "#);
    s.keys("/aa<Enter>");
    s.assert_row_contains(0, "[cursor L4:C1] f.txt");
    s.assert_row_matches(-1, r#"^/aa \| "#);
    s.keys("k");
    s.assert_row_contains(0, "[cursor L3:C1] f.txt");
    s.assert_row_matches(6, r#"^        4 \+ aaa$"#);
    // aaa = one hit at cols 1-2, third a not highlighted
    s.assert_cell(12, 6, C::new().fg("0").bg("3"));
    s.assert_cell(13, 6, C::new().fg("0").bg("3"));
    s.assert_cell(14, 6, C::new().bg("default"));
}

/// F-FIND-02: tabs match as 2 spaces; hit columns counted on tab-expanded text
#[test]
fn f_find_02_tabs() {
    let mut s = Sim::builder()
        .fixture(Fixture::Empty)
        .size(120, 12)
        .file("f.txt", "alpha beta\ngamma Alpha\ndelta\naaa\n\tbeta alpha\n")
        .build();
    s.assert_row_contains(0, "[cursor L1:C1] f.txt");
    s.assert_row_matches(7, r#"^        5 \+   beta alpha$"#);
    s.keys("/  beta<Enter>");
    // two spaces + beta only matches the tab line (L1 has a single space)
    s.assert_row_contains(0, "[cursor L5:C1] f.txt");
    s.assert_row_matches(-1, r#"^/  beta \| \(1-6/6\) "#);
    s.keys("/beta<Enter>");
    // Enter matches current row first; beta after the tab is col 3
    s.assert_row_contains(0, "[cursor L5:C3] f.txt");
    s.keys("k");
    // L1 beta at code col 7 (screen col 18) highlighted
    s.assert_cell(18, 3, C::new().fg("0").bg("3"));
    s.assert_cell(21, 3, C::new().fg("0").bg("3"));
    s.assert_cell(17, 3, C::new().bg("default"));
    // L5 beta drawn after the 2-space tab (screen cols 14-17)
    s.assert_cell(14, 7, C::new().fg("0").bg("3"));
    s.assert_cell(17, 7, C::new().fg("0").bg("3"));
    s.assert_cell(13, 7, C::new().bg("default"));
}

/// F-FIND-03: / and n/N work in browse (file viewer)
#[test]
fn f_find_03_browse() {
    let mut s = Sim::builder().size(80, 12).build();
    s.keys("F");
    s.keys("src/big.ts");
    s.assert_contains("src/big.ts");
    s.keys("<Enter>");
    s.assert_row_matches(0, r#"^\[browse\] .*\[cursor L1:C1\] src/big\.ts"#);
    s.assert_row_matches(-1, r#"^\(1-9/60\) hjkl move"#);
    s.keys("n");
    // no term, n does nothing
    s.assert_row_contains(0, "[cursor L1:C1] src/big.ts");
    s.keys("/v6<Enter>");
    s.assert_row_matches(0, r#"^\[browse\] .*\[cursor L6:C7\] src/big\.ts"#);
    s.assert_row_matches(-1, r#"^/v6 \| \(1-9/60\) "#);
    s.keys("n");
    s.assert_row_matches(0, r#"^\[browse\] .*\[cursor L60:C7\] src/big\.ts"#);
    s.assert_row_matches(-1, r#"^/v6 \| \(52-60/60\) "#);
    s.keys("n");
    s.assert_row_matches(0, r#"^\[browse\] .*\[cursor L6:C7\] src/big\.ts"#);
    s.keys("N");
    s.assert_row_matches(0, r#"^\[browse\] .*\[cursor L60:C7\] src/big\.ts"#);
    s.keys("k");
    let (x, y) = cell_pos(&s, "v60", 0, 0, Some(10));
    s.assert_cell(x, y, C::new().fg("0").bg("3"));
}

/// F-FIND-03: n/N work while a comment is focused
#[test]
fn f_find_03_focused() {
    let mut s = Sim::builder().size(120, 12).build();
    s.keys("/o<Enter>0");
    s.assert_row_contains(0, "[cursor L2:C1] README.md");
    s.assert_row_matches(4, "^   2      -▶hello");
    s.keys("ac1<Enter>");
    s.assert_contains("c1");
    s.keys("J");
    s.assert_row_contains(-1, "e edit  D delete");
    s.keys("n");
    // next match after the del row is the add row "hello world"
    s.assert_matches(r#"^        2 \+▶hello world"#);
    s.assert_row_contains(0, "[cursor L2:C5] README.md");
    s.keys("J");
    s.assert_matches("^   2      -▶hello");
    s.assert_row_contains(0, "[cursor L2:C");
    s.assert_row_contains(-1, "e edit  D delete");
    s.keys("N");
    // nothing before the del row matches o; wraps to last match "more"
    s.assert_matches(r#"^        3 \+▶more"#);
    s.assert_row_contains(0, "[cursor L3:C2] README.md");
}

/// F-FIND-03: Enter jumps to first match >= cursor row (wraps); n/N next/prev with wrap; column = first hit; viewport follows; selection cleared
#[test]
fn f_find_03_jump_wrap() {
    let mut s = Sim::builder().size(80, 12).build();
    s.keys("<Tab>");
    s.assert_row_contains(0, "[2/4] [cursor L30:C1] src/big.ts");
    s.assert_row_matches(-1, r#"^\(28-36/62\) hjkl move"#);
    s.keys("/v5<Enter>");
    // v5 hits v5 (row 5) and v50-v59; first at/after cursor is v50, viewport follows (margin 2)
    s.assert_row_contains(0, "[cursor L50:C7] src/big.ts");
    s.assert_row_matches(-1, r#"^/v5 \| \(46-54/62\) hjkl move"#);
    s.assert_row_matches(8, "^  50   50  ▶const v50 = 1;");
    s.keys("n");
    s.assert_row_contains(0, "[cursor L51:C7] src/big.ts");
    s.keys("N");
    s.assert_row_contains(0, "[cursor L50:C7] src/big.ts");
    s.keys("N");
    // last match before v50 is v5
    s.assert_row_contains(0, "[cursor L5:C7] src/big.ts");
    s.assert_row_matches(-1, r#"^/v5 \| \(4-12/62\) "#);
    s.keys("N");
    // nothing before v5, wraps to last match v59
    s.assert_row_contains(0, "[cursor L59:C7] src/big.ts");
    s.keys("n");
    // nothing after v59, wraps to first match v5
    s.assert_row_contains(0, "[cursor L5:C7] src/big.ts");
    s.assert_row_matches(-1, r#"^/v5 \| \(4-12/62\) "#);
    s.keys("/v5<Enter>");
    // Enter includes the cursor row itself (n would go to v50)
    s.assert_row_contains(0, "[cursor L5:C7] src/big.ts");
    s.keys("G");
    s.assert_row_contains(0, "[cursor L60:C7] src/big.ts");
    s.keys("/v1<Enter>");
    // no match at/after last row, Enter wraps to first match v1
    s.assert_row_contains(0, "[cursor L1:C7] src/big.ts");
    s.assert_row_matches(-1, r#"^/v1 \| \(1-9/62\) "#);
    s.keys("vll");
    s.assert_row_contains(0, "[visual L1:C9] src/big.ts");
    s.keys("n");
    // jump ends the selection, column reset to the hit
    s.assert_row_contains(0, "[cursor L10:C7] src/big.ts");
    s.assert_row_matches(-1, r#"^/v1 \| \(5-13/62\) hjkl move  enter ask"#);
}

/// F-FIND-03: n/N without term do nothing; with term but no match in view: note pattern not found: <term>, no move
#[test]
fn f_find_03_noterm() {
    let mut s = Sim::builder().size(120, 12).build();
    s.assert_row_contains(0, "[1/4] [cursor L2:C1] README.md");
    s.assert_row_matches(-1, r#"^\(1-5/5\) hjkl move"#);
    s.keys("nN");
    s.assert_row_contains(0, "[1/4] [cursor L2:C1] README.md");
    s.assert_row_matches(4, "^   2      -▶hello");
    s.assert_row_matches(-1, r#"^\(1-5/5\) hjkl move  enter ask  J/K comments  \? help$"#);
    s.keys("/Title<Enter>");
    s.assert_row_contains(0, "[cursor L1:C3] README.md");
    s.assert_row_matches(-1, r#"^/Title \| \(1-5/5\) "#);
    s.keys("<Tab>");
    s.assert_row_contains(0, "[2/4] [cursor L30:C1] src/big.ts");
    s.assert_row_matches(-1, r#"^/Title \| \("#);
    s.keys("n");
    s.assert_row_contains(0, "[cursor L30:C1] src/big.ts");
    s.assert_row_matches(-1, r#"^/Title \| pattern not found: Title \| \("#);
    s.keys(":abc<Enter>");
    s.assert_row_matches(-1, r#"^/Title \| not a line number: abc \| "#);
    s.keys("N");
    s.assert_row_contains(0, "[cursor L30:C1] src/big.ts");
    s.assert_row_matches(-1, r#"^/Title \| pattern not found: Title \| \("#);
}

/// F-GOTO-01: : opens goto line: any char typed, BS/Del, Esc cancels, empty Enter no-op, spaces-only note, trimmed number jumps
#[test]
fn f_goto_01_input() {
    let mut s = Sim::builder().size(120, 12).build();
    s.assert_row_contains(0, "[1/4] [cursor L2:C1] README.md");
    s.assert_row_matches(4, "^   2      -▶hello");
    s.assert_row_matches(-1, r#"^\(1-5/5\) hjkl move  enter ask  J/K comments  \? help$"#);
    s.keys("2:");
    s.assert_row_matches(-1, r#"^:█\(1-5/5\) hjkl move  enter ask  J/K comments  \? help$"#);
    s.keys("ab?");
    // any char accepted, ? typed (no help panel)
    s.assert_not_contains("Help ·");
    s.assert_row_matches(-1, r#"^:ab\?█\(1-5/5\) "#);
    s.keys("<BS>");
    s.assert_row_matches(-1, r#"^:ab█\(1-5/5\) "#);
    s.keys("<Del>");
    s.assert_row_matches(-1, r#"^:a█\(1-5/5\) "#);
    s.keys("<Space>x");
    s.assert_row_matches(-1, r#"^:a x█\(1-5/5\) "#);
    s.keys("<Esc>");
    // Esc cancels, no jump, no note
    s.assert_row_contains(0, "[cursor L2:C1] README.md");
    s.assert_row_matches(4, "^   2      -▶hello");
    s.assert_row_matches(-1, r#"^\(1-5/5\) hjkl move  enter ask  J/K comments  \? help$"#);
    s.keys("j");
    // count typed before : was dropped, j moves one row
    s.assert_row_matches(5, r#"^        2 \+▶hello world"#);
    s.keys(":<Enter>");
    // empty text, Enter closes without jump or note
    s.assert_row_matches(5, r#"^        2 \+▶hello world"#);
    s.assert_row_matches(-1, r#"^\(1-5/5\) hjkl move  enter ask  J/K comments  \? help$"#);
    s.keys(": 3 <Enter>");
    // text trimmed, exact hit, no note
    s.assert_row_contains(0, "[cursor L3:C1] README.md");
    s.assert_row_matches(6, r#"^        3 \+▶more"#);
    s.assert_row_matches(-1, r#"^\(1-5/5\) hjkl move  enter ask  J/K comments  \? help$"#);
    s.keys(":   <Enter>");
    // spaces only, note with empty text, no move
    s.assert_row_contains(0, "[cursor L3:C1] README.md");
    s.assert_row_matches(-1, r#"^not a line number:  \| \(1-5/5\) hjkl move"#);
    s.keys("/Title<Enter>");
    s.assert_row_matches(-1, r#"^/Title \| "#);
    s.keys(":");
    // open goto line replaces term and note parts
    s.assert_row_matches(-1, r#"^:█\(1-5/5\) hjkl move  enter ask  J/K comments  \? help$"#);
    s.keys("<Esc>");
    s.assert_row_matches(-1, r#"^/Title \| "#);
    // newlines in a paste (one chunk) are removed
    s.keys(":");
    s.paste("1\r\n\n2");
    s.assert_row_matches(-1, r#"^:12█\(1-5/5\) "#);
}

/// F-GOTO-02: goto errors: not a line number (non-digits, 0, sign, decimal), out of range in full scope; no jump
#[test]
fn f_goto_02_errors() {
    let mut s = Sim::builder().size(120, 12).build();
    s.assert_row_contains(0, "[full] [unified] [solarized] [mcp: off] [1/4] [cursor L2:C1] README.md");
    s.assert_row_matches(4, "^   2      -▶hello");
    s.keys(":abc<Enter>");
    s.assert_row_matches(-1, r#"^not a line number: abc \| \(1-5/5\) hjkl move"#);
    s.keys(":0<Enter>");
    s.assert_row_matches(-1, r#"^not a line number: 0 \| \(1-5/5\) "#);
    s.keys(":-1<Enter>");
    s.assert_row_matches(-1, r#"^not a line number: -1 \| \(1-5/5\) "#);
    s.keys(":+2<Enter>");
    s.assert_row_matches(-1, r#"^not a line number: \+2 \| \(1-5/5\) "#);
    s.keys(":1.5<Enter>");
    s.assert_row_matches(-1, r#"^not a line number: 1\.5 \| \(1-5/5\) "#);
    s.keys(":2x<Enter>");
    s.assert_row_matches(-1, r#"^not a line number: 2x \| \(1-5/5\) "#);
    s.keys(":4<Enter>");
    s.assert_row_matches(-1, r#"^line 4 out of range \(1-3\) \| \(1-5/5\) "#);
    s.keys(":1000<Enter>");
    s.assert_row_contains(0, "[cursor L2:C1] README.md");
    s.assert_row_matches(4, "^   2      -▶hello");
    s.assert_row_matches(-1, r#"^line 1000 out of range \(1-3\) \| \(1-5/5\) "#);
}

/// F-GOTO-02: goto exact line by new-side number (deleted-only rows ignored), no note; unfocuses comment; viewport follows
#[test]
fn f_goto_02_exact() {
    let mut s = Sim::builder().size(120, 12).build();
    s.assert_row_contains(0, "[cursor L2:C1] README.md");
    s.assert_row_matches(4, "^   2      -▶hello");
    s.keys(":2<Enter>");
    // old line 2 is the del row; target is the add row with new no 2
    s.assert_row_contains(0, "[cursor L2:C1] README.md");
    s.assert_row_matches(4, "^   2      - hello$");
    s.assert_row_matches(5, r#"^        2 \+▶hello world"#);
    s.assert_row_matches(-1, r#"^\(1-5/5\) hjkl move  enter ask  J/K comments  \? help$"#);
    s.keys(":1<Enter>");
    s.assert_row_contains(0, "[cursor L1:C1] README.md");
    s.assert_row_matches(3, "^   1    1  ▶# Title");
    s.keys("ac1<Enter>");
    s.keys("J");
    s.assert_row_contains(-1, "e edit  D delete");
    s.keys(":3<Enter>");
    // comment unfocused, cursor on L3
    s.assert_matches(r#"^        3 \+▶more"#);
    s.assert_row_contains(0, "[cursor L3:C1] README.md");
    s.assert_row_matches(-1, r#"\(1-\d+/5\) hjkl move  enter ask  J/K comments  \? help$"#);
}

/// F-GOTO-02: changes scope: line not shown jumps to nearest with note (also N > max, no range error)
#[test]
fn f_goto_02_nearest() {
    let mut s = Sim::builder().size(120, 12).args(["--changes-only"]).build();
    s.keys("<Tab>");
    s.assert_row_contains(0, "[changes] [unified] [solarized] [mcp: off] [2/4] [cursor r1:C1] src/big.ts");
    s.assert_row_matches(2, r#"^@@ -27,7 \+27,7 @@"#);
    s.keys(":5<Enter>");
    s.assert_row_contains(0, "[cursor L27:C1] src/big.ts");
    s.assert_row_matches(3, "^  27   27  ▶const v27");
    s.assert_row_matches(-1, r#"^line 5 not in view, nearest L27 \| "#);
    s.keys(":100<Enter>");
    s.assert_row_contains(0, "[cursor L33:C1] src/big.ts");
    s.assert_row_matches(-1, r#"^line 100 not in view, nearest L33 \| "#);
    s.keys(":30<Enter>");
    s.assert_matches(r#"^       30 \+▶const v30 = 2;"#);
    s.assert_row_contains(0, "[cursor L30:C1] src/big.ts");
}

/// F-GOTO-02: goto on a file with no numbered rows: note no lines in view
#[test]
fn f_goto_02_nolines() {
    let mut s = Sim::builder()
        .fixture(Fixture::Empty)
        .size(120, 12)
        .file("a.txt", "hello\n")
        .file("b.bin", "a\0b\n")
        .build();
    s.keys("<Tab>");
    s.assert_row_contains(0, "[2/2] [cursor r1:C1] b.bin");
    s.assert_row_contains(2, "Binary file");
    s.keys(":1<Enter>");
    s.assert_row_contains(0, "[cursor r1:C1] b.bin");
    s.assert_row_matches(-1, r#"^no lines in view \| \(1-1/1\) "#);
}

/// F-GOTO-02: goto targets new-side numbers (unified new no, split right cell) and browse line numbers; jump resets pane new, col 1, selection
#[test]
fn f_goto_02_sides() {
    let mut s =
        Sim::builder().fixture(Fixture::Empty).size(120, 12).file("g.txt", "aa\nbbbb\ncccc\n").build();
    s.git(&["add", "-A"]);
    s.git(&["commit", "-qm", "base"]);
    s.write_file("g.txt", "xx\nyy\nzz\nbbbb\ncccc\n");
    s.keys("r");
    s.assert_row_contains(0, "[unified] [solarized] [mcp: off] [1/1] ");
    s.assert_row_contains(0, "g.txt +3 -1");
    s.assert_row_matches(3, "^   1      -.aa");
    s.assert_row_matches(7, "^   2    4  .bbbb");
    s.keys(":2<Enter>");
    // new line 2 is yy (old line 2 bbbb has new no 4)
    s.assert_row_contains(0, "[cursor L2:C1] g.txt");
    s.assert_row_matches(5, r#"^        2 \+▶yy"#);
    s.keys(":4<Enter>");
    s.assert_row_contains(0, "[cursor L4:C1] g.txt");
    s.assert_row_matches(7, "^   2    4  ▶bbbb");
    s.keys(":1<Enter>");
    // old line 1 (del row aa) ignored; new line 1 is xx
    s.assert_row_contains(0, "[cursor L1:C1] g.txt");
    s.assert_row_matches(3, "^   1      - aa");
    s.assert_row_matches(4, r#"^        1 \+▶xx"#);
    s.keys(":6<Enter>");
    s.assert_row_contains(0, "[cursor L1:C1] g.txt");
    s.assert_row_matches(-1, r#"^line 6 out of range \(1-5\) \| "#);
    s.keys("s");
    s.assert_row_contains(0, "[split] ");
    s.assert_row_matches(3, "aa.*│.*xx");
    s.assert_row_matches(6, "   2 .*bbbb.*│   4 .*bbbb");
    s.keys(":2<Enter>");
    s.assert_row_contains(0, "[cursor new L2:C1] g.txt");
    s.assert_row_matches(4, r#"│   2 \+▶yy"#);
    s.keys(":4<Enter>");
    s.assert_row_contains(0, "[cursor new L4:C1] g.txt");
    s.keys("p$v");
    s.assert_row_contains(0, "[visual old L2:C4] g.txt");
    s.keys(":5<Enter>");
    // pane back to new, col 1, selection cleared
    s.assert_row_contains(0, "[cursor new L5:C1] g.txt");
    s.assert_row_matches(7, "│   5  ▶cccc");
    s.keys("F");
    s.keys("g.txt<Enter>");
    s.assert_row_matches(0, r#"^\[browse\] .*\[cursor L1:C1\] g\.txt"#);
    s.assert_row_matches(-1, r#"\(1-5/5\) hjkl move"#);
    s.keys(":4<Enter>");
    s.assert_row_matches(0, r#"^\[browse\] .*\[cursor L4:C1\] g\.txt"#);
    s.assert_row_matches(5, "^   4  ▶bbbb");
    s.keys(":9<Enter>");
    s.assert_row_matches(0, r#"^\[browse\] .*\[cursor L4:C1\] g\.txt"#);
    s.assert_row_matches(-1, r#"^line 9 out of range \(1-5\) \| "#);
}

/// F-GOTO-02: nearest row: smallest distance, first row on tie
#[test]
fn f_goto_02_tie() {
    let mut s = Sim::builder()
        .fixture(Fixture::Empty)
        .size(80, 24)
        .args(["--changes-only"])
        .file("t.txt", "l1\nl2\nl3\nl4\nl5\nl6\nl7\nl8\nl9\nl10\nl11\nl12\nl13\nl14\nl15\nl16\nl17\nl18\nl19\nl20\nl21\nl22\nl23\nl24\nl25\nl26\nl27\nl28\nl29\nl30\n")
        .build();
    s.git(&["add", "-A"]);
    s.git(&["commit", "-qm", "base"]);
    s.write_file("t.txt", "L1\nl2\nl3\nl4\nl5\nl6\nl7\nl8\nl9\nl10\nl11\nl12\nl13\nl14\nl15\nl16\nl17\nl18\nl19\nl20\nl21\nl22\nl23\nl24\nl25\nl26\nl27\nl28\nL29\nl30\n");
    s.keys("r");
    s.assert_contains("@@ -1,4 +1,4 @@");
    s.assert_contains("@@ -26,5 +26,5 @@");
    s.keys(":15<Enter>");
    // L4 and L26 both 11 away; first row wins
    s.assert_row_contains(0, "[cursor L4:C1] t.txt");
    s.assert_row_matches(-1, r#"^line 15 not in view, nearest L4 \| "#);
    s.keys(":16<Enter>");
    s.assert_row_contains(0, "[cursor L26:C1] t.txt");
    s.assert_row_matches(-1, r#"^line 16 not in view, nearest L26 \| "#);
    s.keys(":14<Enter>");
    s.assert_row_contains(0, "[cursor L4:C1] t.txt");
    s.assert_row_matches(-1, r#"^line 14 not in view, nearest L4 \| "#);
}

/// F-GOTO-02: goto jump: col 1, selection cleared, viewport follows cursor (margin 2, clamped to bottom)
#[test]
fn f_goto_02_viewport() {
    let mut s = Sim::builder().size(80, 12).build();
    s.keys("<Tab>");
    s.assert_row_contains(0, "[2/4] [cursor L30:C1] src/big.ts");
    s.assert_row_matches(-1, r#"^\(28-36/62\) hjkl move"#);
    s.keys(":1<Enter>");
    s.assert_row_contains(0, "[cursor L1:C1] src/big.ts");
    s.assert_row_matches(-1, r#"^\(1-9/62\) hjkl move"#);
    s.keys(":60<Enter>");
    s.assert_row_contains(0, "[cursor L60:C1] src/big.ts");
    s.assert_row_matches(-1, r#"^\(54-62/62\) hjkl move"#);
    s.keys("$v");
    s.assert_row_contains(0, "[visual L60:C14] src/big.ts");
    s.keys(":45<Enter>");
    s.assert_row_contains(0, "[cursor L45:C1] src/big.ts");
    s.assert_row_matches(-1, r#"^\(45-53/62\) hjkl move  enter ask"#);
    s.assert_row_matches(4, "^  45   45  ▶const v45 = 1;");
}
