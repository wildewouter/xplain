//! Scenario tests: edge.
#![allow(clippy::panic, clippy::unwrap_used)]

use std::os::unix::fs::PermissionsExt;

use xplain_sim::{CellExpect as C, Fixture, Sim};

/// F-THEME-02: solarized theme chrome colors (header chips, rule, hunk, gutter, add/del bg + marks, cursor row,
/// visual, picker, footer).
///
/// Fixture README.md: rows 2 @@, 3 context L1, 4 del L2 (cursor at start), 5 add L2, 6 add L3; code starts col 12.
#[test]
fn f_theme_02_solarized() {
    let mut s = Sim::builder().args(["--theme", "solarized"]).build();
    s.assert_row_matches(
        0,
        r"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/4\] \[cursor L2:C1\] README\.md \+2 -1$",
    );
    s.assert_row_matches(2, r"^@@ -1,2 \+1,3 @@");
    s.assert_row(3, "   1    1   # Title");
    s.assert_row_matches(4, r"^   2      -▶hello");
    s.assert_row(5, "        2 + hello world");

    // header chips: mode + scope `mode`; layout, theme, mcp `view`; path `file`; +N adds; -N dels; cursor tag
    // accent bold
    let chip = |text: &str, offset: usize, want: C| {
        let p = s.find_in_row(0, text).unwrap_or_else(|| panic!("{text:?} not in header\n{}", s.dump()));
        s.assert_cell(p.x + offset, 0, want);
    };
    chip("[all]", 0, C::new().fg("#b58900"));
    chip("[full]", 1, C::new().fg("#b58900"));
    chip("[unified]", 1, C::new().fg("#6c71c4"));
    chip("[solarized]", 1, C::new().fg("#6c71c4"));
    chip("[mcp: off]", 1, C::new().fg("#6c71c4"));
    chip("[cursor", 1, C::new().fg("#cb4b16").bold(true));
    chip("README.md", 0, C::new().fg("#268bd2"));
    chip("+2", 0, C::new().fg("#859900"));
    chip("-1", 0, C::new().fg("#dc322f"));

    // rule dim, hunk header fg hunk, footer dim
    s.assert_cell(0, 1, C::new().ch('─').fg("#586e75"));
    s.assert_cell(0, 2, C::new().ch('@').fg("#2aa198"));
    s.assert_cell(1, -1, C::new().ch('1').fg("#586e75"));

    // context row: gutter number fg, no bg
    s.assert_cell(3, 3, C::new().ch('1').fg("#586e75").bg("default"));

    // add row: number + mark cells bg addBg, mark fg addMark bold, code no bg
    s.assert_cell(0, 5, C::new().bg("#0b3b1f"));
    s.assert_cell(8, 5, C::new().ch('2').fg("#586e75").bg("#0b3b1f"));
    s.assert_cell(10, 5, C::new().ch('+').fg("#859900").bg("#0b3b1f").bold(true));
    s.assert_cell(12, 5, C::new().ch('h').bg("default"));

    // cursor on del row: whole row bg curBg (numbers, mark, code, padding), del mark still shown
    s.assert_cell(0, 4, C::new().bg("#22586b"));
    s.assert_cell(3, 4, C::new().ch('2').bg("#22586b"));
    s.assert_cell(10, 4, C::new().ch('-').fg("#dc322f").bg("#22586b"));
    s.assert_cell(11, 4, C::new().ch('▶').bg("#22586b"));
    s.assert_cell(13, 4, C::new().ch('e').bg("#22586b"));
    s.assert_cell(40, 4, C::new().bg("#22586b"));

    // cursor up to context row; del row gets delBg gutter, context row gets curBg
    s.keys("k");
    s.assert_row_matches(0, r"\[cursor L1:C1\] README\.md ");
    s.assert_row(4, "   2      - hello");
    s.assert_cell(0, 4, C::new().bg("#4a1a1f"));
    s.assert_cell(3, 4, C::new().ch('2').fg("#586e75").bg("#4a1a1f"));
    s.assert_cell(10, 4, C::new().ch('-').fg("#dc322f").bg("#4a1a1f").bold(true));
    s.assert_cell(12, 4, C::new().ch('h').bg("default"));
    s.assert_cell(0, 3, C::new().bg("#22586b"));
    s.assert_cell(40, 3, C::new().bg("#22586b"));

    // char visual selection over `# T`: selected non-cursor cells bg visBg fg visFg
    s.keys("vll");
    s.assert_row_matches(0, r"\[visual L1:C3\] README\.md ");
    s.assert_cell(12, 3, C::new().ch('#').fg("#fdf6e3").bg("#6b4f00"));
    s.assert_cell(13, 3, C::new().ch(' ').fg("#fdf6e3").bg("#6b4f00"));
    s.assert_cell(16, 3, C::new().ch('t').bg("#22586b"));
    s.keys("<Esc>");
    s.assert_row_matches(0, r"\[cursor L1:C3\] README\.md ");
    s.assert_cell(12, 3, C::new().ch('#').bg("#22586b"));

    // picker: selected row selBg/selFg, border modalBorder, box bg modalBg
    s.keys("f");
    s.assert_contains(" Files (1/4)");
    s.assert_matches(r"│>M README\.md \+2 -1 \*\s+│");
    s.assert_text_cell("README.md +2 -1 *", 0, C::new().ch('R').fg("#93a1a1").bg("#073642"));
    s.assert_text_cell("README.md +2 -1 *", 4, C::new().ch('M').fg("#93a1a1").bg("#073642"));
    s.assert_text_cell("╭", 0, C::new().fg("#268bd2"));
    s.assert_text_cell("src/big.ts", 0, C::new().fg("#93a1a1").bg("#002b36"));
    s.keys("<Esc>");
    s.assert_not_contains("Files (");
}

