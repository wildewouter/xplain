//! Scenario tests: cursor.

use serde_json::json;
use xplain_sim::{CellExpect as C, Fixture, Sim};

/// F-CURSOR-02: split cursor row - both panes curBg, ▶ in non-empty panes, char cursor only in active pane, empty active cell ▶ + reverse space
/// f.txt base "a\nb\nc\n", work "a\nB\nc\nd\n". rows: 1 hunk, 2 a|a, 3 b|B, 4 c|c, 5 (empty)|d.
/// 120 cols: pane W=59; left: num 0-3, mark 5, cursor 6, code 7; separator 59; right: num 60-63, mark 65, cursor 66, code 67.
#[test]
fn f_cursor_02_split() {
    let mut s = Sim::builder().fixture(Fixture::Empty).size(120, 12).args(["--split"]).build();
    s.write_file("f.txt", "a\nb\nc\n");
    s.git(&["add", "-A"]);
    s.git(&["commit", "-qm", "base"]);
    s.write_file("f.txt", "a\nB\nc\nd\n");
    s.keys("r");
    s.assert_row_matches(0, r##"\[split\] .*\[cursor new L2:C1\] f\.txt "##);
    s.assert_row_matches(3, r##"^   1   a +│   1   a$"##);
    s.assert_row_matches(4, r##"^   2 -▶b.*│   2 \+▶B"##);
    s.assert_row_matches(6, r##"^ +│   4 \+ d$"##);
    // both panes of cursor row get curBg (left, separator side, right, padding)
    s.assert_cell(0, 4, C::new().bg("#22586b"));
    s.assert_cell(5, 4, C::new().ch('-').fg("#dc322f").bg("#22586b"));
    s.assert_cell(30, 4, C::new().bg("#22586b"));
    s.assert_cell(60, 4, C::new().bg("#22586b"));
    s.assert_cell(65, 4, C::new().ch('+').fg("#859900").bg("#22586b"));
    s.assert_cell(90, 4, C::new().bg("#22586b"));
    // char cursor only in active (new) pane
    s.assert_cell(67, 4, C::new().ch('B').reverse(true));
    s.assert_cell(7, 4, C::new().ch('b').reverse(false));
    s.assert_cell(0, 3, C::new().bg("default"));
    s.keys("p");
    // pane old - char cursor moves to left pane, ▶ still in both
    s.assert_row_contains(0, "[cursor old L2:C1] f.txt");
    s.assert_row_matches(4, r##"^   2 -▶b.*│   2 \+▶B"##);
    s.assert_cell(7, 4, C::new().ch('b').reverse(true));
    s.assert_cell(67, 4, C::new().ch('B').reverse(false));
    s.keys("jj");
    // add-only row, pane old - empty active cell shows ▶ + reverse space; right pane ▶ without char cursor
    s.assert_row_contains(0, "[cursor old r5:C1] f.txt");
    s.assert_row_matches(6, r##"^      ▶.*│   4 \+▶d"##);
    s.assert_row_matches(4, r##"^   2 - b +│   2 \+ B$"##);
    s.assert_cell(7, 6, C::new().ch(' ').bg("#22586b").reverse(true));
    s.assert_cell(67, 6, C::new().ch('d').bg("#22586b").reverse(false));
    s.assert_cell(0, 6, C::new().bg("#22586b"));
    s.keys("p");
    // pane new on add-only row - char cursor on d; empty left pane has no ▶
    s.assert_row_contains(0, "[cursor new L4:C1] f.txt");
    s.assert_row_matches(6, r##"^ +(…)?│   4 \+▶d"##);
    s.assert_cell(67, 6, C::new().ch('d').reverse(true));
    s.assert_cell(6, 6, C::new().ch(' ').bg("#22586b"));
    s.assert_cell(7, 6, C::new().bg("#22586b").reverse(false));
}

/// F-CURSOR-02: unified cursor row - curBg full width, ▶ cell, mark kept, reverse char cursor, empty line reverse space, hunk row bg only
/// e.txt untracked -> fully added. rows: 1 hunk, 2 L1 "abc", 3 L2 "", 4 L3 "\tx" (shown "  x").
#[test]
fn f_cursor_02_unified() {
    let mut s = Sim::builder().fixture(Fixture::Empty).size(80, 12).file("e.txt", "abc\n\n\tx\n").build();
    s.assert_row_contains(0, "[cursor L1:C1] e.txt");
    s.assert_row_matches(2, r##"^@@ -0,0 \+1,3 @@"##);
    s.assert_row_matches(3, r##"^        1 \+▶abc"##);
    s.assert_row_matches(4, r##"^        2 \+ ?$"##);
    s.assert_row_matches(5, r##"^        3 \+   x$"##);
    // cursor row bg curBg on number cells, mark cell, cursor cell, code, padding
    s.assert_cell(0, 3, C::new().bg("#22586b"));
    s.assert_cell(8, 3, C::new().ch('1').bg("#22586b"));
    s.assert_cell(10, 3, C::new().ch('+').fg("#859900").bg("#22586b"));
    s.assert_cell(11, 3, C::new().ch('▶').bg("#22586b"));
    s.assert_cell(12, 3, C::new().ch('a').bg("#22586b").reverse(true));
    s.assert_cell(13, 3, C::new().ch('b').bg("#22586b").reverse(false));
    s.assert_cell(40, 3, C::new().bg("#22586b"));
    // other add rows keep addBg gutter, no cursor bg on code
    s.assert_cell(0, 5, C::new().bg("#0b3b1f"));
    s.assert_cell(11, 5, C::new().ch(' '));
    s.assert_cell(14, 5, C::new().ch('x').bg("default").reverse(false));
    s.keys("2l");
    s.assert_row_contains(0, "[cursor L1:C3] e.txt");
    s.assert_cell(12, 3, C::new().ch('a').reverse(false));
    s.assert_cell(14, 3, C::new().ch('c').reverse(true));
    s.keys("j");
    // empty line - reverse space at code start; mark still shown
    s.assert_row_contains(0, "[cursor L2:C1] e.txt");
    s.assert_row_matches(3, r##"^        1 \+ abc$"##);
    s.assert_row_matches(4, r##"^        2 \+▶"##);
    s.assert_cell(10, 4, C::new().ch('+').bg("#22586b"));
    s.assert_cell(12, 4, C::new().ch(' ').bg("#22586b").reverse(true));
    s.assert_cell(0, 4, C::new().bg("#22586b"));
    s.assert_cell(0, 3, C::new().bg("#0b3b1f"));
    s.assert_cell(12, 3, C::new().bg("default").reverse(false));
    s.keys("kk");
    // hunk row - row bg only, no ▶, no char cursor; column still tracked in tag (C3)
    s.assert_row_contains(0, "[cursor r1:C3] e.txt");
    s.assert_row_matches(2, r##"^@@ -0,0 \+1,3 @@"##);
    s.assert_row_matches(3, r##"^        1 \+ abc$"##);
    s.assert_not_contains("▶");
    s.assert_cell(0, 2, C::new().ch('@').bg("#22586b").reverse(false));
    s.assert_cell(2, 2, C::new().bg("#22586b").reverse(false));
    s.assert_cell(40, 2, C::new().bg("#22586b"));
    s.assert_cell(0, 3, C::new().bg("#0b3b1f"));
}

/// F-CURSOR-03: desired column kept across rows, shown clamped to row length-1 (empty row C1)
/// c.txt untracked: L1 "abcdefgh", L2 "ab", L3 "", L4 "abcdefgh".
#[test]
fn f_cursor_03_column() {
    let mut s = Sim::builder()
        .fixture(Fixture::Empty)
        .size(80, 12)
        .file("c.txt", "abcdefgh\nab\n\nabcdefgh\n")
        .build();
    s.assert_row_contains(0, "[cursor L1:C1] c.txt");
    s.keys("5l");
    s.assert_row_contains(0, "[cursor L1:C6] c.txt");
    s.assert_cell(17, 3, C::new().ch('f').reverse(true));
    s.keys("j");
    // shown clamped to last char
    s.assert_row_contains(0, "[cursor L2:C2] c.txt");
    s.assert_cell(13, 4, C::new().ch('b').reverse(true));
    s.keys("j");
    s.assert_row_contains(0, "[cursor L3:C1] c.txt");
    s.assert_cell(12, 5, C::new().ch(' ').reverse(true));
    s.keys("j");
    // desired column restored on longer row
    s.assert_row_contains(0, "[cursor L4:C6] c.txt");
    s.assert_cell(17, 6, C::new().ch('f').reverse(true));
    s.keys("2k");
    s.assert_row_contains(0, "[cursor L2:C2] c.txt");
    s.keys("gj");
    // g is a vertical move too - desired column kept
    s.assert_row_contains(0, "[cursor L1:C6] c.txt");
    s.keys("G");
    s.assert_row_contains(0, "[cursor L4:C6] c.txt");
}

/// F-CURSOR-03: g first row (count ignored), G last row, nG row n over all rows incl hunk row, clamped
/// n.txt untracked (60 lines): row 1 hunk, row k+1 = L<k>.
#[test]
fn f_cursor_03_g() {
    let mut s = Sim::builder().fixture(Fixture::Empty).size(80, 16).file("n.txt", "line 1\nline 2\nline 3\nline 4\nline 5\nline 6\nline 7\nline 8\nline 9\nline 10\nline 11\nline 12\nline 13\nline 14\nline 15\nline 16\nline 17\nline 18\nline 19\nline 20\nline 21\nline 22\nline 23\nline 24\nline 25\nline 26\nline 27\nline 28\nline 29\nline 30\nline 31\nline 32\nline 33\nline 34\nline 35\nline 36\nline 37\nline 38\nline 39\nline 40\nline 41\nline 42\nline 43\nline 44\nline 45\nline 46\nline 47\nline 48\nline 49\nline 50\nline 51\nline 52\nline 53\nline 54\nline 55\nline 56\nline 57\nline 58\nline 59\nline 60\n").build();
    s.assert_row_contains(0, "[cursor L1:C1] n.txt");
    s.keys("G");
    s.assert_row_contains(0, "[cursor L60:C1] n.txt");
    s.assert_row_matches(-2, r##"^       60 \+▶line 60\b"##);
    s.keys("g");
    s.assert_row_contains(0, "[cursor r1:C1] n.txt");
    s.assert_row_matches(2, r##"^@@ -0,0 \+1,60 @@"##);
    s.keys("5G");
    // row 5 = hunk row + 4 -> L4
    s.assert_row_contains(0, "[cursor L4:C1] n.txt");
    s.assert_row_matches(6, r##"^        4 \+▶line 4\b"##);
    s.keys("20j5g");
    // count before g ignored -> row 1
    s.assert_row_contains(0, "[cursor r1:C1] n.txt");
    s.keys("1G");
    s.assert_row_contains(0, "[cursor r1:C1] n.txt");
    s.keys("61G");
    s.assert_row_contains(0, "[cursor L60:C1] n.txt");
    s.keys("30G");
    s.assert_row_contains(0, "[cursor L29:C1] n.txt");
    s.keys("500G");
    // clamped to last row
    s.assert_row_contains(0, "[cursor L60:C1] n.txt");
    s.keys("2G");
    s.assert_row_contains(0, "[cursor L1:C1] n.txt");
    s.keys("G");
    // plain G after a counted G - count not reused
    s.assert_row_contains(0, "[cursor L60:C1] n.txt");
}

/// F-CURSOR-03: j/k/arrows, d/u half page, Space/PageDown/PageUp (H-1), counts, clamped at both ends
/// n.txt untracked (60 lines, fully added): row 1 hunk, row k+1 = L<k>. 80x16: H=13, d/u = 6 rows, page = 12 rows.
#[test]
fn f_cursor_03_moves() {
    let mut s = Sim::builder().fixture(Fixture::Empty).size(80, 16).file("n.txt", "line 1\nline 2\nline 3\nline 4\nline 5\nline 6\nline 7\nline 8\nline 9\nline 10\nline 11\nline 12\nline 13\nline 14\nline 15\nline 16\nline 17\nline 18\nline 19\nline 20\nline 21\nline 22\nline 23\nline 24\nline 25\nline 26\nline 27\nline 28\nline 29\nline 30\nline 31\nline 32\nline 33\nline 34\nline 35\nline 36\nline 37\nline 38\nline 39\nline 40\nline 41\nline 42\nline 43\nline 44\nline 45\nline 46\nline 47\nline 48\nline 49\nline 50\nline 51\nline 52\nline 53\nline 54\nline 55\nline 56\nline 57\nline 58\nline 59\nline 60\n").build();
    s.assert_row_contains(0, "[cursor L1:C1] n.txt");
    s.assert_row_matches(3, r##"^        1 \+▶line 1\b"##);
    s.keys("j");
    s.assert_row_contains(0, "[cursor L2:C1] n.txt");
    s.assert_row_matches(4, r##"^        2 \+▶line 2\b"##);
    s.keys("<Down>");
    s.assert_row_contains(0, "[cursor L3:C1] n.txt");
    s.keys("k");
    s.assert_row_contains(0, "[cursor L2:C1] n.txt");
    s.keys("<Up>");
    s.assert_row_contains(0, "[cursor L1:C1] n.txt");
    s.keys("3j");
    s.assert_row_contains(0, "[cursor L4:C1] n.txt");
    s.assert_row_matches(6, r##"^        4 \+▶line 4\b"##);
    s.keys("2<Down>");
    s.assert_row_contains(0, "[cursor L6:C1] n.txt");
    s.keys("2k");
    s.assert_row_contains(0, "[cursor L4:C1] n.txt");
    s.keys("2<Up>");
    s.assert_row_contains(0, "[cursor L2:C1] n.txt");
    s.keys("d");
    // floor(13/2) = 6 rows
    s.assert_row_contains(0, "[cursor L8:C1] n.txt");
    s.keys("2d");
    s.assert_row_contains(0, "[cursor L20:C1] n.txt");
    s.keys("u");
    s.assert_row_contains(0, "[cursor L14:C1] n.txt");
    s.keys("2u");
    s.assert_row_contains(0, "[cursor L2:C1] n.txt");
    s.keys("<Space>");
    // H-1 = 12 rows
    s.assert_row_contains(0, "[cursor L14:C1] n.txt");
    s.keys("<PageDown>");
    s.assert_row_contains(0, "[cursor L26:C1] n.txt");
    s.keys("<PageUp>");
    s.assert_row_contains(0, "[cursor L14:C1] n.txt");
    s.keys("2<PageDown>");
    s.assert_row_contains(0, "[cursor L38:C1] n.txt");
    s.keys("2<PageUp>");
    s.assert_row_contains(0, "[cursor L14:C1] n.txt");
    s.keys("3<Space>");
    s.assert_row_contains(0, "[cursor L50:C1] n.txt");
    s.keys("3<PageDown>");
    // clamped to last row
    s.assert_row_contains(0, "[cursor L60:C1] n.txt");
    s.keys("j<Down>d<Space>");
    s.assert_row_contains(0, "[cursor L60:C1] n.txt");
    s.assert_row_matches(-1, r##"^\(49-61/61\) "##);
    s.keys("100k");
    // clamped to first row (hunk row)
    s.assert_row_contains(0, "[cursor r1:C1] n.txt");
    s.assert_row_matches(-1, r##"^\(1-13/61\) "##);
    s.keys("k<Up>u<PageUp>");
    s.assert_row_contains(0, "[cursor r1:C1] n.txt");
    s.keys("3u");
    s.assert_row_contains(0, "[cursor r1:C1] n.txt");
    s.keys("99999j");
    s.assert_row_contains(0, "[cursor L60:C1] n.txt");
}

/// F-CURSOR-04: h/l/Left/Right with count, clamped to 0..len-1; Left/Right never switch file
/// h.txt untracked: L1 "abcdef". Second file z.txt so a file switch would show.
#[test]
fn f_cursor_04_hl() {
    let mut s = Sim::builder()
        .fixture(Fixture::Empty)
        .size(80, 12)
        .file("h.txt", "abcdef\n")
        .file("z.txt", "zzz\n")
        .build();
    s.assert_row_contains(0, "[1/2] [cursor L1:C1] h.txt");
    s.keys("l");
    s.assert_row_contains(0, "[cursor L1:C2] h.txt");
    s.assert_cell(13, 3, C::new().ch('b').reverse(true));
    s.assert_cell(12, 3, C::new().ch('a').reverse(false));
    s.keys("<Right>");
    s.assert_row_contains(0, "[1/2] [cursor L1:C3] h.txt");
    s.keys("h");
    s.assert_row_contains(0, "[cursor L1:C2] h.txt");
    s.keys("<Left>");
    s.assert_row_contains(0, "[1/2] [cursor L1:C1] h.txt");
    s.keys("h<Left>");
    // clamped at col 1, no file switch
    s.assert_row_contains(0, "[1/2] [cursor L1:C1] h.txt");
    s.assert_cell(12, 3, C::new().ch('a').reverse(true));
    s.keys("3l");
    s.assert_row_contains(0, "[cursor L1:C4] h.txt");
    s.assert_cell(15, 3, C::new().ch('d').reverse(true));
    s.keys("10l");
    // clamped to last char
    s.assert_row_contains(0, "[cursor L1:C6] h.txt");
    s.assert_cell(17, 3, C::new().ch('f').reverse(true));
    s.keys("l<Right>");
    s.assert_row_contains(0, "[1/2] [cursor L1:C6] h.txt");
    s.keys("2<Left>");
    s.assert_row_contains(0, "[cursor L1:C4] h.txt");
    s.keys("2<Right>");
    s.assert_row_contains(0, "[cursor L1:C6] h.txt");
    s.keys("2h");
    s.assert_row_contains(0, "[cursor L1:C4] h.txt");
    s.keys("99h");
    s.assert_row_contains(0, "[1/2] [cursor L1:C1] h.txt");
}

/// F-CURSOR-04: 0 col 1, ^ first non-blank (all blank: col 1), $ last char and sticky on vertical moves until horizontal move; tabs = 2 cols
/// s.txt untracked: L1 "   xyz tail" (11), L2 "ab" , L3 "    " (blank), L4 "\tqr" (shown "  qr"), L5 "0123456789abcd" (14).
#[test]
fn f_cursor_04_line_ends() {
    let mut s = Sim::builder()
        .fixture(Fixture::Empty)
        .size(80, 12)
        .file("s.txt", "   xyz tail\nab\n    \n\tqr\n0123456789abcd\n")
        .build();
    s.assert_row_contains(0, "[cursor L1:C1] s.txt");
    s.assert_row_matches(6, r##"^        4 \+   qr$"##);
    s.keys("^");
    s.assert_row_contains(0, "[cursor L1:C4] s.txt");
    s.assert_cell(15, 3, C::new().ch('x').reverse(true));
    s.keys("$");
    s.assert_row_contains(0, "[cursor L1:C11] s.txt");
    s.assert_cell(22, 3, C::new().ch('l').reverse(true));
    s.keys("0");
    s.assert_row_contains(0, "[cursor L1:C1] s.txt");
    s.assert_cell(12, 3, C::new().ch(' ').reverse(true));
    s.keys("j$");
    s.assert_row_contains(0, "[cursor L2:C2] s.txt");
    s.keys("j");
    // blank row, $ sticky -> last char of blank row
    s.assert_row_contains(0, "[cursor L3:C4] s.txt");
    s.keys("^");
    // all blank -> col 1
    s.assert_row_contains(0, "[cursor L3:C1] s.txt");
    s.keys("$j");
    // tab expanded to 2 spaces -> "  qr" last char C4
    s.assert_row_contains(0, "[cursor L4:C4] s.txt");
    s.assert_cell(15, 6, C::new().ch('r').reverse(true));
    s.keys("j");
    // $ sticky - last char of longer row
    s.assert_row_contains(0, "[cursor L5:C14] s.txt");
    s.assert_cell(25, 7, C::new().ch('d').reverse(true));
    s.keys("4k");
    s.assert_row_contains(0, "[cursor L1:C11] s.txt");
    s.keys("kj");
    // through the hunk row and back, still sticky
    s.assert_row_contains(0, "[cursor L1:C11] s.txt");
    s.keys("3j");
    s.assert_row_contains(0, "[cursor L4:C4] s.txt");
    s.keys("^");
    // ^ on tab row -> first non-blank after the 2 tab columns
    s.assert_row_contains(0, "[cursor L4:C3] s.txt");
    s.assert_cell(14, 6, C::new().ch('q').reverse(true));
    s.keys("j");
    // horizontal move ended sticky $; column 3 kept
    s.assert_row_contains(0, "[cursor L5:C3] s.txt");
    s.keys("$h");
    s.assert_row_contains(0, "[cursor L5:C13] s.txt");
    s.keys("4k");
    // after h the desired column is 13, not end of line
    s.assert_row_contains(0, "[cursor L1:C11] s.txt");
    s.keys("4j");
    s.assert_row_contains(0, "[cursor L5:C13] s.txt");
}

/// F-CURSOR-04: motions use the cursor pane text: split old vs new pane; browse (no hunk row) b stops at row 1 col 1
/// f.txt base "aa bb\n", work "cccc dd\n". Split 120 cols: left code col 7, right code col 67.
#[test]
fn f_cursor_04_pane_text() {
    let mut s = Sim::builder().fixture(Fixture::Empty).size(120, 12).args(["--split"]).build();
    s.write_file("f.txt", "aa bb\n");
    s.git(&["add", "-A"]);
    s.git(&["commit", "-qm", "base"]);
    s.write_file("f.txt", "cccc dd\n");
    s.keys("r");
    s.assert_row_contains(0, "[cursor new L1:C1] f.txt");
    s.keys("w");
    // new pane text "cccc dd"
    s.assert_row_contains(0, "[cursor new L1:C6] f.txt");
    s.assert_cell(72, 3, C::new().ch('d').reverse(true));
    s.keys("$");
    s.assert_row_contains(0, "[cursor new L1:C7] f.txt");
    s.keys("p0w");
    // old pane text "aa bb"
    s.assert_row_contains(0, "[cursor old L1:C4] f.txt");
    s.assert_cell(10, 3, C::new().ch('b').reverse(true));
    s.keys("$");
    s.assert_row_contains(0, "[cursor old L1:C5] f.txt");
    s.keys("Ff.txt<Enter>");
    // browse - rows are file lines only
    s.assert_row_matches(0, r##"^\[browse\] .*\[cursor L1:C1\] f\.txt"##);
    s.assert_row_matches(2, r##"^   1  ▶cccc dd"##);
    s.keys("w");
    s.assert_row_contains(0, "[cursor L1:C6] f.txt");
    s.assert_cell(12, 2, C::new().ch('d').reverse(true));
    s.keys("b");
    s.assert_row_contains(0, "[cursor L1:C1] f.txt");
    s.keys("b");
    // row 1 col 1 - b stays
    s.assert_row_contains(0, "[cursor L1:C1] f.txt");
    s.assert_cell(7, 2, C::new().ch('c').reverse(true));
    s.keys("e");
    s.assert_row_contains(0, "[cursor L1:C4] f.txt");
}

/// F-CURSOR-04: w/b/e vim word motions across rows; w stops at empty row and stays at last char of last row; hunk row text is header text
/// w.txt untracked: L1 "foo bar.baz  qux", L2 "", L3 "  end_x+yy". rows: 1 hunk "@@ -0,0 +1,3 @@", 2 L1, 3 L2, 4 L3.
#[test]
fn f_cursor_04_words() {
    let mut s = Sim::builder()
        .fixture(Fixture::Empty)
        .size(80, 12)
        .file("w.txt", "foo bar.baz  qux\n\n  end_x+yy\n")
        .build();
    s.assert_row_contains(0, "[cursor L1:C1] w.txt");
    s.keys("w");
    s.assert_row_contains(0, "[cursor L1:C5] w.txt");
    s.assert_cell(16, 3, C::new().ch('b').reverse(true));
    s.keys("w");
    // '.' is its own class (other)
    s.assert_row_contains(0, "[cursor L1:C8] w.txt");
    s.keys("w");
    s.assert_row_contains(0, "[cursor L1:C9] w.txt");
    s.keys("w");
    s.assert_row_contains(0, "[cursor L1:C14] w.txt");
    s.keys("w");
    // crosses rows, stops at the empty row
    s.assert_row_contains(0, "[cursor L2:C1] w.txt");
    s.keys("w");
    s.assert_row_contains(0, "[cursor L3:C3] w.txt");
    s.keys("w");
    s.assert_row_contains(0, "[cursor L3:C8] w.txt");
    s.keys("w");
    s.assert_row_contains(0, "[cursor L3:C9] w.txt");
    s.keys("w");
    // no next word - last char of last row
    s.assert_row_contains(0, "[cursor L3:C10] w.txt");
    s.assert_cell(21, 5, C::new().ch('y').reverse(true));
    s.keys("w");
    s.assert_row_contains(0, "[cursor L3:C10] w.txt");
    s.keys("b");
    s.assert_row_contains(0, "[cursor L3:C9] w.txt");
    s.keys("b");
    s.assert_row_contains(0, "[cursor L3:C8] w.txt");
    s.keys("b");
    s.assert_row_contains(0, "[cursor L3:C3] w.txt");
    s.keys("b");
    // back across rows, stops at the empty row
    s.assert_row_contains(0, "[cursor L2:C1] w.txt");
    s.keys("b");
    s.assert_row_contains(0, "[cursor L1:C14] w.txt");
    s.keys("2b");
    s.assert_row_contains(0, "[cursor L1:C8] w.txt");
    s.keys("2b");
    s.assert_row_contains(0, "[cursor L1:C1] w.txt");
    s.keys("b");
    // hunk row text "@@ -0,0 +1,3 @@" is used; last word start "@@" at C14
    s.assert_row_contains(0, "[cursor r1:C14] w.txt");
    s.keys("0");
    s.assert_row_contains(0, "[cursor r1:C1] w.txt");
    s.keys("b");
    // row 1 col 1 - b stays
    s.assert_row_contains(0, "[cursor r1:C1] w.txt");
    s.keys("w");
    // hunk text words - "-" at C4
    s.assert_row_contains(0, "[cursor r1:C4] w.txt");
    s.keys("j0");
    s.assert_row_contains(0, "[cursor L1:C1] w.txt");
    s.keys("e");
    s.assert_row_contains(0, "[cursor L1:C3] w.txt");
    s.keys("e");
    s.assert_row_contains(0, "[cursor L1:C7] w.txt");
    s.keys("e");
    s.assert_row_contains(0, "[cursor L1:C8] w.txt");
    s.keys("2e");
    s.assert_row_contains(0, "[cursor L1:C16] w.txt");
    s.keys("e");
    // e crosses rows (skips the empty row) to end of "end_x"
    s.assert_row_contains(0, "[cursor L3:C7] w.txt");
    s.assert_cell(18, 5, C::new().ch('x').reverse(true));
    s.keys("e");
    s.assert_row_contains(0, "[cursor L3:C8] w.txt");
    s.keys("e");
    s.assert_row_contains(0, "[cursor L3:C10] w.txt");
    s.keys("gj03w");
    s.assert_row_contains(0, "[cursor L1:C9] w.txt");
}

/// F-CURSOR-05: count works in browse; digits while comment focused keep focus, next motion unfocuses and uses count
#[test]
fn f_cursor_05_browse_focused() {
    let mut s = Sim::builder().fixture(Fixture::Empty).size(120, 16).file("n.txt", "line 01 abc\nline 02 abc\nline 03 abc\nline 04 abc\nline 05 abc\nline 06 abc\nline 07 abc\nline 08 abc\nline 09 abc\nline 10 abc\nline 11 abc\nline 12 abc\nline 13 abc\nline 14 abc\nline 15 abc\nline 16 abc\nline 17 abc\nline 18 abc\nline 19 abc\nline 20 abc\nline 21 abc\nline 22 abc\nline 23 abc\nline 24 abc\nline 25 abc\nline 26 abc\nline 27 abc\nline 28 abc\nline 29 abc\nline 30 abc\n").build();
    s.assert_row_contains(0, "[cursor L1:C1] n.txt");
    s.keys("ac1<Enter>");
    s.assert_contains("line L1");
    s.keys("J");
    s.assert_row_contains(-1, "e edit  D delete");
    s.keys("3");
    // focus kept while count pending
    s.assert_row_matches(-1, r##"e edit  D delete  a ask/follow up  j/k scroll  esc back  \? help$"##);
    s.assert_row_contains(0, "[cursor L1:C1] n.txt");
    s.keys("j");
    // motion unfocuses and moves 3 rows
    s.assert_row_contains(0, "[cursor L4:C1] n.txt");
    s.assert_row_matches(-1, r##"hjkl move  enter ask  J/K comments  \? help$"##);
    s.keys("Fn.txt<Enter>");
    s.assert_row_matches(0, r##"^\[browse\] .*\[cursor L1:C1\] n\.txt"##);
    s.keys("4j");
    s.assert_row_matches(0, r##"^\[browse\] .*\[cursor L5:C1\] n\.txt"##);
    s.assert_matches(r##"^   5  ▶line 05 abc"##);
    s.keys("2w");
    s.assert_row_matches(0, r##"^\[browse\] .*\[cursor L5:C9\] n\.txt"##);
    s.keys("12G");
    // browse has no hunk row - row 12 = L12
    s.assert_row_matches(0, r##"^\[browse\] .*\[cursor L12:C9\] n\.txt"##);
    s.keys("3k");
    s.assert_row_matches(0, r##"^\[browse\] .*\[cursor L9:C9\] n\.txt"##);
}

/// F-CURSOR-05: count cleared by ? r M F f C E n N q, Ctrl combos and keys ignored in browse: 3?j, 3rj, 3M<Esc>j, 3<C-x>j move 1 row
/// n.txt untracked, 30 lines: row 1 hunk, row k+1 = L<k>. Each group: count 3, the key, then j (must move 1 row).
#[test]
fn f_cursor_05_cleared_global() {
    let mut s = Sim::builder().fixture(Fixture::Empty).size(80, 16).file("n.txt", "line 01 abc\nline 02 abc\nline 03 abc\nline 04 abc\nline 05 abc\nline 06 abc\nline 07 abc\nline 08 abc\nline 09 abc\nline 10 abc\nline 11 abc\nline 12 abc\nline 13 abc\nline 14 abc\nline 15 abc\nline 16 abc\nline 17 abc\nline 18 abc\nline 19 abc\nline 20 abc\nline 21 abc\nline 22 abc\nline 23 abc\nline 24 abc\nline 25 abc\nline 26 abc\nline 27 abc\nline 28 abc\nline 29 abc\nline 30 abc\n").build();
    s.assert_row_contains(0, "[cursor L1:C1] n.txt");
    s.keys("3?j");
    // help panel opened by ?, j passes through and moves 1 row
    s.assert_contains(" Help · Diff view");
    s.assert_row_contains(0, "[cursor L2:C1] n.txt");
    s.keys("??");
    s.assert_not_contains(" Help · ");
    s.keys("3rj");
    s.assert_row_contains(0, "[cursor L3:C1] n.txt");
    s.assert_row_matches(-1, r##"^reloaded \| "##);
    s.keys("3M<Esc>j");
    s.assert_row_contains(0, "[cursor L4:C1] n.txt");
    s.keys("3<C-x>j");
    s.assert_row_contains(0, "[cursor L5:C1] n.txt");
    s.keys("3F<Esc>j");
    s.assert_row_contains(0, "[cursor L6:C1] n.txt");
    s.keys("3f<Esc>j");
    s.assert_row_contains(0, "[cursor L7:C1] n.txt");
    s.keys("3C<Esc>j");
    s.assert_row_contains(0, "[cursor L8:C1] n.txt");
    s.keys("3Ej");
    s.assert_row_contains(0, "[cursor L9:C1] n.txt");
    s.assert_row_matches(-1, r##"^no comments to export \| "##);
    // n / N without a find term
    s.keys("3nj");
    s.assert_row_contains(0, "[cursor L10:C1] n.txt");
    s.keys("3Nj");
    s.assert_row_contains(0, "[cursor L11:C1] n.txt");
    // q opens the quit dialog, n closes it
    s.keys("3qnj");
    s.assert_row_contains(0, "[cursor L12:C1] n.txt");
    // find term abc matches every line; Enter stays on L12, column on the hit (C9)
    s.keys("/abc<Enter>");
    s.assert_row_contains(0, "[cursor L12:C9] n.txt");
    // n jumps to the next match (count ignored), then j moves 1 row
    s.keys("3nj");
    s.assert_row_contains(0, "[cursor L14:C9] n.txt");
    s.keys("3Nj");
    s.assert_row_contains(0, "[cursor L14:C9] n.txt");
    s.keys("Fn.txt<Enter>");
    s.assert_row(0, "[browse] [solarized] [mcp: off] [cursor L1:C1] n.txt");
    // keys ignored in browse (s c m f p Tab S-Tab) clear the count
    s.keys("3sj");
    s.assert_row_contains(0, "[cursor L2:C1] n.txt");
    s.keys("3cj");
    s.assert_row_contains(0, "[cursor L3:C1] n.txt");
    s.keys("3mj");
    s.assert_row_contains(0, "[cursor L4:C1] n.txt");
    s.keys("3fj");
    s.assert_row_contains(0, "[cursor L5:C1] n.txt");
    s.keys("3pj");
    s.assert_row_contains(0, "[cursor L6:C1] n.txt");
    s.keys("3<Tab>j");
    s.assert_row_contains(0, "[cursor L7:C1] n.txt");
    s.keys("3<S-Tab>j");
    s.assert_row_contains(0, "[cursor L8:C1] n.txt");
    s.keys("3<C-x>j");
    s.assert_row_contains(0, "[cursor L9:C1] n.txt");
    // count still works after all that
    s.keys("3j");
    s.assert_row_contains(0, "[cursor L12:C1] n.txt");
}

/// F-CURSOR-05: count cleared by keys that ignore it (i ] [ Esc t J K), by / : ( ), editor open, file change
/// n.txt untracked, 30 lines: row 1 hunk, row k+1 = L<k>. m.txt second file.
#[test]
fn f_cursor_05_cleared() {
    let mut s = Sim::builder().fixture(Fixture::Empty).size(80, 16).file("n.txt", "line 01 abc\nline 02 abc\nline 03 abc\nline 04 abc\nline 05 abc\nline 06 abc\nline 07 abc\nline 08 abc\nline 09 abc\nline 10 abc\nline 11 abc\nline 12 abc\nline 13 abc\nline 14 abc\nline 15 abc\nline 16 abc\nline 17 abc\nline 18 abc\nline 19 abc\nline 20 abc\nline 21 abc\nline 22 abc\nline 23 abc\nline 24 abc\nline 25 abc\nline 26 abc\nline 27 abc\nline 28 abc\nline 29 abc\nline 30 abc\n").file("m.txt", "m1\nm2\nm3\nm4\nm5\nm6\n").build();
    s.assert_row_contains(0, "[1/2] [cursor L1:C1] m.txt");
    s.keys("<Tab>");
    s.assert_row_contains(0, "[2/2] [cursor L1:C1] n.txt");
    s.keys("3ij");
    s.assert_row_contains(0, "[cursor L2:C1] n.txt");
    s.keys("3<Esc>j");
    s.assert_row_contains(0, "[cursor L3:C1] n.txt");
    s.keys("3]j");
    // ] finds no later change start (no move), count dropped
    s.assert_row_contains(0, "[cursor L4:C1] n.txt");
    s.keys("3[");
    // [ jumps to the only change start (L1), count ignored
    s.assert_row_contains(0, "[cursor L1:C1] n.txt");
    s.keys("j");
    s.assert_row_contains(0, "[cursor L2:C1] n.txt");
    s.keys("3t");
    s.assert_row_contains(0, "[vibrant]");
    s.keys("j");
    s.assert_row_contains(0, "[cursor L3:C1] n.txt");
    s.keys("3J");
    s.assert_row_contains(-1, "no comments");
    s.keys("j");
    s.assert_row_contains(0, "[cursor L4:C1] n.txt");
    s.keys("3Kj");
    s.assert_row_contains(0, "[cursor L5:C1] n.txt");
    s.keys("3/");
    s.assert_row_matches(-1, r##"^/█"##);
    s.keys("<Esc>j");
    s.assert_row_contains(0, "[cursor L6:C1] n.txt");
    s.keys("3:");
    s.assert_row_matches(-1, r##"^:█"##);
    s.keys("<Esc>j");
    s.assert_row_contains(0, "[cursor L7:C1] n.txt");
    s.keys("3(");
    s.assert_row_contains(-1, "no numbered comments");
    s.keys("j");
    s.assert_row_contains(0, "[cursor L8:C1] n.txt");
    s.keys("3)j");
    s.assert_row_contains(0, "[cursor L9:C1] n.txt");
    s.keys("3a");
    // editor open clears the count
    s.assert_row_contains(-1, "enter send");
    s.keys("<Esc>j");
    s.assert_row_contains(0, "[cursor L10:C1] n.txt");
    s.keys("3<Tab>j");
    // file change clears the count
    s.assert_row_contains(0, "[1/2] [cursor L2:C1] m.txt");
    s.keys("3<S-Tab>j");
    s.assert_row_contains(0, "[2/2] [cursor L2:C1] n.txt");
}

/// F-CURSOR-05: count cap 99999: more digits keep 99999 (nG on 100001-line file)
/// big.txt untracked, 100001 lines: row 1 hunk, row k+1 = L<k>, last row 100002 = L100001.
#[test]
fn f_cursor_05_count_cap() {
    let mut s = Sim::builder().fixture(Fixture::Empty).size(120, 16).build();
    s.write_file("big.txt", &(1..=100001).map(|n| format!("{n}\n")).collect::<String>());
    s.keys("r");
    s.assert_row_matches(0, r##"\[1/1\] \[cursor L1:C1\] big\.txt \+100001 -0$"##);
    s.keys("12000G");
    // count above 9999 kept (row 12000 = L11999)
    s.assert_row_contains(0, "[cursor L11999:C1] big.txt");
    s.keys("99999G");
    s.assert_row_contains(0, "[cursor L99998:C1] big.txt");
    s.keys("g100000G");
    // 100000 capped to 99999
    s.assert_row_contains(0, "[cursor L99998:C1] big.txt");
    s.keys("g1234567G");
    // more digits keep 99999 (no wrap, no reset, no clamp to last row)
    s.assert_row_contains(0, "[cursor L99998:C1] big.txt");
    s.keys("G");
    s.assert_row_contains(0, "[cursor L100001:C1] big.txt");
}

/// F-CURSOR-05: count prefix - multi digit (0 extends), applies to next key only, not shown, capped digits still a count
/// n.txt untracked, 30 lines "line NN abc" (11 chars): row 1 hunk, row k+1 = L<k>. 80x16: H=13.
#[test]
fn f_cursor_05_count() {
    let mut s = Sim::builder().fixture(Fixture::Empty).size(80, 16).file("n.txt", "line 01 abc\nline 02 abc\nline 03 abc\nline 04 abc\nline 05 abc\nline 06 abc\nline 07 abc\nline 08 abc\nline 09 abc\nline 10 abc\nline 11 abc\nline 12 abc\nline 13 abc\nline 14 abc\nline 15 abc\nline 16 abc\nline 17 abc\nline 18 abc\nline 19 abc\nline 20 abc\nline 21 abc\nline 22 abc\nline 23 abc\nline 24 abc\nline 25 abc\nline 26 abc\nline 27 abc\nline 28 abc\nline 29 abc\nline 30 abc\n").build();
    s.assert_row_matches(0, r##"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/1\] \[cursor L1:C1\] n\.txt \+30 -0$"##);
    s.assert_row_matches(-1, r##"^\(1-13/31\) hjkl move  enter ask  J/K comments  \? help$"##);
    s.keys("2l");
    s.assert_row_contains(0, "[cursor L1:C3] n.txt");
    s.keys("5");
    // pending count not shown anywhere - header, footer, rows unchanged
    s.assert_row_matches(0, r##"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/1\] \[cursor L1:C3\] n\.txt \+30 -0$"##);
    s.assert_row_matches(-1, r##"^\(1-13/31\) hjkl move  enter ask  J/K comments  \? help$"##);
    s.assert_row_matches(3, r##"^        1 \+▶line 01 abc"##);
    s.assert_not_contains("5j");
    s.keys("j");
    s.assert_row_contains(0, "[cursor L6:C3] n.txt");
    s.keys("j");
    // count applied to one key only
    s.assert_row_contains(0, "[cursor L7:C3] n.txt");
    s.keys("10j");
    // 0 extends the pending count (10 rows), no col-1 motion
    s.assert_row_contains(0, "[cursor L17:C3] n.txt");
    s.keys("3l");
    s.assert_row_contains(0, "[cursor L17:C6] n.txt");
    s.keys("l");
    s.assert_row_contains(0, "[cursor L17:C7] n.txt");
    s.keys("12k");
    s.assert_row_contains(0, "[cursor L5:C7] n.txt");
    s.keys("3<Up>");
    s.assert_row_contains(0, "[cursor L2:C7] n.txt");
    s.keys("20G");
    s.assert_row_contains(0, "[cursor L19:C7] n.txt");
    s.keys("105k");
    s.assert_row_contains(0, "[cursor r1:C7] n.txt");
    s.keys("1234567j");
    // count capped (99999) - extra digits stay part of the count, no col-1 motion
    s.assert_row_contains(0, "[cursor L30:C7] n.txt");
    s.keys("0");
    // no count pending - 0 is col 1
    s.assert_row_contains(0, "[cursor L30:C1] n.txt");
}

/// F-CURSOR-06: p is a no-op in unified, in split below 100 cols (same as unified) and in browse - pane always new
#[test]
fn f_cursor_06_noop() {
    let mut s = Sim::builder().fixture(Fixture::Empty).size(80, 12).build();
    s.write_file("f.txt", "a\nxy\n");
    s.git(&["add", "-A"]);
    s.git(&["commit", "-qm", "base"]);
    s.write_file("f.txt", "a\nXY\n");
    s.keys("r");
    s.assert_row_matches(0, r##"\[unified\] .*\[cursor L2:C1\] f\.txt "##);
    s.assert_row_matches(4, r##"^   2      -▶xy"##);
    s.keys("l");
    s.assert_cell(13, 4, C::new().ch('y').reverse(true));
    s.keys("p");
    // unified - no pane prefix, cursor unchanged
    s.assert_row_contains(0, "[cursor L2:C2] f.txt");
    s.assert_row_matches(4, r##"^   2      -▶xy"##);
    s.assert_row_matches(-1, r##"(^|\| )\(1-4/4\) hjkl move  enter ask  J/K comments  \? help$"##);
    s.assert_not_contains("[cursor old");
    s.assert_not_contains("[cursor new");
    s.assert_cell(13, 4, C::new().ch('y').reverse(true));
    s.keys("jp");
    s.assert_row_contains(0, "[cursor L2:C2] f.txt");
    s.assert_not_contains("[cursor old");
    s.assert_cell(13, 5, C::new().ch('Y').reverse(true));
    s.keys("s");
    // split setting on, 80 cols - rendered unified
    s.assert_row_contains(-1, "too narrow for split | ");
    s.assert_row_contains(0, "[cursor L2:C2] f.txt");
    s.keys("p");
    s.assert_row_contains(0, "[cursor L2:C2] f.txt");
    s.assert_row_matches(5, r##"^        2 \+▶XY"##);
    s.assert_not_contains("[cursor old");
    s.assert_not_contains("[cursor new");
    s.assert_not_contains("│");
    s.assert_cell(13, 5, C::new().ch('Y').reverse(true));
    s.keys("kp");
    s.assert_row_contains(0, "[cursor L2:C2] f.txt");
    s.assert_not_contains("[cursor old");
    s.assert_cell(13, 4, C::new().ch('y').reverse(true));
    s.keys("Ff.txt<Enter>jl");
    s.assert_row_matches(0, r##"^\[browse\] .*\[cursor L2:C2\] f\.txt$"##);
    s.keys("p");
    // browse - p ignored
    s.assert_row_matches(0, r##"^\[browse\] .*\[cursor L2:C2\] f\.txt$"##);
    s.assert_cell(8, 3, C::new().ch('Y').reverse(true));
}

/// F-CURSOR-06: p toggles pane old/new in effective split; header tag shows pane; p ends selection; del-only row char cursor in left pane
/// f.txt base "a\nbb\nc\nxy\n", work "a\nc\nXY\n". rows: 1 hunk, 2 a|a, 3 bb|(empty), 4 c|c, 5 xy|XY.
/// 120 cols: left code col 7, right code col 67.
#[test]
fn f_cursor_06_split() {
    let mut s = Sim::builder().fixture(Fixture::Empty).size(120, 12).args(["--split"]).build();
    s.write_file("f.txt", "a\nbb\nc\nxy\n");
    s.git(&["add", "-A"]);
    s.git(&["commit", "-qm", "base"]);
    s.write_file("f.txt", "a\nc\nXY\n");
    s.keys("r");
    // start on first change row (del-only), pane new; char cursor in left pane (tag pane prefix UNSPEC-20)
    s.assert_row_matches(0, r##"\[cursor (old |new )?L2:C1\] f\.txt "##);
    s.assert_row_matches(4, r##"^   2 -▶bb.*│ *$"##);
    s.assert_row_contains(-1, "p pane  ? help");
    s.assert_cell(7, 4, C::new().ch('b').reverse(true));
    s.keys("2j");
    s.assert_row_contains(0, "[cursor new L3:C1] f.txt");
    s.assert_row_matches(6, r##"^   4 -▶xy.*│   3 \+▶XY"##);
    s.assert_cell(67, 6, C::new().ch('X').reverse(true));
    s.assert_cell(7, 6, C::new().ch('x').reverse(false));
    s.keys("p");
    // old pane - tag shows old side and old line number
    s.assert_row_contains(0, "[cursor old L4:C1] f.txt");
    s.assert_cell(7, 6, C::new().ch('x').reverse(true));
    s.assert_cell(67, 6, C::new().ch('X').reverse(false));
    s.keys("l");
    s.assert_row_contains(0, "[cursor old L4:C2] f.txt");
    s.assert_cell(8, 6, C::new().ch('y').reverse(true));
    s.keys("p");
    s.assert_row_contains(0, "[cursor new L3:C2] f.txt");
    s.assert_cell(68, 6, C::new().ch('Y').reverse(true));
    s.assert_cell(8, 6, C::new().ch('y').reverse(false));
    s.keys("v");
    s.assert_row_contains(0, "[visual new L3:C2] f.txt");
    s.keys("p");
    // p ends the selection and toggles the pane
    s.assert_row_contains(0, "[cursor old L4:C2] f.txt");
    s.assert_row_matches(-1, r##"hjkl move  enter ask  J/K comments  p pane  \? help$"##);
    s.assert_cell(8, 6, C::new().ch('y').reverse(true));
    s.assert_cell(7, 6, C::new().ch('x').bg("#22586b").reverse(false));
}

/// F-CURSOR-07: browse horizontal follow at 80 cols (cw 73, m 4)
/// 80 cols: browse code col 7, cw 73. $ on 100-char line: s = 99-73+1+4 = 31, cursor at 7+68 = 75.
#[test]
fn f_cursor_07_browse_narrow() {
    let mut s = Sim::builder().fixture(Fixture::Empty).size(80, 10).file("h.txt", "x00|x01|x02|x03|x04|x05|x06|x07|x08|x09|x10|x11|x12|x13|x14|x15|x16|x17|x18|x19|x20|x21|x22|x23|x24|\nc00|c01|c02|c03|c04|c05|c06|c07|c08|c09|c10|c11|c12|c13|c14|c15|c16|c17|c18|c19|c20|c21|c22|c23|c24|\n").build();
    s.keys("Fh.txt<Enter>");
    s.assert_row_matches(0, r##"^\[browse\] .*\[cursor L1:C1\] h\.txt"##);
    s.assert_row_matches(2, r##"^   1  ▶x00\|x01\|"##);
    s.assert_row_matches(3, r##"^   2   c00\|c01\|"##);
    s.keys("$");
    s.assert_row_contains(0, "[cursor L1:C100] h.txt");
    s.assert_row_matches(2, r##"^   1  ▶\|x08\|x09\|.*x24\|"##);
    s.assert_row_matches(3, r##"^   2   \|c08\|c09\|.*c24\|$"##);
    s.assert_cell(75, 2, C::new().ch('|').reverse(true));
    s.assert_cell(7, 2, C::new().ch('|').reverse(false));
    s.keys("26h");
    // col 73 >= s+m (35) - kept
    s.assert_row_matches(2, r##"^   1  ▶\|x08\|"##);
    s.keys("38h");
    // col 35 = s+m - kept
    s.assert_row_matches(2, r##"^   1  ▶\|x08\|"##);
    s.assert_cell(11, 2, C::new().ch('|').reverse(true));
    s.keys("h");
    // col 34 < s+m -> s = 30
    s.assert_row_matches(2, r##"^   1  ▶7\|x08\|"##);
}

/// F-CURSOR-07: horizontal follow in split (cw floor((cols-1)/2)-7, both panes shifted) and browse (cw cols-7); new file starts at shift 0
/// f.txt base L1 "o00|..o24|" + L2 "c00|..c24|", work L1 "x00|..x24|" + same L2 (100 chars each). g.txt second file.
/// split 120: cw 52, m 4; left code col 7, right code col 67.
#[test]
fn f_cursor_07_browse_split() {
    let mut s = Sim::builder().fixture(Fixture::Empty).size(120, 10).args(["--split"]).build();
    s.write_file("f.txt", "o00|o01|o02|o03|o04|o05|o06|o07|o08|o09|o10|o11|o12|o13|o14|o15|o16|o17|o18|o19|o20|o21|o22|o23|o24|\nc00|c01|c02|c03|c04|c05|c06|c07|c08|c09|c10|c11|c12|c13|c14|c15|c16|c17|c18|c19|c20|c21|c22|c23|c24|\n");
    s.write_file("g.txt", "g\n");
    s.git(&["add", "-A"]);
    s.git(&["commit", "-qm", "base"]);
    s.write_file("f.txt", "x00|x01|x02|x03|x04|x05|x06|x07|x08|x09|x10|x11|x12|x13|x14|x15|x16|x17|x18|x19|x20|x21|x22|x23|x24|\nc00|c01|c02|c03|c04|c05|c06|c07|c08|c09|c10|c11|c12|c13|c14|c15|c16|c17|c18|c19|c20|c21|c22|c23|c24|\n");
    s.write_file("g.txt", "G\n");
    s.keys("r");
    s.assert_row_contains(0, "[1/2] [cursor new L1:C1] f.txt");
    s.assert_row_matches(3, r##"^   1 -▶o00\|o01\|.*│   1 \+▶x00\|x01\|"##);
    s.keys("$");
    // col 99 -> s = 99-52+1+4 = 52; cursor at 67+47 = 114; both panes and context row shifted
    s.assert_row_contains(0, "[cursor new L1:C100] f.txt");
    s.assert_row_matches(2, r##"^@@ -1,2 \+1,2 @@$"##);
    s.assert_row_matches(3, r##"^   1 -▶o13\|o14\|.*│   1 \+▶x13\|x14\|.*x24\|"##);
    s.assert_row_matches(4, r##"^   2   c13\|c14\|.*│   2   c13\|c14\|.*c24\|$"##);
    s.assert_cell(114, 3, C::new().ch('|').reverse(true));
    s.keys("<Tab><S-Tab>");
    // file shown again from its start position - col 1, shift 0
    s.assert_row_contains(0, "[1/2] [cursor new L1:C1] f.txt");
    s.assert_row_matches(3, r##"^   1 -▶o00\|o01\|.*│   1 \+▶x00\|x01\|"##);
    s.assert_cell(67, 3, C::new().ch('x').reverse(true));
    s.keys("Ff.txt<Enter>");
    s.assert_row_matches(0, r##"^\[browse\] .*\[cursor L1:C1\] f\.txt"##);
    s.assert_row_matches(2, r##"^   1  ▶x00\|x01\|"##);
    s.keys("$");
    // browse cw = 113 fits 100 chars - no shift
    s.assert_row_contains(0, "[cursor L1:C100] f.txt");
    s.assert_row_matches(2, r##"^   1  ▶x00\|x01\|"##);
    s.assert_cell(106, 2, C::new().ch('|').reverse(true));
}

/// F-CURSOR-07: narrow code width - margin min(4, floor((cw-1)/2)): cols 20 -> cw 8, m 3
/// a.txt untracked L1 = alphabet twice. Unified code col 12..19; last cell of a cut row is "…".
#[test]
fn f_cursor_07_margin() {
    let mut s = Sim::builder()
        .fixture(Fixture::Empty)
        .size(20, 8)
        .file("a.txt", "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ\n")
        .build();
    s.assert_row_matches(3, r##"^        1 \+▶abcdefg"##);
    s.assert_cell(12, 3, C::new().ch('a').reverse(true));
    s.keys("4l");
    // col 4 = s+cw-1-m (0+8-1-3) - shift 0
    s.assert_row_matches(3, r##"^        1 \+▶abcdefg"##);
    s.assert_cell(16, 3, C::new().ch('e').reverse(true));
    s.keys("l");
    // col 5 -> s = 5-8+1+3 = 1
    s.assert_row_matches(3, r##"^        1 \+▶bcdefgh"##);
    s.assert_cell(16, 3, C::new().ch('f').reverse(true));
    s.keys("0");
    s.assert_row_matches(3, r##"^        1 \+▶abcdefg"##);
}

/// F-CURSOR-07: unified horizontal follow - cw cols-12, margin 4; shift kept inside margins, moves by rule; all line rows shifted, numbers and hunk row fixed
/// h.txt untracked: L1 = "x00|x01|...x24|" (100 chars, index 4k = 'x' of group k), L2 = "y00|...y14|" (60 chars).
/// 80 cols: code starts col 12, cw 68, m 4.
#[test]
fn f_cursor_07_unified() {
    let mut s = Sim::builder().fixture(Fixture::Empty).size(80, 10).file("h.txt", "x00|x01|x02|x03|x04|x05|x06|x07|x08|x09|x10|x11|x12|x13|x14|x15|x16|x17|x18|x19|x20|x21|x22|x23|x24|\ny00|y01|y02|y03|y04|y05|y06|y07|y08|y09|y10|y11|y12|y13|y14|\n").build();
    s.assert_row_contains(0, "[cursor L1:C1] h.txt");
    s.assert_row_matches(2, r##"^@@ -0,0 \+1,2 @@$"##);
    s.assert_row_matches(3, r##"^        1 \+▶x00\|x01\|"##);
    s.assert_row_matches(4, r##"^        2 \+ y00\|y01\|"##);
    s.keys("$");
    // col 99 -> s = 99-68+1+4 = 36; cursor at screen col 12+99-36 = 75
    s.assert_row_contains(0, "[cursor L1:C100] h.txt");
    s.assert_row_matches(2, r##"^@@ -0,0 \+1,2 @@$"##);
    s.assert_row_matches(3, r##"^        1 \+▶x09\|x10\|.*x24\|"##);
    s.assert_row_matches(4, r##"^        2 \+ y09\|y10\|y11\|y12\|y13\|y14\|$"##);
    s.assert_cell(75, 3, C::new().ch('|').reverse(true));
    s.assert_cell(12, 3, C::new().ch('x').reverse(false));
    s.keys("50h");
    // col 49 >= s+m (40) - shift kept
    s.assert_row_contains(0, "[cursor L1:C50] h.txt");
    s.assert_row_matches(3, r##"^        1 \+▶x09\|x10\|"##);
    s.assert_cell(25, 3, C::new().ch('1').reverse(true));
    s.keys("10h");
    // col 39 < s+m -> s = 39-4 = 35
    s.assert_row_contains(0, "[cursor L1:C40] h.txt");
    s.assert_row_matches(3, r##"^        1 \+▶\|x09\|x10\|"##);
    s.assert_row_matches(4, r##"^        2 \+ \|y09\|y10\|"##);
    s.assert_row_matches(2, r##"^@@ -0,0 \+1,2 @@$"##);
    s.assert_cell(16, 3, C::new().ch('|').reverse(true));
    s.keys("0");
    s.assert_row_contains(0, "[cursor L1:C1] h.txt");
    s.assert_row_matches(3, r##"^        1 \+▶x00\|x01\|"##);
    s.assert_row_matches(4, r##"^        2 \+ y00\|y01\|"##);
    s.keys("63l");
    // col 63 = s+cw-1-m (63) - shift still 0
    s.assert_row_contains(0, "[cursor L1:C64] h.txt");
    s.assert_row_matches(3, r##"^        1 \+▶x00\|x01\|"##);
    s.assert_cell(75, 3, C::new().ch('|').reverse(true));
    s.keys("l");
    // col 64 > 63 -> s = 64-68+1+4 = 1
    s.assert_row_contains(0, "[cursor L1:C65] h.txt");
    s.assert_row_matches(3, r##"^        1 \+▶00\|x01\|"##);
    s.assert_row_matches(4, r##"^        2 \+ 00\|y01\|"##);
    s.assert_cell(75, 3, C::new().ch('x').reverse(true));
    s.keys("j");
    // vertical move to shorter row - col 59, still inside margins, shift kept
    s.assert_row_contains(0, "[cursor L2:C60] h.txt");
    s.assert_row_matches(3, r##"^        1 \+ 00\|x01\|"##);
    s.assert_row_matches(4, r##"^        2 \+▶00\|y01\|"##);
    s.assert_cell(70, 4, C::new().ch('|').reverse(true));
    s.keys("2k");
    // hunk row never shifted
    s.assert_row_contains(0, "[cursor r1:");
    s.assert_row_matches(2, r##"^@@ -0,0 \+1,2 @@"##);
}

/// F-CURSOR-08: global diff keys act beside cursor keys - t, s, c, ? keep working with the cursor moved
#[test]
fn f_cursor_08_global_keys() {
    let mut s = Sim::builder()
        .fixture(Fixture::Empty)
        .size(120, 14)
        .file("f.txt", "alpha beta\ngamma\ndelta\n")
        .build();
    s.keys("jl");
    s.assert_row_matches(0, r##"^\[all\] \[full\] \[unified\] \[solarized\] .*\[cursor L2:C2\] f\.txt"##);
    s.keys("t");
    s.assert_row_matches(0, r##"^\[all\] \[full\] \[unified\] \[vibrant\] .*\[cursor L2:C2\] f\.txt"##);
    s.keys("?");
    s.assert_contains("Help · Diff view");
    s.keys("?");
    s.keys("?");
    s.assert_not_contains("Help ·");
    s.keys("s");
    s.assert_row_matches(0, r##"^\[all\] \[full\] \[split\] \[vibrant\] .*\[cursor new L1:C1\] f\.txt"##);
    s.keys("c");
    s.assert_row_matches(0, r##"^\[all\] \[changes\] \[split\] \[vibrant\] "##);
}

/// F-CURSOR-08: i is unbound - no-op in diff, visual, focused comment and browse (nothing typed, no mode, no note)
#[test]
fn f_cursor_08_i_noop() {
    let mut s = Sim::builder()
        .fixture(Fixture::Empty)
        .size(120, 14)
        .file("f.txt", "alpha beta\ngamma\ndelta\n")
        .build();
    s.keys("l");
    s.assert_row_matches(0, r##"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/1\] \[cursor L1:C2\] f\.txt \+3 -0$"##);
    s.assert_row_matches(-1, r##"^\(1-4/4\) hjkl move  enter ask  J/K comments  \? help$"##);
    s.keys("i");
    // diff view - unchanged
    s.assert_row_matches(0, r##"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/1\] \[cursor L1:C2\] f\.txt \+3 -0$"##);
    s.assert_row_matches(3, r##"^        1 \+▶alpha beta"##);
    s.assert_row_matches(4, r##"^        2 \+ gamma$"##);
    s.assert_row_matches(-1, r##"^\(1-4/4\) hjkl move  enter ask  J/K comments  \? help$"##);
    s.assert_not_contains("╭");
    s.assert_not_contains("insert");
    s.assert_not_contains("INSERT");
    s.assert_cell(13, 3, C::new().ch('l').reverse(true));
    s.keys("vl");
    s.assert_row_contains(0, "[visual L1:C3] f.txt");
    s.assert_row_matches(-1, r##"^\(1-4/4\) v/esc end  enter ask  hjkl move  \? help$"##);
    s.keys("i");
    // visual - selection kept
    s.assert_row_contains(0, "[visual L1:C3] f.txt");
    s.assert_row_matches(-1, r##"^\(1-4/4\) v/esc end  enter ask  hjkl move  \? help$"##);
    s.assert_cell(13, 3, C::new().ch('l').bg("#6b4f00"));
    s.keys("<Esc>ac1<Enter>J");
    s.assert_row_matches(-1, r##"e edit  D delete  a ask/follow up  j/k scroll  esc back  \? help$"##);
    s.assert_contains("▸ sent  line L1");
    s.keys("i");
    // focused comment - focus kept, no edit
    s.assert_row_matches(-1, r##"e edit  D delete  a ask/follow up  j/k scroll  esc back  \? help$"##);
    s.assert_contains("▸ sent  line L1");
    s.assert_not_contains("edit line L1");
    s.keys("<Esc>Ff.txt<Enter>j");
    s.assert_row_matches(0, r##"^\[browse\] \[solarized\] \[mcp: off\] \[cursor L2:C1\] f\.txt$"##);
    s.keys("i");
    // browse - unchanged
    s.assert_row_matches(0, r##"^\[browse\] \[solarized\] \[mcp: off\] \[cursor L2:C1\] f\.txt$"##);
    s.assert_row_matches(-1, r##"\(1-3/3\) hjkl move  enter ask  J/K comments  \? help$"##);
    s.assert_not_contains("enter send");
    s.assert_not_contains("insert");
}

/// F-CURSOR-09: start - changes scope: first row (hunk row) col 1; split: pane new
#[test]
fn f_cursor_09_start_changes_split() {
    let mut s = Sim::builder().size(120, 24).args(["--changes-only", "--split"]).build();
    s.assert_row_matches(
        0,
        r##"^\[all\] \[changes\] \[split\] .*\[1/4\] \[cursor new r1:C1\] README\.md \+2 -1$"##,
    );
    s.assert_row_matches(2, r##"^@@ -1,2 \+1,3 @@"##);
    s.assert_not_contains("▶");
    s.assert_cell(0, 2, C::new().ch('@').bg("#22586b"));
    s.keys("2j");
    // pane new - char cursor in right pane
    s.assert_row_contains(0, "[cursor new L2:C1] README.md");
    s.assert_cell(67, 4, C::new().ch('h').reverse(true));
    s.assert_cell(7, 4, C::new().ch('h').reverse(false));
}

/// F-CURSOR-09: start - first change row is not the first line row: cursor skips the unchanged lines
/// README.md base "# Title\nhello\n" (standard fixture); overwritten before start with 2 appended lines.
#[test]
fn f_cursor_09_start_later() {
    let s = Sim::builder().size(80, 24).file("README.md", "# Title\nhello\nnew three\nnew four\n").build();
    s.assert_row_matches(0, r##"\[1/4\] \[cursor L3:C1\] README\.md "##);
    s.assert_row_matches(3, r##"^   1    1   # Title$"##);
    s.assert_row_matches(4, r##"^   2    2   hello$"##);
    s.assert_row_matches(5, r##"^        3 \+▶new three"##);
    s.assert_row_matches(6, r##"^        4 \+ new four$"##);
    s.assert_cell(12, 5, C::new().ch('n').bg("#22586b").reverse(true));
}

/// F-CURSOR-09: start - cursor on from first frame at first change row of first file (full scope), col 1, pane new; Esc/i never turn it off
#[test]
fn f_cursor_09_start() {
    let mut s = Sim::builder().size(80, 24).build();
    s.assert_row_matches(
        0,
        r##"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/4\] \[cursor L2:C1\] README\.md "##,
    );
    s.assert_row_matches(2, r##"^@@ -1,2 \+1,3 @@"##);
    s.assert_row_matches(3, r##"^   1    1   # Title$"##);
    s.assert_row_matches(4, r##"^   2      -▶hello"##);
    s.assert_row_matches(5, r##"^        2 \+ hello world$"##);
    s.assert_row(-1, "(1-5/5) hjkl move  enter ask  J/K comments  ? help");
    s.assert_cell(12, 4, C::new().ch('h').bg("#22586b").reverse(true));
    s.assert_cell(10, 4, C::new().ch('-').bg("#22586b"));
    s.assert_cell(0, 3, C::new().bg("default"));
    s.keys("<Esc><Esc>ii<Esc>");
    // no plain scroll mode - cursor stays on, unchanged
    s.assert_row_matches(0, r##"\[1/4\] \[cursor L2:C1\] README\.md "##);
    s.assert_row_matches(4, r##"^   2      -▶hello"##);
    s.assert_row(-1, "(1-5/5) hjkl move  enter ask  J/K comments  ? help");
    s.assert_cell(12, 4, C::new().ch('h').bg("#22586b").reverse(true));
}

/// F-CURSOR-10: Esc chain in browse - ends selection first, unfocuses comment first, then back to diff
#[test]
fn f_cursor_10_browse() {
    let mut s = Sim::builder()
        .fixture(Fixture::Empty)
        .size(120, 16)
        .file("f.txt", "alpha beta\ngamma\ndelta\n")
        .file("g.txt", "other\n")
        .build();
    s.assert_row_matches(0, r##"^\[all\] .*\[1/2\] \[cursor L1:C1\] f\.txt"##);
    s.keys("Fg.txt<Enter>");
    s.assert_row_matches(0, r##"^\[browse\] \[solarized\] \[mcp: off\] \[cursor L1:C1\] g\.txt$"##);
    s.keys("vl");
    s.assert_row_matches(0, r##"^\[browse\] .*\[visual L1:C2\] g\.txt$"##);
    s.keys("<Esc>");
    // selection ended, still browsing
    s.assert_row_matches(0, r##"^\[browse\] .*\[cursor L1:C2\] g\.txt$"##);
    s.assert_row_matches(-1, r##"hjkl move  enter ask  J/K comments  \? help$"##);
    s.keys("<Esc>");
    // back to diff, same diff file
    s.assert_row_matches(0, r##"^\[all\] .*\[1/2\] \[cursor L1:C1\] f\.txt"##);
    s.keys("Fg.txt<Enter>ac1<Enter>J");
    s.assert_contains("▸ sent  line L1");
    s.assert_row_matches(0, r##"^\[browse\] .*g\.txt$"##);
    s.assert_row_matches(-1, r##"e edit  D delete  a ask/follow up  j/k scroll  esc back  \? help$"##);
    s.keys("<Esc>");
    // comment unfocused, still browsing
    s.assert_not_contains("▸ sent");
    s.assert_row_matches(0, r##"^\[browse\] .*\[cursor L1:C1\] g\.txt$"##);
    s.assert_row_matches(-1, r##"hjkl move  enter ask  J/K comments  \? help$"##);
    s.keys("<Esc>");
    s.assert_row_matches(0, r##"^\[all\] .*\[1/2\] \[cursor L1:C1\] f\.txt"##);
}

/// F-CURSOR-10: Esc chain - unpick copy button before unfocusing the comment
#[test]
fn f_cursor_10_copy_button() {
    let mut s = Sim::builder()
        .fixture(Fixture::Empty)
        .size(120, 20)
        .file("${CONFIG}/xplain/config.json", &json!({"mcp":{"autostart":true}}).to_string())
        .file("f.txt", "alpha\nbeta\n")
        .build();
    s.assert_row_contains(0, "[mcp: on]");
    let r = s.mcp_call("annotate", json!({"file":"f.txt","line":1,"text":"see:\n```ts\nconst x = 1;\n```"}));
    assert_eq!(r.status, 200);
    assert_eq!(r.tool_result(), json!({"ok":true}));
    s.assert_contains("[ copy ]");
    s.keys("J");
    s.assert_contains("▸ sent  agent note L1");
    s.assert_not_contains("enter copy  esc cancel");
    s.assert_row_matches(-1, r##"e edit  D delete  a ask/follow up  j/k scroll  esc back  \? help$"##);
    s.keys("<Down>");
    s.assert_contains("▸ sent  agent note L1");
    s.assert_contains("enter copy  esc cancel");
    s.keys("<Esc>");
    // button unpicked, comment still focused
    s.assert_contains("▸ sent  agent note L1");
    s.assert_not_contains("enter copy  esc cancel");
    s.assert_row_matches(-1, r##"e edit  D delete  a ask/follow up  j/k scroll  esc back  \? help$"##);
    s.keys("<Esc>");
    // now unfocused
    s.assert_contains(" sent  agent note L1");
    s.assert_not_contains("▸ sent");
    s.assert_not_contains("enter copy  esc cancel");
    s.assert_row_contains(0, "[cursor L1:C1] f.txt");
    s.assert_row_matches(-1, r##"hjkl move  enter ask  J/K comments  \? help$"##);
}

/// F-CURSOR-10: Esc chain in diff - nothing to undo: no-op, no note; ends selection (cursor kept); unfocuses comment; help panel does not eat Esc
#[test]
fn f_cursor_10_diff() {
    let mut s = Sim::builder()
        .fixture(Fixture::Empty)
        .size(120, 16)
        .file("f.txt", "alpha beta\ngamma\ndelta\n")
        .build();
    s.keys("jl");
    s.assert_row_matches(0, r##"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/1\] \[cursor L2:C2\] f\.txt \+3 -0$"##);
    s.assert_row(-1, "(1-4/4) hjkl move  enter ask  J/K comments  ? help");
    s.keys("<Esc>");
    // nothing to undo - no state change, no note
    s.assert_row_matches(0, r##"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/1\] \[cursor L2:C2\] f\.txt \+3 -0$"##);
    s.assert_row_matches(4, r##"^        2 \+▶gamma"##);
    s.assert_row(-1, "(1-4/4) hjkl move  enter ask  J/K comments  ? help");
    s.assert_cell(13, 4, C::new().ch('a').reverse(true));
    s.keys("vl");
    s.assert_row_contains(0, "[visual L2:C3] f.txt");
    s.assert_row(-1, "(1-4/4) v/esc end  enter ask  hjkl move  ? help");
    s.assert_cell(13, 4, C::new().ch('a').bg("#6b4f00"));
    s.keys("<Esc>");
    // selection ended, cursor stays
    s.assert_row_contains(0, "[cursor L2:C3] f.txt");
    s.assert_row(-1, "(1-4/4) hjkl move  enter ask  J/K comments  ? help");
    s.assert_cell(13, 4, C::new().ch('a').bg("#22586b"));
    s.assert_cell(14, 4, C::new().ch('m').reverse(true));
    s.keys("<Esc>");
    s.assert_row_contains(0, "[cursor L2:C3] f.txt");
    s.assert_row(-1, "(1-4/4) hjkl move  enter ask  J/K comments  ? help");
    s.keys("?vl");
    s.assert_contains("Help · Visual selection");
    s.assert_row_contains(0, "[visual L2:C4] f.txt");
    s.keys("<Esc>");
    // help open - Esc goes to the chain (ends selection), panel stays open
    s.assert_contains("Help · Diff view");
    s.assert_row_contains(0, "[cursor L2:C4] f.txt");
    s.keys("??");
    s.assert_not_contains("Help ·");
    s.keys("ac1<Enter>kJ");
    // J focuses the comment and moves the cursor to its row
    s.assert_contains("▸ sent  line L2");
    s.assert_row_contains(0, "[cursor L2:C4] f.txt");
    s.assert_row_matches(-1, r##"e edit  D delete  a ask/follow up  j/k scroll  esc back  \? help$"##);
    s.keys("<Esc>");
    // comment unfocused, cursor stays
    s.assert_contains(" sent  line L2");
    s.assert_not_contains("▸ sent");
    s.assert_row_contains(0, "[cursor L2:C4] f.txt");
    s.assert_row_matches(-1, r##"hjkl move  enter ask  J/K comments  \? help$"##);
    s.keys("<Esc>");
    s.assert_not_contains("▸ sent");
    s.assert_row_matches(0, r##"^\[all\] .*\[cursor L2:C4\] f\.txt"##);
    s.assert_row_matches(-1, r##"hjkl move  enter ask  J/K comments  \? help$"##);
}