fn empty() -> Sim {
    Sim::builder().fixture(Fixture::Empty).build()
}

fn commit_base(s: &Sim) {
    s.git(&["add", "-A"]);
    s.git(&["commit", "-qm", "base"]);
}

fn chmod(s: &Sim, path: &str, mode: u32) {
    std::fs::set_permissions(s.path(path), std::fs::Permissions::from_mode(mode)).unwrap();
}

/// Cell of `text` searched on `row`, `offset` cells in.
fn at(s: &Sim, row: isize, text: &str, offset: usize, want: C) {
    let p = s.find_in_row(row, text).unwrap_or_else(|| panic!("{text:?} not in row {row}\n{}", s.dump()));
    s.assert_cell(p.x + offset, row, want);
}

/// F-EDGE-01: diff paths shown repo-root relative, only git's own a/ b/ prefix stripped once (repo dirs a/ b/ kept);
/// deleted file shows old path; new file shows new path.
#[test]
fn f_edge_01_paths() {
    let mut s = empty();
    s.assert_row_contains(0, "No changes");
    s.write_file("docs/gone.md", "g1\ng2\n");
    s.write_file("src/deep/mod.ts", "export const v = 1;\n");
    s.write_file("b/a/x.txt", "x1\n");
    s.write_file("b/a/w.txt", "w1\nw2\n");
    s.write_file("a/old.txt", "r1\nr2\nr3\nr4\n");
    commit_base(&s);
    s.remove_file("docs/gone.md");
    s.remove_file("b/a/w.txt");
    s.write_file("src/deep/mod.ts", "export const v = 2;\n");
    s.write_file("b/a/x.txt", "x2\n");
    s.write_file("lib/fresh.txt", "new\n");
    s.write_file("a/y.txt", "y\n");
    s.git(&["add", "lib/fresh.txt", "a/y.txt"]);
    s.git(&["mv", "a/old.txt", "b/a/new.txt"]);
    s.keys("r");
    // added file in repo dir a/ keeps its own a/
    s.assert_row_matches(
        0,
        r"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/7\] \[cursor L1:C1\] a/y\.txt \+1 -0$",
    );
    s.assert_row_matches(2, r"^@@ -0,0 \+1 @@");
    s.assert_not_contains("/dev/null");
    s.assert_not_contains("b/a/y");
    s.keys("<Tab>");
    // rename between repo dirs a/ and b/ keeps both
    s.assert_row_matches(0, r"\[2/7\] \[cursor r1:C1\] a/old\.txt -> b/a/new\.txt \+0 -0$");
    s.assert_row_matches(2, r"^Renamed, no content changes");
    s.keys("<Tab>");
    // deleted file in repo dir b/a/ listed by its full old path
    s.assert_row_matches(0, r"\[3/7\] \[cursor L1:C1\] b/a/w\.txt \+0 -2$");
    s.assert_row_matches(2, r"^@@ -1,2 \+0,0 @@");
    s.assert_not_contains("/dev/null");
    s.keys("<Tab>");
    s.assert_row_matches(0, r"\[4/7\] \[cursor L1:C1\] b/a/x\.txt \+1 -1$");
    s.keys("<Tab>");
    // deleted file listed by its old path (not /dev/null), no a/ prefix
    s.assert_row_matches(0, r"\[5/7\] \[cursor L1:C1\] docs/gone\.md \+0 -2$");
    s.assert_row_matches(2, r"^@@ -1,2 \+0,0 @@");
    for t in ["/dev/null", "a/docs", "b/docs"] {
        s.assert_not_contains(t);
    }
    s.keys("<Tab>");
    // added file listed by its new path (not /dev/null), no b/ prefix
    s.assert_row_matches(0, r"\[6/7\] \[cursor L1:C1\] lib/fresh\.txt \+1 -0$");
    s.assert_not_contains("/dev/null");
    s.assert_not_contains("b/lib");
    s.keys("<Tab>");
    s.assert_row_matches(0, r"\[7/7\] \[cursor L1:C1\] src/deep/mod\.ts \+1 -1$");
    s.assert_not_contains("a/src");
    s.assert_not_contains("b/src");
    s.keys("f");
    s.assert_contains(" Files (7/7)");
    for m in [
        r"│ A a/y\.txt \+1 -0\s+│",
        r"│ R a/old\.txt -> b/a/new\.txt \+0 -0\s+│",
        r"│ D b/a/w\.txt \+0 -2\s+│",
        r"│ M b/a/x\.txt \+1 -1\s+│",
        r"│ D docs/gone\.md \+0 -2\s+│",
        r"│ A lib/fresh\.txt \+1 -0\s+│",
        r"│>M src/deep/mod\.ts \+1 -1 \*\s+│",
    ] {
        s.assert_matches(m);
    }
    s.assert_not_contains("/dev/null");
}

/// F-EDGE-01: paths with spaces shown exactly (the TAB git appends after such names is not part of the path).
#[test]
fn f_edge_01_spaces() {
    let mut s = empty();
    s.assert_row_contains(0, "No changes");
    s.write_file("my file.txt", "a\n");
    s.write_file("del me.txt", "d1\nd2\n");
    commit_base(&s);
    s.write_file("my file.txt", "b\n");
    s.remove_file("del me.txt");
    s.write_file("new one.txt", "n\n");
    s.git(&["add", "new one.txt"]);
    s.write_file("un tracked.txt", "u\n");
    s.keys("r");
    // deleted file (`--- a/del me.txt<TAB>`)
    s.assert_row_matches(
        0,
        r"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/4\] \[cursor L1:C1\] del me\.txt \+0 -2$",
    );
    s.assert_row_matches(2, r"^@@ -1,2 \+0,0 @@");
    s.assert_not_contains("/dev/null");
    s.keys("<Tab>");
    // modified file
    s.assert_row_matches(0, r"\[2/4\] \[cursor L1:C1\] my file\.txt \+1 -1$");
    s.keys("<Tab>");
    // added file (`+++ b/new one.txt<TAB>`)
    s.assert_row_matches(0, r"\[3/4\] \[cursor L1:C1\] new one\.txt \+1 -0$");
    s.assert_row_matches(2, r"^@@ -0,0 \+1 @@");
    s.assert_not_contains("/dev/null");
    s.keys("<Tab>");
    // untracked file
    s.assert_row_matches(0, r"\[4/4\] \[cursor L1:C1\] un tracked\.txt \+1 -0$");
    s.keys("f");
    // picker rows: path, then one space and the counts (no tab cell)
    s.assert_contains(" Files (4/4)");
    for m in [
        r"│ D del me\.txt \+0 -2\s+│",
        r"│ M my file\.txt \+1 -1\s+│",
        r"│ A new one\.txt \+1 -0\s+│",
        r"│>A un tracked\.txt \+1 -0 \*\s+│",
    ] {
        s.assert_matches(m);
    }
}

/// F-EDGE-02: rename without content change has no hunks, one dim note row, +0 -0.
#[test]
fn f_edge_02_rename_pure() {
    let mut s = empty();
    s.write_file("same.txt", "s1\ns2\ns3\n");
    s.write_file("keep.txt", "k\n");
    commit_base(&s);
    s.git(&["mv", "same.txt", "moved.txt"]);
    s.assert_row_contains(0, "No changes");
    s.keys("r");
    s.assert_row_matches(
        0,
        r"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/1\] \[cursor r1:C1\] same\.txt -> moved\.txt \+0 -0$",
    );
    s.assert_row_matches(2, r"^Renamed, no content changes");
    s.assert_row(3, "");
    s.assert_row_matches(-1, r"\(1-1/1\) ");
    for t in ["@@", "s1", "No textual changes"] {
        s.assert_not_contains(t);
    }
    // note row fg dim
    at(&s, 2, "Renamed", 0, C::new().fg("#586e75"));
    s.keys("f");
    s.assert_matches(r"│>R same\.txt -> moved\.txt \+0 -0 \*\s+│");
}

/// F-EDGE-02: rename with content change shows old -> new in header and picker with status R.
#[test]
fn f_edge_02_rename() {
    let mut s = empty();
    s.write_file("old.txt", "one\ntwo\nthree\nfour\nfive\n");
    s.write_file("a0.txt", "x\n");
    commit_base(&s);
    s.git(&["mv", "old.txt", "new.txt"]);
    s.write_file("new.txt", "one\ntwo\nthree\nfour\nFIVE\n");
    s.write_file("a0.txt", "y\n");
    s.assert_row_contains(0, "No changes");
    s.assert_not_contains("old.txt");
    s.keys("r");
    s.assert_row_matches(0, r"\[1/2\] \[cursor L1:C1\] a0\.txt \+1 -1$");
    s.keys("f");
    s.assert_contains(" Files (1/2)");
    s.assert_matches(r"│>M a0\.txt \+1 -1 \*\s+│");
    s.assert_matches(r"│ R old\.txt -> new\.txt \+1 -1\s+│");
    // picker status R in mode color (unselected row)
    s.assert_text_cell(" R old.txt", 1, C::new().ch('R').fg("#b58900"));
    s.keys("<Esc><Tab>");
    s.assert_row_matches(
        0,
        r"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[2/2\] \[cursor L5:C1\] old\.txt -> new\.txt \+1 -1$",
    );
    s.assert_row_matches(2, r"^@@ -1,5 \+1,5 @@");
    s.assert_row(3, "   1    1   one");
    s.assert_row_matches(7, r"^   5      -▶five");
    s.assert_row(8, "        5 + FIVE");
    s.assert_not_contains("Renamed");
    s.assert_not_contains("No textual changes");
    // header path colored as file path (whole old -> new)
    at(&s, 0, "old.txt -> new.txt", 0, C::new().fg("#268bd2"));
    at(&s, 0, "old.txt -> new.txt", 11, C::new().ch('n').fg("#268bd2"));
}

/// F-EDGE-03: GIT binary patch form (--binary, staged) also shows note row "Binary file", +0 -0, status M.
#[test]
fn f_edge_03_binary_patch() {
    let mut s = Sim::builder().fixture(Fixture::Empty).args(["--staged", "--binary"]).build();
    s.write_bytes("bin.dat", b"a\0b");
    commit_base(&s);
    s.write_bytes("bin.dat", b"a\0c");
    s.git(&["add", "bin.dat"]);
    s.assert_row_contains(0, "No changes");
    s.keys("r");
    s.assert_row_matches(
        0,
        r"^\[staged\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/1\] \[cursor r1:C1\] bin\.dat \+0 -0$",
    );
    s.assert_row_matches(2, r"^Binary file");
    s.assert_row(3, "");
    for t in ["GIT binary", "literal", "@@"] {
        s.assert_not_contains(t);
    }
    s.keys("f");
    s.assert_matches(r"│>M bin\.dat \+0 -0 \*\s+│");
}

/// F-EDGE-03: modified tracked binary file shows one dim note row "Binary file", +0 -0, status M.
#[test]
fn f_edge_03_binary() {
    let mut s = empty();
    s.write_bytes("bin.dat", b"a\0b\n");
    s.write_file("t.txt", "t1\n");
    commit_base(&s);
    s.write_bytes("bin.dat", b"a\0c\n");
    s.write_file("t.txt", "t2\n");
    s.assert_row_contains(0, "No changes");
    s.keys("r");
    s.assert_row_matches(
        0,
        r"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/2\] \[cursor r1:C1\] bin\.dat \+0 -0$",
    );
    s.assert_row_matches(2, r"^Binary file");
    s.assert_row(3, "");
    s.assert_row_matches(-1, r"\(1-1/1\) ");
    s.assert_not_contains("@@");
    s.assert_not_contains("differ");
    at(&s, 2, "Binary file", 0, C::new().fg("#586e75"));
    s.keys("f");
    s.assert_matches(r"│>M bin\.dat \+0 -0 \*\s+│");
    s.assert_matches(r"│ M t\.txt \+1 -1\s+│");
}

/// F-EDGE-04: deleted empty tracked file (no hunks) shows note row "No textual changes"; mode change with content
/// change shows hunks, no note.
#[test]
fn f_edge_04_deleted_empty() {
    let mut s = empty();
    s.write_file("both.sh", "echo a\n");
    s.write_file("gone.txt", "");
    commit_base(&s);
    s.write_file("both.sh", "echo b\n");
    chmod(&s, "both.sh", 0o755);
    s.remove_file("gone.txt");
    s.assert_row_contains(0, "No changes");
    s.keys("r");
    // mode + content change - hunks, no note
    s.assert_row_matches(0, r"\[1/2\] \[cursor L1:C1\] both\.sh \+1 -1$");
    s.assert_row_matches(2, r"^@@ -1 \+1 @@");
    s.assert_not_contains("No textual changes");
    s.keys("<Tab>");
    s.assert_row_matches(0, r"\[2/2\] \[cursor r1:C1\] gone\.txt \+0 -0$");
    s.assert_row_matches(2, r"^No textual changes");
    s.assert_row(3, "");
    for t in ["@@", "Binary file", "Renamed"] {
        s.assert_not_contains(t);
    }
    s.keys("f");
    s.assert_matches(r"│ M both\.sh \+1 -1\s+│");
    s.assert_matches(r"│>M gone\.txt \+0 -0 \*\s+│");
}

/// F-EDGE-04: mode change and tracked empty new file show note row "No textual changes".
#[test]
fn f_edge_04_no_textual() {
    let mut s = empty();
    s.write_file("run.sh", "echo hi\n");
    commit_base(&s);
    s.write_file("run.sh", "echo hi\n");
    chmod(&s, "run.sh", 0o755);
    s.write_file("empty.txt", "");
    s.git(&["add", "empty.txt"]);
    s.assert_row_contains(0, "No changes");
    s.keys("r");
    s.assert_row_matches(
        0,
        r"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/2\] \[cursor r1:C1\] empty\.txt \+0 -0$",
    );
    s.assert_row_matches(2, r"^No textual changes");
    s.assert_row(3, "");
    s.assert_row_matches(-1, r"\(1-1/1\) ");
    for t in ["@@", "Binary file", "Renamed"] {
        s.assert_not_contains(t);
    }
    at(&s, 2, "No textual changes", 0, C::new().fg("#586e75"));
    s.keys("<Tab>");
    s.assert_row_matches(0, r"\[2/2\] \[cursor r1:C1\] run\.sh \+0 -0$");
    s.assert_row_matches(2, r"^No textual changes");
    s.assert_row(3, "");
    s.assert_not_contains("@@");
    s.assert_not_contains("echo hi");
    s.keys("f");
    s.assert_matches(r"│ M empty\.txt \+0 -0\s+│");
    s.assert_matches(r"│>M run\.sh \+0 -0 \*\s+│");
}

/// F-EDGE-05: picker status letters A/D/R/M by hunk headers, incl. untracked, empty and binary files; letter colors.
#[test]
fn f_edge_05_status() {
    let mut s = empty();
    s.write_file("del.txt", "d1\n");
    s.write_file("trunc.txt", "t1\nt2\n");
    s.write_file("grow.txt", "");
    s.write_file("mod.txt", "m1\n");
    s.write_file("ren.txt", "r1\nr2\nr3\nr4\nr5\n");
    s.write_bytes("bdel.bin", b"a\0b");
    commit_base(&s);
    s.remove_file("del.txt");
    s.write_file("trunc.txt", "");
    s.write_file("grow.txt", "g\n");
    s.write_file("mod.txt", "m2\n");
    s.git(&["mv", "ren.txt", "ren2.txt"]);
    s.write_file("ren2.txt", "r1\nr2\nr3\nr4\nR5\n");
    s.remove_file("bdel.bin");
    s.write_bytes("badd.bin", b"x\0y");
    s.write_file("added.txt", "a1\na2\n");
    s.git(&["add", "badd.bin", "added.txt"]);
    s.write_file("utext.txt", "u\n");
    s.write_bytes("ubin.bin", b"u\0v");
    s.write_file("uempty.txt", "");
    s.assert_row_contains(0, "No changes");
    s.keys("r");
    s.assert_row_matches(0, r"\[1/11\] ");
    s.keys("f");
    s.assert_contains(" Files (1/11)");
    for m in [
        r"│>A added\.txt \+2 -0 \*\s+│",
        r"│ M badd\.bin \+0 -0\s+│",
        r"│ M bdel\.bin \+0 -0\s+│",
        r"│ D del\.txt \+0 -1\s+│",
        r"│ A grow\.txt \+1 -0\s+│",
        r"│ M mod\.txt \+1 -1\s+│",
        r"│ R ren\.txt -> ren2\.txt \+1 -1\s+│",
        r"│ D trunc\.txt \+0 -2\s+│",
        r"│ M ubin\.bin \+0 -0\s+│",
        r"│ M uempty\.txt \+0 -0\s+│",
        r"│ A utext\.txt \+1 -0\s+│",
    ] {
        s.assert_matches(m);
    }
    // letter colors (solarized) - A adds, D dels, R mode, M accent
    s.assert_text_cell(" A utext.txt", 1, C::new().ch('A').fg("#859900"));
    s.assert_text_cell(" D del.txt", 1, C::new().ch('D').fg("#dc322f"));
    s.assert_text_cell(" R ren.txt", 1, C::new().ch('R').fg("#b58900"));
    s.assert_text_cell(" M mod.txt", 1, C::new().ch('M').fg("#cb4b16"));
    s.keys("<Esc>");
    // emptied tracked file counts as D (hunk header has +0,0 @@)
    s.keys("<S-Tab><S-Tab><S-Tab><S-Tab>");
    s.assert_row_matches(0, r"\[8/11\] \[cursor L1:C1\] trunc\.txt \+0 -2$");
    s.assert_row_matches(2, r"^@@ -1,2 \+0,0 @@");
    // previously empty tracked file now with content counts as A
    s.keys("<S-Tab><S-Tab><S-Tab><S-Tab>");
    s.assert_row_matches(0, r"\[4/11\] \[cursor L1:C1\] del\.txt \+0 -1$");
    s.assert_row_matches(2, r"^@@ -1 \+0,0 @@");
    s.keys("<Tab>");
    s.assert_row_matches(0, r"\[5/11\] \[cursor L1:C1\] grow\.txt \+1 -0$");
    s.assert_row_matches(2, r"^@@ -0,0 \+1 @@");
    // added / deleted tracked binary - no hunks, note row Binary file
    s.keys("<S-Tab><S-Tab>");
    s.assert_row_matches(0, r"\[3/11\] \[cursor r1:C1\] bdel\.bin \+0 -0$");
    s.assert_row_matches(2, r"^Binary file");
    s.assert_not_contains("@@");
    s.assert_not_contains("No textual changes");
    s.keys("<S-Tab>");
    s.assert_row_matches(0, r"\[2/11\] \[cursor r1:C1\] badd\.bin \+0 -0$");
    s.assert_row_matches(2, r"^Binary file");
    // untracked text file shows as fully added
    s.keys("<S-Tab><S-Tab>");
    s.assert_row_matches(0, r"\[11/11\] \[cursor L1:C1\] utext\.txt \+1 -0$");
    s.assert_row_matches(2, r"^@@ -0,0 \+1 @@");
    s.assert_row_matches(3, r"^        1 \+▶u");
    // untracked empty file - no hunks
    s.keys("<S-Tab>");
    s.assert_row_matches(0, r"\[10/11\] \[cursor r1:C1\] uempty\.txt \+0 -0$");
    s.assert_row_matches(2, r"^No textual changes");
    // untracked binary file - no hunks
    s.keys("<S-Tab>");
    s.assert_row_matches(0, r"\[9/11\] \[cursor r1:C1\] ubin\.bin \+0 -0$");
    s.assert_row_matches(2, r"^Binary file");
}

/// F-EDGE-06: adding the final newline gives marker only on the del side.
#[test]
fn f_edge_06_newline_added() {
    let mut s = empty();
    s.write_file("f.txt", "a\nb");
    commit_base(&s);
    s.write_file("f.txt", "a\nb\n");
    s.assert_row_contains(0, "No changes");
    s.keys("r");
    s.assert_row_matches(0, r"\[1/1\] \[cursor L2:C1\] f\.txt \+1 -1$");
    s.assert_row_matches(2, r"^@@ -1,2 \+1,2 @@");
    s.assert_row(3, "   1    1   a");
    s.assert_row_matches(4, r"^   2      -▶b");
    s.assert_row(5, "   2      -  No newline at end of file");
    s.assert_row(6, "        2 + b");
    s.assert_row(7, "");
    s.assert_row_matches(-1, r"\(1-5/5\) ");
}

/// F-EDGE-06: git no-newline marker line becomes extra del/add row with same line number, not counted.
#[test]
fn f_edge_06_no_newline() {
    let mut s = empty();
    s.write_file("f.txt", "a\nb");
    commit_base(&s);
    s.write_file("f.txt", "a\nc");
    s.assert_row_contains(0, "No changes");
    s.keys("r");
    s.assert_row_matches(
        0,
        r"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/1\] \[cursor L2:C1\] f\.txt \+1 -1$",
    );
    s.assert_row_matches(2, r"^@@ -1,2 \+1,2 @@");
    s.assert_row(3, "   1    1   a");
    s.assert_row_matches(4, r"^   2      -▶b");
    s.assert_row(5, "   2      -  No newline at end of file");
    s.assert_row(6, "        2 + c");
    s.assert_row(7, "        2 +  No newline at end of file");
    s.assert_row(8, "");
    s.assert_row_matches(-1, r"\(1-6/6\) ");
    s.assert_not_contains("\\ No newline");
    // marker rows keep row kind - del / add gutter bg and mark
    s.assert_cell(0, 5, C::new().bg("#4a1a1f"));
    s.assert_cell(10, 5, C::new().ch('-').fg("#dc322f").bg("#4a1a1f"));
    s.assert_cell(0, 7, C::new().bg("#0b3b1f"));
    s.assert_cell(10, 7, C::new().ch('+').fg("#859900").bg("#0b3b1f"));
    // marker row carries the line number of the line before it
    s.keys("j");
    s.assert_row_matches(0, r"\[cursor L2:C1\] f\.txt ");
    s.assert_row_matches(5, r"^   2      -▶ No newline at end of file");
    s.assert_row(4, "   2      - b");
}

/// F-EDGE-08: line text raw, tabs as 2 spaces, CJK chars take 2 cells.
#[test]
fn f_edge_08_content() {
    let mut s = Sim::builder()
        .fixture(Fixture::Empty)
        .file("t.txt", "\tx\na\tb\n日本語x\n<&> %s $HOME \\n\n")
        .build();
    s.assert_row_matches(0, r"\[1/1\] \[cursor L1:C1\] t\.txt \+4 -0$");
    s.assert_row_matches(2, r"^@@ -0,0 \+1,4 @@");
    s.assert_row_matches(3, r"^        1 \+▶  x\s*…?$");
    s.assert_row(4, "        2 + a  b");
    s.assert_row(5, "        3 + 日本語x");
    s.assert_row(6, "        4 + <&> %s $HOME \\n");
    // leading tab = 2 cells, x in 3rd code cell (code starts col 12)
    s.assert_cell(14, 3, C::new().ch('x'));
    s.assert_cell(12, 4, C::new().ch('a'));
    s.assert_cell(15, 4, C::new().ch('b'));
    // each CJK char takes 2 cells
    s.assert_cell(12, 5, C::new().ch('日'));
    s.assert_cell(14, 5, C::new().ch('本'));
    s.assert_cell(16, 5, C::new().ch('語'));
    s.assert_cell(18, 5, C::new().ch('x'));
    // tab also counts as 2 columns for the cursor (ASCII)
    s.keys("$");
    s.assert_row_matches(0, r"\[cursor L1:C3\] ");
}

/// F-EDGE-08: tabs as 2 spaces and CJK chars 2 cells also in both split panes and in browse.
#[test]
fn f_edge_08_split_browse() {
    let mut s = Sim::builder().fixture(Fixture::Empty).args(["--split"]).build();
    s.write_file("t.txt", "\tx\n日本\ty\n");
    commit_base(&s);
    s.write_file("t.txt", "\tX\n日本\ty\n");
    s.keys("r");
    // split, code starts at col 7 (left) and col 67 (right, after the divider at col 59)
    s.assert_row_matches(0, r"\[split\] .*\[1/1\] \[cursor new L1:C1\] t\.txt \+1 -1$");
    s.assert_row_matches(3, r"^   1 -▶  x +…?│   1 \+▶  X +…?$");
    s.assert_row_matches(4, r"^   2   日本  y +│   2   日本  y$");
    s.assert_cell(9, 3, C::new().ch('x'));
    s.assert_cell(69, 3, C::new().ch('X'));
    s.assert_cell(7, 4, C::new().ch('日'));
    s.assert_cell(9, 4, C::new().ch('本'));
    s.assert_cell(13, 4, C::new().ch('y'));
    s.assert_cell(67, 4, C::new().ch('日'));
    s.assert_cell(69, 4, C::new().ch('本'));
    s.assert_cell(73, 4, C::new().ch('y'));
    s.keys("Ft.txt<Enter>");
    // browse, code starts at col 7
    s.assert_row_matches(0, r"^\[browse\] .*\[cursor L1:C1\] t\.txt$");
    s.assert_row_matches(2, r"^   1  ▶  X +…?$");
    s.assert_row(3, "   2   日本  y");
    s.assert_cell(9, 2, C::new().ch('X'));
    s.assert_cell(7, 3, C::new().ch('日'));
    s.assert_cell(9, 3, C::new().ch('本'));
    s.assert_cell(13, 3, C::new().ch('y'));
}

/// F-THEME-01: t works in browse; ignored while picker open; typed into search query.
#[test]
fn f_theme_01_browse_modal() {
    let mut s = Sim::builder().build();
    s.assert_row_matches(0, r"\[solarized\] \[mcp: off\] \[1/4\] ");
    // picker open - t ignored
    s.keys("f");
    s.assert_contains(" Files (1/4)");
    s.keys("t");
    s.assert_contains(" Files (1/4)");
    s.assert_row_matches(0, r"\[solarized\] ");
    s.keys("<Esc>");
    s.assert_not_contains("Files (");
    s.assert_row_matches(0, r"\[solarized\] \[mcp: off\] \[1/4\] ");
    // search modal open - t is query text, not theme
    s.keys("F");
    s.assert_contains(" Search (");
    s.keys("t");
    s.assert_matches(" > t");
    s.assert_row_matches(0, r"\[solarized\] ");
    s.keys("<BS>README.md<Enter>");
    s.assert_row_matches(0, r"^\[browse\] \[solarized\] \[mcp: off\] \[cursor L1:C1\] README\.md$");
    s.keys("t");
    s.assert_row_matches(0, r"^\[browse\] \[vibrant\] \[mcp: off\] \[cursor L1:C1\] README\.md$");
    at(&s, 0, "[browse]", 0, C::new().fg("3"));
    // theme kept when leaving browse
    s.keys("<Esc>");
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[vibrant\] \[mcp: off\] \[1/4\] ");
}

/// F-THEME-01: t starts from configured theme and does not write the config file.
#[test]
fn f_theme_01_config_unsaved() {
    let mut s = Sim::builder().config_json(serde_json::json!({"theme": "colorblind"})).build();
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[colorblind\] \[mcp: off\] ");
    s.keys("t");
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[light\] \[mcp: off\] ");
    s.keys("t");
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] ");
    let text = s.file("${CONFIG}/xplain/config.json");
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["theme"], "colorblind");
    assert!(text.contains("colorblind"));
    assert!(!text.contains("solarized"));
    assert!(!text.contains("light"));
}

/// F-THEME-01: t cycles themes solarized > vibrant > dull > contrast > colorblind > light and wraps; colors follow;
/// not saved.
#[test]
fn f_theme_01_cycle() {
    let mut s = Sim::builder().build();
    s.assert_row_matches(
        0,
        r"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/4\] \[cursor L2:C1\] README\.md \+2 -1$",
    );
    at(&s, 0, "[all]", 0, C::new().fg("#b58900"));
    s.keys("t");
    s.assert_row_matches(
        0,
        r"^\[all\] \[full\] \[unified\] \[vibrant\] \[mcp: off\] \[1/4\] \[cursor L2:C1\] README\.md \+2 -1$",
    );
    at(&s, 0, "[all]", 0, C::new().fg("3"));
    s.assert_cell(0, 4, C::new().bg("#33336b"));
    s.keys("t");
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[dull\] \[mcp: off\] \[1/4\] ");
    at(&s, 0, "[all]", 0, C::new().fg("#b39f80"));
    s.keys("t");
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[contrast\] \[mcp: off\] \[1/4\] ");
    at(&s, 0, "[all]", 0, C::new().fg("#ffff00"));
    s.keys("t");
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[colorblind\] \[mcp: off\] \[1/4\] ");
    at(&s, 0, "[all]", 0, C::new().fg("#f0c674"));
    s.keys("t");
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[light\] \[mcp: off\] \[1/4\] ");
    at(&s, 0, "[all]", 0, C::new().fg("#8a5a00"));
    s.assert_cell(0, 4, C::new().bg("#ffe9a0"));
    // wraps back to first
    s.keys("t");
    s.assert_row_matches(
        0,
        r"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/4\] \[cursor L2:C1\] README\.md \+2 -1$",
    );
    s.assert_row_matches(-1, r"^\(1-5/5\) hjkl move  enter ask  J/K comments  \? help$");
    at(&s, 0, "[all]", 0, C::new().fg("#b58900"));
    s.assert_cell(0, 4, C::new().bg("#22586b"));
    // theme not saved - no config file written
    assert!(s.try_file("${CONFIG}/xplain/config.json").is_none(), "config file was written");
}

/// Chrome colors of one theme, as asserted by the F-THEME-02 scenarios.
struct Pal {
    name: &'static str,
    mode: &'static str,
    view: &'static str,
    accent: &'static str,
    file: &'static str,
    adds: &'static str,
    dels: &'static str,
    dim: &'static str,
    hunk: &'static str,
    num: &'static str,
    add_bg: &'static str,
    add_mark: &'static str,
    del_mark: &'static str,
    cur_bg: &'static str,
    del_bg: &'static str,
    vis_fg: &'static str,
    vis_bg: &'static str,
    sel_fg: &'static str,
    sel_bg: &'static str,
    border: &'static str,
    modal_fg: &'static str,
    modal_bg: &'static str,
}

/// Header chips, rule, hunk, gutter, add/del bg + marks, cursor row, visual, picker, footer of theme `p`.
///
/// Fixture README.md: rows 2 @@, 3 context L1, 4 del L2 (cursor at start), 5 add L2, 6 add L3; code starts col 12.
fn theme_chrome(p: &Pal) {
    let mut s = Sim::builder().args(["--theme", p.name]).build();
    s.assert_row_matches(
        0,
        &format!(
            r"^\[all\] \[full\] \[unified\] \[{}\] \[mcp: off\] \[1/4\] \[cursor L2:C1\] README\.md \+2 -1$",
            p.name
        ),
    );
    s.assert_row_matches(2, r"^@@ -1,2 \+1,3 @@");
    s.assert_row(3, "   1    1   # Title");
    s.assert_row_matches(4, r"^   2      -▶hello");
    s.assert_row(5, "        2 + hello world");

    // header chips: mode + scope `mode`; layout, theme, mcp `view`; path `file`; +N adds; -N dels; cursor accent bold
    at(&s, 0, "[all]", 0, C::new().fg(p.mode));
    at(&s, 0, "[full]", 1, C::new().fg(p.mode));
    at(&s, 0, "[unified]", 1, C::new().fg(p.view));
    at(&s, 0, &format!("[{}]", p.name), 1, C::new().fg(p.view));
    at(&s, 0, "[mcp: off]", 1, C::new().fg(p.view));
    at(&s, 0, "[cursor", 1, C::new().fg(p.accent).bold(true));
    at(&s, 0, "README.md", 0, C::new().fg(p.file));
    at(&s, 0, "+2", 0, C::new().fg(p.adds));
    at(&s, 0, "-1", 0, C::new().fg(p.dels));

    // rule dim, hunk header fg hunk, footer dim
    s.assert_cell(0, 1, C::new().ch('─').fg(p.dim));
    s.assert_cell(0, 2, C::new().ch('@').fg(p.hunk));
    s.assert_cell(1, -1, C::new().ch('1').fg(p.dim));

    // context row: gutter number fg, no bg
    s.assert_cell(3, 3, C::new().ch('1').fg(p.num).bg("default"));

    // add row: number + mark cells bg addBg, mark fg addMark bold, code no bg
    s.assert_cell(0, 5, C::new().bg(p.add_bg));
    s.assert_cell(8, 5, C::new().ch('2').fg(p.num).bg(p.add_bg));
    s.assert_cell(10, 5, C::new().ch('+').fg(p.add_mark).bg(p.add_bg).bold(true));
    s.assert_cell(12, 5, C::new().ch('h').bg("default"));

    // cursor on del row: whole row bg curBg (numbers, mark, code, padding), del mark still shown
    s.assert_cell(0, 4, C::new().bg(p.cur_bg));
    s.assert_cell(3, 4, C::new().ch('2').bg(p.cur_bg));
    s.assert_cell(10, 4, C::new().ch('-').fg(p.del_mark).bg(p.cur_bg));
    s.assert_cell(11, 4, C::new().ch('▶').bg(p.cur_bg));
    s.assert_cell(13, 4, C::new().ch('e').bg(p.cur_bg));
    s.assert_cell(40, 4, C::new().bg(p.cur_bg));

    // cursor up to context row; del row gets delBg gutter, context row gets curBg
    s.keys("k");
    s.assert_row_matches(0, r"\[cursor L1:C1\] README\.md ");
    s.assert_row(4, "   2      - hello");
    s.assert_cell(0, 4, C::new().bg(p.del_bg));
    s.assert_cell(3, 4, C::new().ch('2').fg(p.num).bg(p.del_bg));
    s.assert_cell(10, 4, C::new().ch('-').fg(p.del_mark).bg(p.del_bg).bold(true));
    s.assert_cell(12, 4, C::new().ch('h').bg("default"));
    s.assert_cell(0, 3, C::new().bg(p.cur_bg));
    s.assert_cell(40, 3, C::new().bg(p.cur_bg));

    // char visual selection over `# T`: selected non-cursor cells bg visBg fg visFg
    s.keys("vll");
    s.assert_row_matches(0, r"\[visual L1:C3\] README\.md ");
    s.assert_cell(12, 3, C::new().ch('#').fg(p.vis_fg).bg(p.vis_bg));
    s.assert_cell(13, 3, C::new().ch(' ').fg(p.vis_fg).bg(p.vis_bg));
    s.assert_cell(16, 3, C::new().ch('t').bg(p.cur_bg));
    s.keys("<Esc>");
    s.assert_row_matches(0, r"\[cursor L1:C3\] README\.md ");
    s.assert_cell(12, 3, C::new().ch('#').bg(p.cur_bg));

    // picker: selected row selBg/selFg, border modalBorder, box bg modalBg
    s.keys("f");
    s.assert_contains(" Files (1/4)");
    s.assert_matches(r"│>M README\.md \+2 -1 \*\s+│");
    s.assert_text_cell("README.md +2 -1 *", 0, C::new().ch('R').fg(p.sel_fg).bg(p.sel_bg));
    s.assert_text_cell("README.md +2 -1 *", 4, C::new().ch('M').fg(p.sel_fg).bg(p.sel_bg));
    s.assert_text_cell("╭", 0, C::new().fg(p.border));
    s.assert_text_cell("src/big.ts", 0, C::new().fg(p.modal_fg).bg(p.modal_bg));
    s.keys("<Esc>");
    s.assert_not_contains("Files (");
}

/// F-THEME-02: colorblind theme chrome colors.
#[test]
fn f_theme_02_colorblind() {
    theme_chrome(&Pal {
        name: "colorblind",
        mode: "#f0c674",
        view: "#b48ead",
        accent: "#56b6f7",
        file: "#56b6f7",
        adds: "#5fafff",
        dels: "#ffaf3f",
        dim: "#8a8a8a",
        hunk: "#5fafff",
        num: "#808080",
        add_bg: "#12345a",
        add_mark: "#5fafff",
        del_mark: "#ffaf3f",
        cur_bg: "#5a5a5a",
        del_bg: "#5a3410",
        vis_fg: "#000000",
        vis_bg: "#b8a000",
        sel_fg: "#000000",
        sel_bg: "#56b6f7",
        border: "#56b6f7",
        modal_fg: "#e0e0e0",
        modal_bg: "#000000",
    });
}

/// F-THEME-02: contrast theme chrome colors.
#[test]
fn f_theme_02_contrast() {
    theme_chrome(&Pal {
        name: "contrast",
        mode: "#ffff00",
        view: "#ff00ff",
        accent: "#ffff00",
        file: "#00ffff",
        adds: "#00ff00",
        dels: "#ff0000",
        dim: "#808080",
        hunk: "#ffffff",
        num: "#808080",
        add_bg: "#005f00",
        add_mark: "#ffffff",
        del_mark: "#ffffff",
        cur_bg: "#3a3aa8",
        del_bg: "#870000",
        vis_fg: "#ffffff",
        vis_bg: "#af5f00",
        sel_fg: "#000000",
        sel_bg: "#ffff00",
        border: "#ffffff",
        modal_fg: "#ffffff",
        modal_bg: "#000000",
    });
}

/// F-THEME-02: dull theme chrome colors.
#[test]
fn f_theme_02_dull() {
    theme_chrome(&Pal {
        name: "dull",
        mode: "#b39f80",
        view: "#a8899c",
        accent: "#7f9fa8",
        file: "#9aa5b1",
        adds: "#7f9c7f",
        dels: "#a87f7f",
        dim: "#5f6368",
        hunk: "#7f9fa8",
        num: "#5f6368",
        add_bg: "#26332a",
        add_mark: "#7f9c7f",
        del_mark: "#a87f7f",
        cur_bg: "#3f3f5f",
        del_bg: "#382528",
        vis_fg: "#f0f0f0",
        vis_bg: "#6b5a2e",
        sel_fg: "#d0d0d0",
        sel_bg: "#3a3f47",
        border: "#5f6368",
        modal_fg: "#c0c0c0",
        modal_bg: "#1c1c1c",
    });
}

/// F-THEME-02: light theme chrome colors.
#[test]
fn f_theme_02_light() {
    theme_chrome(&Pal {
        name: "light",
        mode: "#8a5a00",
        view: "#8f1f8f",
        accent: "#00609c",
        file: "#00609c",
        adds: "#0a6b1f",
        dels: "#a01010",
        dim: "#6e6e6e",
        hunk: "#00609c",
        num: "#6a6a6a",
        add_bg: "#d4f0d4",
        add_mark: "#0a6b1f",
        del_mark: "#a01010",
        cur_bg: "#ffe9a0",
        del_bg: "#f8d4d4",
        vis_fg: "#000000",
        vis_bg: "#7fb2ff",
        sel_fg: "#101010",
        sel_bg: "#bcd8ff",
        border: "#303030",
        modal_fg: "#202020",
        modal_bg: "#f4f4f4",
    });
}

/// F-THEME-02: vibrant theme chrome colors (ANSI palette indexes).
#[test]
fn f_theme_02_vibrant() {
    theme_chrome(&Pal {
        name: "vibrant",
        mode: "3",
        view: "5",
        accent: "6",
        file: "6",
        adds: "2",
        dels: "1",
        dim: "8",
        hunk: "6",
        num: "8",
        add_bg: "#1f4d2b",
        add_mark: "10",
        del_mark: "9",
        cur_bg: "#33336b",
        del_bg: "#5a1f26",
        vis_fg: "#ffffff",
        vis_bg: "#875f00",
        sel_fg: "0",
        sel_bg: "6",
        border: "6",
        modal_fg: "#e4e4e4",
        modal_bg: "0",
    });
}
