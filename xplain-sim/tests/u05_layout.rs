//! Scenario tests: layout.
#![allow(clippy::panic, clippy::unwrap_used)]

use xplain_sim::{CellExpect as C, Fixture, Sim};

const HINTS: &str = r"hjkl move  enter ask  J/K comments  \? help$";

/// Cell of `text` searched on `row`, `offset` cells in.
fn at(s: &Sim, row: isize, text: &str, offset: usize, want: C) {
    let p = s.find_in_row(row, text).unwrap_or_else(|| panic!("{text:?} not in row {row}\n{}", s.dump()));
    s.assert_cell(p.x + offset, row, want);
}

/// Row `row` has no box drawing characters.
fn no_box(s: &Sim, row: isize) {
    let r = s.row(row);
    for t in ["╭", "╰", "│"] {
        assert!(!r.contains(t), "row {row} contains {t:?}\n{}", s.dump());
    }
}

fn sim(cols: u16, rows: u16) -> Sim {
    Sim::builder().size(cols, rows).build()
}

fn empty(cols: u16, rows: u16) -> Sim {
    Sim::builder().fixture(Fixture::Empty).size(cols, rows).build()
}

fn commit_base(s: &Sim) {
    s.git(&["add", "-A"]);
    s.git(&["commit", "-qm", "base"]);
}

/// F-HEADER-01: diff header format and solarized chip colors.
#[test]
fn f_header_01_default() {
    let mut s = sim(120, 40);
    s.assert_row(0, "[all] [full] [unified] [solarized] [mcp: off] [1/4] [cursor L2:C1] README.md +2 -1");
    at(&s, 0, "[all]", 1, C::new().fg("#b58900"));
    at(&s, 0, "[full]", 1, C::new().fg("#b58900"));
    at(&s, 0, "[unified]", 1, C::new().fg("#6c71c4"));
    at(&s, 0, "[solarized]", 1, C::new().fg("#6c71c4"));
    at(&s, 0, "[mcp: off]", 1, C::new().fg("#6c71c4"));
    at(&s, 0, "[1/4]", 1, C::new().bold(true));
    at(&s, 0, "README.md", 0, C::new().fg("#268bd2"));
    at(&s, 0, "+2", 1, C::new().fg("#859900"));
    at(&s, 0, "-1", 1, C::new().fg("#dc322f"));
    s.keys("<Tab>");
    s.assert_row(0, "[all] [full] [unified] [solarized] [mcp: off] [2/4] [cursor L30:C1] src/big.ts +1 -1");
    s.keys("<S-Tab><S-Tab>");
    s.assert_row(0, "[all] [full] [unified] [solarized] [mcp: off] [4/4] [cursor L1:C1] x.bin +1 -0");
}

/// F-HEADER-01: diff header chips follow mode, scope, layout, theme (vibrant ANSI colors).
#[test]
fn f_header_01_flags() {
    let mut s = Sim::builder()
        .size(120, 24)
        .args(["--unstaged", "--changes-only", "--split", "--theme", "vibrant"])
        .build();
    s.assert_row(
        0,
        "[unstaged] [changes] [split] [vibrant] [mcp: off] [1/3] [cursor new r1:C1] README.md +2 -1",
    );
    // vibrant: mode yellow (3), view magenta (5), file cyan (6), adds green (2), dels red (1)
    at(&s, 0, "[unstaged]", 1, C::new().fg("3"));
    at(&s, 0, "[changes]", 1, C::new().fg("3"));
    at(&s, 0, "[split]", 1, C::new().fg("5"));
    at(&s, 0, "[vibrant]", 1, C::new().fg("5"));
    at(&s, 0, "[mcp: off]", 1, C::new().fg("5"));
    at(&s, 0, "[1/3]", 1, C::new().bold(true));
    at(&s, 0, "README.md", 0, C::new().fg("6"));
    at(&s, 0, "+2", 1, C::new().fg("2"));
    at(&s, 0, "-1", 1, C::new().fg("1"));
    s.keys("s");
    s.assert_row_matches(
        0,
        r"^\[unstaged\] \[changes\] \[unified\] \[vibrant\] \[mcp: off\] \[1/3\] \[cursor r1:C1\] README\.md \+2 -1$",
    );
}

/// F-HEADER-01: header [mcp: on] only while the server runs.
#[test]
fn f_header_01_mcp() {
    let mut s = sim(120, 24);
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/4\] ");
    s.keys("M<Enter>");
    s.assert_contains("● on");
    s.keys("<Esc>");
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: on\] \[1/4\] ");
    s.keys("M<Enter><Esc>");
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/4\] ");
}

/// F-HEADER-01: renamed file: header path `<old> -> <new>`.
#[test]
fn f_header_01_rename() {
    let mut s = sim(120, 24);
    s.assert_row_contains(0, "[1/4] [cursor L2:C1] README.md +2 -1");
    s.git(&["mv", "src/big.ts", "src/huge.ts"]);
    s.keys("r");
    // git order after the rename is README.md, src/c.tsx, src/big.ts -> src/huge.ts, x.bin
    s.keys("<Tab><Tab>");
    s.assert_row_matches(
        0,
        r"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[\d/4\] \[cursor L30:C1\] src/big\.ts -> src/huge\.ts \+1 -1$",
    );
}

/// F-HEADER-02: browse header shows [mcp: on] while the server runs.
#[test]
fn f_header_02_browse_mcp() {
    let mut s =
        Sim::builder().size(120, 24).config_json(serde_json::json!({"mcp": {"autostart": true}})).build();
    s.keys("F");
    s.keys("README.md<Enter>");
    s.assert_row(0, "[browse] [solarized] [mcp: on] [cursor L1:C1] README.md");
    s.keys("j");
    s.assert_row(0, "[browse] [solarized] [mcp: on] [cursor L2:C1] README.md");
}

/// F-HEADER-02: browse header: [browse] [theme] [mcp] cursortag path; no scope/layout/index/counts.
#[test]
fn f_header_02_browse() {
    let mut s = sim(120, 24);
    s.assert_row(0, "[all] [full] [unified] [solarized] [mcp: off] [1/4] [cursor L2:C1] README.md +2 -1");
    s.keys("F");
    s.keys("package.json<Enter>");
    s.assert_row(0, "[browse] [solarized] [mcp: off] [cursor L1:C1] package.json");
    at(&s, 0, "[browse]", 1, C::new().fg("#b58900"));
    at(&s, 0, "[solarized]", 1, C::new().fg("#6c71c4"));
    at(&s, 0, "[mcp: off]", 1, C::new().fg("#6c71c4"));
    at(&s, 0, "package.json", 0, C::new().fg("#268bd2"));
    s.keys("t");
    s.assert_row(0, "[browse] [vibrant] [mcp: off] [cursor L1:C1] package.json");
    s.keys("<Esc>");
    s.assert_row_matches(
        0,
        r"^\[all\] \[full\] \[unified\] \[vibrant\] \[mcp: off\] \[1/4\] \[cursor L2:C1\] README\.md \+2 -1$",
    );
}

/// F-HEADER-03: cursor tag in browse: L<line no>:C<col>; no pane prefix even with split on.
#[test]
fn f_header_03_browse() {
    let mut s = Sim::builder().size(120, 24).args(["--split"]).build();
    s.assert_row_contains(0, "[cursor new L2:C1] README.md");
    s.keys("F");
    s.keys("README.md<Enter>");
    s.assert_row(0, "[browse] [solarized] [mcp: off] [cursor L1:C1] README.md");
    s.keys("j$");
    s.assert_row(0, "[browse] [solarized] [mcp: off] [cursor L2:C11] README.md");
    s.keys("V");
    s.assert_row(0, "[browse] [solarized] [mcp: off] [visual L2:C11] README.md");
}

/// F-HEADER-03: no cursor tag on the no-changes screen; tag appears once changes exist.
#[test]
fn f_header_03_nochanges() {
    let mut s = empty(120, 24);
    s.assert_row(0, "[all] [mcp: off] No changes (m cycles mode, F search, q quits)");
    s.assert_not_contains("[cursor");
    s.write_file("n.txt", "x\n");
    s.keys("r");
    s.assert_row_matches(
        0,
        r"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/1\] \[cursor L1:C1\] n\.txt \+1 -0$",
    );
}

/// F-HEADER-03: cursor tag on a note row (binary file) uses r<row>.
#[test]
fn f_header_03_note_row() {
    let s = Sim::builder().size(120, 24).fixture(Fixture::Empty).file_bytes("b.bin", b"a\0b\n").build();
    s.assert_contains("Binary file");
    s.assert_row_matches(0, r"\[1/1\] \[cursor r1:C1\] b\.bin \+0 -0$");
}

/// F-HEADER-03: cursor tag in split: old/new pane prefix; pane old uses left old no (empty left: r<n>); pane new
/// uses right new no, else left old no.
#[test]
fn f_header_03_split() {
    let mut s = Sim::builder()
        .size(120, 24)
        .fixture(Fixture::Empty)
        .args(["--split"])
        .file("p.txt", "ctx1\nold2\nold3\nctx4\nold5\nctx6\n")
        .build();
    commit_base(&s);
    s.write_file("p.txt", "ctx1\nnew2\nctx4\nnew4a\nnew4b\nctx6\n");
    s.keys("r");
    // split rows: 1 hunk, 2 ctx1 1|1, 3 old2 2|new2 2, 4 old3 3|empty, 5 ctx4 4|3, 6 old5 5|new4a 4,
    // 7 empty|new4b 5, 8 ctx6 6|6
    s.keys("g");
    s.assert_row_contains(0, "[cursor new r1:C1] p.txt");
    s.keys("j");
    s.assert_row_contains(0, "[cursor new L1:C1] p.txt");
    s.keys("3j");
    // context row, pane new shows right (new) number
    s.assert_row_contains(0, "[cursor new L3:C1] p.txt");
    s.keys("p");
    // pane old shows left (old) number
    s.assert_row_contains(0, "[cursor old L4:C1] p.txt");
    s.keys("2j");
    // pane old on row with empty left side
    s.assert_row_contains(0, "[cursor old r7:C1] p.txt");
    s.keys("p");
    s.assert_row_contains(0, "[cursor new L5:C1] p.txt");
    s.keys("3k");
    // pane prefix on del-only split row with pane new is UNSPEC-20; only the number part is asserted
    // pane new on row with empty right side falls back to left old no
    s.assert_row_matches(0, r"\[cursor (old |new )?L3:C1\] p\.txt");
    s.keys("kv");
    s.assert_row_contains(0, "[visual new L2:C1] p.txt");
    s.keys("p");
    // p ends the selection and switches pane
    s.assert_row_contains(0, "[cursor old L2:C1] p.txt");
}

/// F-HEADER-03: cursor tag unified: L<new no> (del row: old no), r<row> on hunk row, :C<col+1> with clamped column,
/// [visual ...] while selecting; accent bold.
#[test]
fn f_header_03_unified() {
    let mut s = Sim::builder()
        .size(120, 24)
        .fixture(Fixture::Empty)
        .file("p.txt", "ctx1\nold2\nold3\nctx4\nold5\nctx6\n")
        .build();
    commit_base(&s);
    s.write_file("p.txt", "ctx1\nnew2\nctx4\nnew4a\nnew4b\n\nctx6\n");
    s.keys("r");
    // rows: 1 hunk, 2 ctx1 (1/1), 3 -old2 (2), 4 -old3 (3), 5 +new2 (2), 6 ctx4 (4/3), 7 -old5 (5), 8 +new4a (4),
    // 9 +new4b (5), 10 + empty (6), 11 ctx6 (6/7)
    s.keys("g");
    s.assert_row_contains(0, "[1/1] [cursor r1:C1] p.txt ");
    at(&s, 0, "[cursor", 1, C::new().fg("#cb4b16").bold(true));
    // column is tracked on the hunk row too
    s.keys("3l");
    s.assert_row_contains(0, "[cursor r1:C4] p.txt");
    s.keys("0j");
    s.assert_row_contains(0, "[cursor L1:C1] p.txt");
    s.keys("2j");
    // del row shows its old number
    s.assert_row_contains(0, "[cursor L3:C1] p.txt");
    s.keys("j");
    s.assert_row_contains(0, "[cursor L2:C1] p.txt");
    // context row shows new number (old 4, new 3)
    s.keys("j");
    s.assert_row_contains(0, "[cursor L3:C1] p.txt");
    s.keys("2j$");
    s.assert_row_contains(0, "[cursor L4:C5] p.txt");
    // longer row keeps $ at its end
    s.keys("j");
    s.assert_row_contains(0, "[cursor L5:C5] p.txt");
    // empty line shows C1
    s.keys("j");
    s.assert_row_contains(0, "[cursor L6:C1] p.txt");
    s.keys("kh");
    s.assert_row_contains(0, "[cursor L5:C4] p.txt");
    // shown column is clamped on the empty row, desired column kept
    s.keys("j");
    s.assert_row_contains(0, "[cursor L6:C1] p.txt");
    s.keys("k");
    s.assert_row_contains(0, "[cursor L5:C4] p.txt");
    s.keys("v");
    s.assert_row_contains(0, "[visual L5:C4] p.txt");
    at(&s, 0, "[visual", 1, C::new().fg("#cb4b16").bold(true));
    s.keys("h");
    s.assert_row_contains(0, "[visual L5:C3] p.txt");
    s.keys("v");
    s.assert_row_contains(0, "[cursor L5:C3] p.txt");
}

/// F-LAYOUT-01: frame at 120x40: header row 0, dim rule of cols-1 dashes on row 1, viewport H=R-3=37 rows, dim
/// footer on last row.
#[test]
fn f_layout_01_frame() {
    let mut s = sim(120, 40);
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] ");
    s.assert_row_matches(1, r"^─{119}$");
    s.assert_row_matches(2, r"^@@ -1,2 \+1,3 @@$");
    s.assert_row_matches(-1, &format!(r"^\(1-5/5\) {HINTS}"));
    s.assert_cell(0, 1, C::new().ch('─').fg("#586e75"));
    s.assert_cell(118, 1, C::new().ch('─').fg("#586e75"));
    s.assert_cell(0, -1, C::new().ch('(').fg("#586e75"));
    // src/big.ts has 62 rows; g puts the hunk row on top, viewport shows rows 1..37
    s.keys("<Tab>g");
    s.assert_row_matches(0, r"\[2/4\] \[cursor r1:C1\] src/big\.ts ");
    s.assert_row_matches(1, r"^─{119}$");
    s.assert_row_matches(2, r"^@@ ");
    s.assert_row_matches(3, r"^   1    1   const v1 = 1;$");
    s.assert_row_matches(38, r"^  35   35   const v35 = 1;$");
    s.assert_row_matches(39, &format!(r"^\(1-37/62\) {HINTS}"));
    s.keys("G");
    s.assert_row_matches(2, r"^  25   25   const v25 = 1;$");
    s.assert_row_matches(-1, r"^\(26-62/62\) ");
}

/// F-LAYOUT-01: frame at 80x6 (smallest specified): H=max(3,R-3)=3 viewport rows, footer on row 5.
#[test]
fn f_layout_01_min() {
    let mut s = sim(80, 6);
    s.keys("<Tab>g");
    s.assert_row_matches(0, r"^\[all\] ");
    s.assert_row_matches(1, r"^─{79}$");
    s.assert_row_matches(2, r"^@@ ");
    s.assert_row_matches(4, r"^   2    2   const v2 = 1;$");
    s.assert_row_matches(5, r"^\(1-3/62\) hjkl move");
    s.keys("j");
    s.assert_row_matches(0, r"\[cursor L1:C1\] ");
    s.assert_row_matches(5, r"^\(1-3/62\) ");
    // margin min(2, floor((3-1)/2)) = 1; cursor row 3 needs one row below
    s.keys("j");
    s.assert_row_matches(0, r"\[cursor L2:C1\] ");
    s.assert_row_matches(2, r"^   1    1   const v1 = 1;$");
    s.assert_row_matches(5, r"^\(2-4/62\) ");
}

/// F-LAYOUT-01: frame at 80x8: rule 79 dashes, viewport H=5 (rows 2..6), footer on row 7.
#[test]
fn f_layout_01_small() {
    let mut s = sim(80, 8);
    s.keys("<Tab>g");
    s.assert_row_matches(0, r"^\[all\] ");
    s.assert_row_matches(1, r"^─{79}$");
    s.assert_row_matches(2, r"^@@ ");
    s.assert_row_matches(6, r"^   4    4   const v4 = 1;$");
    s.assert_row_matches(7, &format!(r"^\(1-5/62\) {HINTS}"));
    s.keys("G");
    s.assert_row_matches(2, r"^  56   56   const v56 = 1;$");
    s.assert_row_matches(6, r"^  60   60  ▶const v60 = 1; ");
    s.assert_row_matches(7, r"^\(58-62/62\) ");
}

/// F-LAYOUT-02: footer (a-b/n): a=top+1, b=min(n, top+H), n=rows of view.
#[test]
fn f_layout_02_count() {
    let mut s = sim(80, 10);
    // H=7; README.md has 5 rows, b clamps to n
    s.assert_row_matches(-1, r"^\(1-5/5\) ");
    s.keys("<Tab>g");
    s.assert_row_matches(-1, r"^\(1-7/62\) ");
    s.keys("10j");
    // cursor row 11, margin 2 keeps row 13 visible, top 6
    s.assert_row_matches(-1, r"^\(7-13/62\) ");
}

/// F-LAYOUT-02: footer: find input open shows /<text>█ glued to (a-b/n); after Enter the active term shows as
/// /<term> | .
#[test]
fn f_layout_02_find_open() {
    let mut s = sim(80, 24);
    s.assert_row_matches(-1, &format!(r"^\(1-5/5\) {HINTS}"));
    s.keys("/");
    s.assert_row_matches(-1, &format!(r"^/█\(1-5/5\) {HINTS}"));
    s.keys("hel");
    s.assert_row_matches(-1, &format!(r"^/hel█\(1-5/5\) {HINTS}"));
    s.keys("<Enter>");
    s.assert_row_matches(-1, &format!(r"^/hel \| \(1-5/5\) {HINTS}"));
    // while find is open again the term part is replaced by the input
    s.keys("/x");
    s.assert_row_matches(-1, r"^/x█\(1-5/5\) hjkl move");
    s.keys("<Esc>");
    s.assert_row_matches(-1, r"^/hel \| \(1-5/5\) hjkl move");
}

/// F-LAYOUT-02: footer: goto input open shows :<text>█; a note shows as <note> | before (a-b/n).
#[test]
fn f_layout_02_goto_open() {
    let mut s = sim(80, 24);
    s.keys(":");
    s.assert_row_matches(-1, &format!(r"^:█\(1-5/5\) {HINTS}"));
    s.keys("abc");
    s.assert_row_matches(-1, &format!(r"^:abc█\(1-5/5\) {HINTS}"));
    s.keys("<Enter>");
    s.assert_row_matches(-1, &format!(r"^not a line number: abc \| \(1-5/5\) {HINTS}"));
    // goto open replaces the note part too
    s.keys(":1");
    s.assert_row_matches(-1, r"^:1█\(1-5/5\) hjkl move");
}

/// F-LAYOUT-02: footer: too narrow for split also shown in browse; (a-b/n) counts browse rows.
#[test]
fn f_layout_02_narrow_browse() {
    let mut s = Sim::builder().size(80, 24).args(["--split"]).build();
    s.keys("F");
    s.keys("README.md<Enter>");
    s.assert_row_matches(0, r"^\[browse\] ");
    s.assert_row_matches(-1, &format!(r"^too narrow for split \| \(1-3/3\) {HINTS}"));
}

/// F-LAYOUT-02: footer: too narrow for split also on the no-changes screen.
#[test]
fn f_layout_02_narrow_nochanges() {
    let s = Sim::builder().size(80, 24).fixture(Fixture::Empty).args(["--split"]).build();
    s.assert_row_contains(0, "No changes");
    s.assert_row_matches(-1, &format!(r"^too narrow for split \| \(0-0/0\) {HINTS}"));
}

/// F-LAYOUT-02: footer: too narrow for split only when split setting on and cols < 100; s toggles it.
#[test]
fn f_layout_02_narrow() {
    let mut s = sim(99, 24);
    s.assert_row_matches(-1, &format!(r"^\(1-5/5\) {HINTS}"));
    s.keys("s");
    s.assert_row_matches(-1, &format!(r"^too narrow for split \| \(1-5/5\) {HINTS}"));
    s.keys("s");
    s.assert_row_matches(-1, &format!(r"^\(1-5/5\) {HINTS}"));
}

/// F-LAYOUT-02: footer note: replaced by the next note; cleared by a successful config save.
#[test]
fn f_layout_02_note() {
    let mut s = sim(80, 24);
    s.keys(":abc<Enter>");
    s.assert_row_matches(-1, r"^not a line number: abc \| \(1-5/5\) hjkl move");
    s.keys(":xyz<Enter>");
    s.assert_row_matches(-1, r"^not a line number: xyz \| \(1-5/5\) hjkl move");
    // select the current theme in the config modal (saves config)
    s.keys("C<Enter>");
    let v: serde_json::Value = serde_json::from_str(&s.file("${CONFIG}/xplain/config.json")).unwrap();
    assert_eq!(v["theme"], "solarized");
    s.keys("<Esc>");
    s.assert_row_matches(-1, &format!(r"^\(1-5/5\) {HINTS}"));
}

/// F-LAYOUT-02: footer order: /<term> | <note> | too narrow for split | (a-b/n) hints.
#[test]
fn f_layout_02_order() {
    let mut s = Sim::builder().size(99, 24).args(["--split"]).build();
    s.assert_row_matches(-1, r"^too narrow for split \| \(1-5/5\) hjkl move  enter ask");
    s.keys("/zzz<Enter>");
    s.assert_row_matches(
        -1,
        r"^/zzz \| pattern not found: zzz \| too narrow for split \| \(1-5/5\) hjkl move  enter ask",
    );
    // find open replaces term + note, narrow part stays
    s.keys("/q");
    s.assert_row_matches(-1, r"^/q█too narrow for split \| \(1-5/5\) hjkl move  enter ask");
    s.keys("<Esc>:7");
    s.assert_row_matches(-1, r"^:7█too narrow for split \| \(1-5/5\) ");
}

/// F-LAYOUT-02: footer at cols 100 with split: no narrow part, split hints, n = 4 paired split rows.
#[test]
fn f_layout_02_wide() {
    let s = Sim::builder().size(100, 24).args(["--split"]).build();
    s.assert_row_matches(-1, r"^\(1-4/4\) hjkl move  enter ask  J/K comments  p pane  \? help$");
    s.assert_not_contains("too narrow");
}

/// F-LAYOUT-03: unified row colors (solarized): hunk fg, gutter numbers, bold marks, add/del bg on number+mark cells
/// only, context no bg.
#[test]
fn f_layout_03_colors() {
    let mut s = sim(80, 24);
    // move cursor to last row so rows 2-5 carry no cursor background
    s.keys("G");
    s.assert_row_contains(0, "[cursor L3:C1]");
    s.assert_cell(0, 2, C::new().ch('@').fg("#2aa198"));
    // context row
    s.assert_cell(3, 3, C::new().ch('1').fg("#586e75").bg("default"));
    s.assert_cell(8, 3, C::new().ch('1').fg("#586e75").bg("default"));
    s.assert_cell(12, 3, C::new().ch('#').bg("default"));
    // del row
    s.assert_cell(3, 4, C::new().ch('2').fg("#586e75").bg("#4a1a1f"));
    s.assert_cell(10, 4, C::new().ch('-').fg("#dc322f").bg("#4a1a1f").bold(true));
    s.assert_cell(12, 4, C::new().ch('h').bg("default"));
    // add row
    s.assert_cell(8, 5, C::new().ch('2').fg("#586e75").bg("#0b3b1f"));
    s.assert_cell(2, 5, C::new().ch(' ').bg("#0b3b1f"));
    s.assert_cell(10, 5, C::new().ch('+').fg("#859900").bg("#0b3b1f").bold(true));
    s.assert_cell(12, 5, C::new().ch('h').bg("default"));
}

/// F-LAYOUT-03: unified rows: hunk header row then OOOO NNNN mark cursor code; cursor cell ▶ moves with cursor.
#[test]
fn f_layout_03_rows() {
    let mut s = sim(80, 24);
    s.assert_row_matches(2, r"^@@ -1,2 \+1,3 @@$");
    s.assert_row_matches(3, r"^   1    1   # Title$");
    s.assert_row_matches(4, r"^   2      -▶hello");
    s.assert_row_matches(5, r"^        2 \+ hello world$");
    s.assert_row_matches(6, r"^        3 \+ more$");
    s.keys("G");
    s.assert_row_matches(3, r"^   1    1   # Title$");
    s.assert_row_matches(4, r"^   2      - hello$");
    s.assert_row_matches(5, r"^        2 \+ hello world$");
    s.assert_row_matches(6, r"^        3 \+▶more");
}

/// F-LAYOUT-03: unified rows: tab shown as 2 spaces; long line cut at screen edge with … in last cell (no wrap).
#[test]
fn f_layout_03_tabs_long() {
    let s = Sim::builder()
        .size(80, 24)
        .fixture(Fixture::Empty)
        .file(
            "t.txt",
            "first\n\tx\tb\n0123456789abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789END\nlast\n",
        )
        .build();
    s.assert_row_contains(0, "[cursor L1:C1] t.txt +4 -0");
    s.assert_row_matches(2, r"^@@ -0,0 \+1,4 @@$");
    s.assert_row_matches(3, r"^        1 \+▶first");
    s.assert_row_matches(4, r"^        2 \+   x  b$");
    s.assert_row_matches(
        5,
        r"^        3 \+ 0123456789abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJKLMNOPQRSTU…$",
    );
    s.assert_row_matches(6, r"^        4 \+ last$");
}

/// F-LAYOUT-03: unified rows: line numbers > 9999 are not cut, that row widens.
#[test]
fn f_layout_03_wide_numbers() {
    let mut s = empty(90, 24);
    let lines: String = (1..=10001).map(|n| format!("{n}\n")).collect();
    s.write_file("n.txt", &lines);
    s.keys("r");
    s.assert_row_contains(0, "[cursor L1:C1] n.txt +10001 -0");
    s.keys(":9999<Enter>");
    s.assert_row_contains(0, "[cursor L9999:C1] n.txt");
    s.assert_row_matches(-4, r"^     9999 \+▶9999");
    s.assert_row_matches(-3, r"^     10000 \+ 10000$");
    s.keys("j");
    s.assert_row_contains(0, "[cursor L10000:C1] n.txt");
    s.assert_row_matches(-3, r"^     10000 \+▶10000");
    s.assert_row_matches(-2, r"^     10001 \+ 10001$");
}

/// F-LAYOUT-04: s is ignored in browse; split setting unchanged when back in diff.
#[test]
fn f_layout_04_browse() {
    let mut s = Sim::builder().size(120, 24).args(["--split"]).build();
    s.keys("F");
    s.keys("README.md<Enter>");
    s.assert_not_contains("│");
    s.assert_row_matches(0, r"^\[browse\] ");
    s.assert_row_matches(2, r"^   1  ▶# Title");
    s.keys("s");
    s.assert_not_contains("│");
    s.assert_row_matches(0, r"^\[browse\] ");
    s.assert_row_matches(2, r"^   1  ▶# Title");
    s.assert_row_matches(3, r"^   2   hello world$");
    s.keys("<Esc>");
    s.assert_row_matches(0, r"^\[all\] \[full\] \[split\] ");
    s.assert_row_matches(3, r"^   1   # Title +│   1   # Title$");
}

/// F-LAYOUT-04: --split at cols 99: not effective, unified rows, no pane prefix in cursor tag.
#[test]
fn f_layout_04_narrow_start() {
    let s = Sim::builder().size(99, 24).args(["--split"]).build();
    s.assert_not_contains("│");
    s.assert_row_contains(0, "[cursor L2:C1] README.md");
    s.assert_row_matches(3, r"^   1    1   # Title$");
    s.assert_row_matches(5, r"^        2 \+ hello world$");
    s.assert_row_matches(-1, r"^too narrow for split \| \(1-5/5\) ");
}

/// F-LAYOUT-04: s at cols < 100: split setting on but unified rendering, rows unchanged, cursor kept, footer too
/// narrow.
#[test]
fn f_layout_04_narrow() {
    let mut s = sim(99, 24);
    s.keys("G");
    s.assert_row_contains(0, "[cursor L3:C1] README.md");
    s.assert_row_matches(6, r"^        3 \+▶more");
    s.keys("s");
    // chip [split]/[unified] at cols < 100 is UNSPEC-3, not asserted
    s.assert_not_contains("│");
    s.assert_row_contains(0, "[cursor L3:C1] README.md");
    s.assert_row_matches(3, r"^   1    1   # Title$");
    s.assert_row_matches(4, r"^   2      - hello$");
    s.assert_row_matches(5, r"^        2 \+ hello world$");
    s.assert_row_matches(6, r"^        3 \+▶more");
    s.assert_row_matches(-1, &format!(r"^too narrow for split \| \(1-5/5\) {HINTS}"));
    s.keys("s");
    s.assert_row_matches(0, r"\[unified\] .*\[cursor L3:C1\] README\.md");
    s.assert_row_matches(6, r"^        3 \+▶more");
    s.assert_row_matches(-1, r"^\(1-5/5\) hjkl move");
}

/// F-LAYOUT-04: split pairing: i-th del pairs with i-th add, surplus leaves other side empty, context both sides
/// with own numbers, hunk row full width, dim │ at col W=floor((cols-1)/2).
#[test]
fn f_layout_04_pairing() {
    let mut s = Sim::builder()
        .size(120, 24)
        .fixture(Fixture::Empty)
        .args(["--split"])
        .file("p.txt", "ctx1\nold2\nold3\nctx4\nold5\nctx6\n")
        .build();
    commit_base(&s);
    s.write_file("p.txt", "ctx1\nnew2\nctx4\nnew4a\nnew4b\nctx6\n");
    s.keys("r");
    // G puts the cursor on the last row so rows 2-8 carry no cursor
    s.keys("G");
    s.assert_row_matches(0, r"^\[all\] \[full\] \[split\] .* p\.txt \+3 -3$");
    s.assert_row_matches(2, r"^@@ -1,6 \+1,6 @@$");
    s.assert_row_matches(3, r"^   1   ctx1 {48}│   1   ctx1$");
    s.assert_row_matches(4, r"^   2 - old2 {48}│   2 \+ new2$");
    s.assert_row_matches(5, r"^   3 - old3 {48}│$");
    s.assert_row_matches(6, r"^   4   ctx4 {48}│   3   ctx4$");
    s.assert_row_matches(7, r"^   5 - old5 {48}│   4 \+ new4a$");
    s.assert_row_matches(8, r"^ {59}│   5 \+ new4b$");
    s.assert_row_matches(9, r"^   6  ▶ctx6 +.?│   6  ▶ctx6");
    s.assert_row_matches(-1, r"\(1-8/8\) hjkl move  enter ask  J/K comments  p pane  \? help$");
    s.assert_cell(59, 3, C::new().ch('│').fg("#586e75"));
    s.assert_cell(5, 4, C::new().ch('-').fg("#dc322f").bg("#4a1a1f").bold(true));
    s.assert_cell(65, 4, C::new().ch('+').fg("#859900").bg("#0b3b1f").bold(true));
    // back to unified the same diff has one row per line; effective layout changed, so cursor resets to the first
    // change (F-NAV-08)
    s.keys("s");
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] .*\[cursor L2:C1\] p\.txt");
    s.assert_row_matches(4, r"^   2      -▶old2");
    s.assert_row_matches(5, r"^   3      - old3$");
    s.assert_row_matches(6, r"^        2 \+ new2$");
    s.assert_row_matches(7, r"^   4    3   ctx4$");
    s.assert_row_matches(-1, r"\(1-10/10\) hjkl move  enter ask  J/K comments  \? help$");
}

/// F-LAYOUT-04: split effective from cols 100: W = floor(99/2) = 49, separator at col 49.
#[test]
fn f_layout_04_threshold() {
    let s = Sim::builder().size(100, 24).args(["--split"]).build();
    s.assert_row_matches(0, r"^\[all\] \[full\] \[split\] .*\[cursor new L2:C1\] README\.md ");
    s.assert_row_matches(3, r"^   1   # Title {35}│   1   # Title$");
    s.assert_row_matches(5, r"^ {49}│   3 \+ more$");
    s.assert_cell(49, 3, C::new().ch('│').fg("#586e75"));
}

/// F-LAYOUT-04: s at cols >= 100 toggles split/unified: header chip, pane layout, split hints, cursor reset to
/// first change.
#[test]
fn f_layout_04_toggle() {
    let mut s = sim(120, 24);
    s.keys("G");
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] .*\[cursor L3:C1\] README\.md ");
    s.assert_row_matches(3, r"^   1    1   # Title$");
    s.assert_row_matches(-1, &format!(r"^\(1-5/5\) {HINTS}"));
    s.keys("s");
    s.assert_row_matches(0, r"^\[all\] \[full\] \[split\] .*\[cursor new L2:C1\] README\.md ");
    s.assert_row_matches(2, r"^@@ -1,2 \+1,3 @@$");
    s.assert_row_matches(3, r"^   1   # Title {45}│   1   # Title$");
    s.assert_row_matches(4, r"^   2 -▶hello .*│   2 \+▶hello world");
    s.assert_row_matches(5, r"^ {59}│   3 \+ more$");
    s.assert_row_matches(-1, r"^\(1-4/4\) hjkl move  enter ask  J/K comments  p pane  \? help$");
    s.assert_cell(59, 3, C::new().ch('│').fg("#586e75"));
    s.keys("G");
    s.assert_row_contains(0, "[cursor new L3:C1]");
    s.keys("s");
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] .*\[cursor L2:C1\] README\.md ");
    s.assert_row_matches(3, r"^   1    1   # Title$");
    s.assert_row_matches(4, r"^   2      -▶hello");
    s.assert_row_matches(-1, &format!(r"^\(1-5/5\) {HINTS}"));
}

/// F-LAYOUT-04: split panes: NNNN cell, mark, cursor cell, code; each pane cut at its own width with … in its last
/// cell.
#[test]
fn f_layout_04_truncate() {
    let mut s = Sim::builder()
        .size(101, 24)
        .fixture(Fixture::Empty)
        .args(["--split"])
        .file("w.txt", "a\n0123456789abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ\nz\n")
        .build();
    commit_base(&s);
    s.write_file("w.txt", "a\n0123456789abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ\nZ\n");
    s.keys("r");
    s.keys("G");
    // W = floor(100/2) = 50; pane text 50 cells = 7 prefix cells + 42 code chars + …
    s.assert_row_contains(0, "[cursor new L3:C1] w.txt +1 -1");
    s.assert_row_matches(3, r"^   1   a {42}│   1   a$");
    s.assert_row_matches(
        4,
        r"^   2   0123456789abcdefghijklmnopqrstuvwxyz012345…│   2   0123456789abcdefghijklmnopqrstuvwxyz012345…$",
    );
    s.assert_cell(49, 4, C::new().ch('…'));
    s.assert_cell(50, 4, C::new().ch('│'));
}

/// F-LAYOUT-04: s at cols >= 100 resets cursor to first change and top to max(0, firstChange-3) (F-NAV-08).
#[test]
fn f_layout_04_viewport_wide() {
    let mut s = sim(120, 12);
    s.keys("<Tab>g");
    s.assert_row_contains(0, "[cursor r1:C1] src/big.ts");
    s.assert_row_matches(-1, r"^\(1-9/62\) ");
    s.keys("s");
    // split has 61 rows (del/add of line 30 paired on row 31)
    s.assert_row_contains(0, "[split] [solarized] [mcp: off] [2/4] [cursor new L30:C1] src/big.ts");
    s.assert_row_matches(5, r"^  30 -▶const v30 = 1; .*│  30 \+▶const v30 = 2;");
    s.assert_row_matches(-1, r"^\(28-36/61\) hjkl move  enter ask  J/K comments  p pane  \? help$");
}

/// F-LAYOUT-04: s viewport: cols < 100 keeps cursor, top reset to 0 then follow (F-NAV-10); cols >= 100 cursor +
/// top per F-NAV-08.
#[test]
fn f_layout_04_viewport() {
    let mut s = sim(99, 12);
    s.keys("<Tab>G30k");
    // H=9; moving up leaves cursor row 32 two rows below top
    s.assert_row_contains(0, "[cursor L30:C1] src/big.ts");
    s.assert_row_matches(-1, r"^\(30-38/62\) ");
    s.keys("s");
    s.assert_row_contains(0, "[cursor L30:C1] src/big.ts");
    s.assert_row_matches(-1, r"^too narrow for split \| \(26-34/62\) ");
}

/// F-LAYOUT-05: browse rows: single pane NNNN, mark space, cursor cell, code; no hunk row.
#[test]
fn f_layout_05_browse_rows() {
    let mut s = sim(80, 24);
    s.assert_row_matches(2, r"^@@ ");
    s.keys("F");
    s.keys("README.md<Enter>");
    s.assert_not_contains("@@");
    s.assert_row_matches(0, r"^\[browse\] ");
    s.assert_row_matches(2, r"^   1  ▶# Title");
    s.assert_row_matches(3, r"^   2   hello world$");
    s.assert_row_matches(4, r"^   3   more$");
    s.assert_row(5, "");
    s.assert_row_matches(-1, r"^\(1-3/3\) ");
    s.assert_cell(3, 3, C::new().ch('2').fg("#586e75"));
    s.keys("j");
    s.assert_row_matches(2, r"^   1   # Title$");
    s.assert_row_matches(3, r"^   2  ▶hello world");
}

/// F-LAYOUT-05: browse rows: tabs as 2 spaces, long line cut with … (browse of an unchanged file).
#[test]
fn f_layout_05_browse_tabs() {
    let mut s = Sim::builder()
        .size(80, 24)
        .fixture(Fixture::Empty)
        .file(
            "t.txt",
            "first\n\tx\tb\n0123456789abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789END\n",
        )
        .build();
    commit_base(&s);
    s.keys("r");
    s.assert_row_contains(0, "No changes");
    s.keys("F");
    s.keys("t.txt<Enter>");
    // prefix is 7 cells, so 72 code chars + … fill 80 columns
    s.assert_row_matches(0, r"^\[browse\] .*t\.txt$");
    s.assert_row_matches(2, r"^   1  ▶first");
    s.assert_row_matches(3, r"^   2     x  b$");
    s.assert_row_matches(
        4,
        r"^   3   0123456789abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ…$",
    );
}

/// F-LAYOUT-06: picker centered at 120x40: width floor(120*0.7)=84 at col 18, height 8 at row 16.
#[test]
fn f_layout_06_center_120() {
    let mut s = sim(120, 40);
    s.keys("f");
    no_box(&s, 15);
    s.assert_row_matches(16, r"^.{18}╭─{82}╮");
    s.assert_row_matches(17, r"^.{18}│ Files \(1/4\) +│$");
    s.assert_row_matches(23, r"^.{18}╰─{82}╯");
    no_box(&s, 24);
}

/// F-LAYOUT-06: delete comment modal (25x3) centered at 81x25.
#[test]
fn f_layout_06_center_delete() {
    let mut s = sim(81, 25);
    s.keys("ahi<Enter>");
    s.keys("J");
    s.assert_not_contains("Delete comment?");
    s.keys("D");
    s.assert_row_matches(11, r"^.{28}╭─{23}╮");
    s.assert_row_matches(12, r"^.{28}│ Delete comment\? \(y/n\) │$");
    s.assert_row_matches(13, r"^.{28}╰─{23}╯");
}

/// F-LAYOUT-06: MCP (64 wide, 11 high) and quit (22x3) modals centered at 80x25.
#[test]
fn f_layout_06_center_small() {
    let mut s = sim(80, 25);
    s.keys("M");
    no_box(&s, 6);
    s.assert_row_matches(7, r"^.{8}╭─{62}╮");
    s.assert_row_matches(8, r"^.{8}│ MCP +│$");
    s.assert_row_matches(17, r"^.{8}╰─{62}╯");
    no_box(&s, 18);
    s.keys("<Esc>");
    s.assert_not_contains("MCP");
    s.keys("q");
    no_box(&s, 10);
    s.assert_row_matches(11, r"^.{29}╭─{20}╮");
    s.assert_row_matches(12, r"^.{29}│ Quit xplain\? \(y/n\) │$");
    s.assert_row_matches(13, r"^.{29}╰─{20}╯");
    no_box(&s, 14);
}

/// F-LAYOUT-06: modals centered horizontally and vertically at 80x24: picker (56x8), search (56x16), config
/// (56x10).
#[test]
fn f_layout_06_center() {
    let mut s = sim(80, 24);
    s.keys("f");
    no_box(&s, 7);
    s.assert_row_matches(8, r"^.{12}╭─{54}╮");
    s.assert_row_matches(9, r"^.{12}│ Files \(1/4\) +│$");
    s.assert_row_matches(15, r"^.{12}╰─{54}╯");
    no_box(&s, 16);
    s.keys("<Esc>");
    s.assert_not_contains("╭");
    s.assert_not_contains("Files (");
    s.keys("F");
    s.assert_row_matches(4, r"^.{12}╭─{54}╮");
    s.assert_row_matches(5, r"^.{12}│ Search \(1/6\) +│$");
    s.assert_row_matches(19, r"^.{12}╰─{54}╯");
    no_box(&s, 20);
    s.keys("<Esc>");
    s.keys("C");
    s.assert_row_matches(7, r"^.{12}╭─{54}╮");
    s.assert_row_matches(8, r"^.{12}│ Config +│$");
    s.assert_row_matches(16, r"^.{12}╰─{54}╯");
    no_box(&s, 17);
}

/// F-LAYOUT-06: help panel is drawn over a modal it overlaps (search box at rows 2-17, help bottom-right).
#[test]
fn f_layout_06_help_over() {
    let mut s = sim(80, 24);
    s.keys("F");
    s.assert_row_matches(4, r"^.{12}╭─{54}╮");
    s.assert_row_matches(18, r"^.{12}│ ↑↓/\^n\^p move enter open esc close +│$");
    s.keys("<Esc>");
    s.assert_not_contains("Search (");
    s.keys("?F");
    s.assert_row_matches(2, r"^.{12}╭─{54}╮");
    s.assert_row_matches(3, r"^.{12}│ Search \(1/6\) +│$");
    s.assert_row_matches(14, r"^.{12}│ +│ Help · File search +│$");
    s.assert_row_matches(16, r"^.{12}│ ↑↓/\^n\^p move enter open esc close │  type +filter files +│$");
    s.assert_row_matches(17, r"^.{12}╰─+│  backspace +delete char +│$");
}

/// F-LAYOUT-06: picker moves to screen row 3 when help opens over it, back to center when help closes.
#[test]
fn f_layout_06_help_toggle() {
    let mut s = sim(80, 24);
    s.keys("f");
    s.assert_row_matches(8, r"^.{12}╭─{54}╮");
    s.keys("?");
    s.assert_contains("Help · File picker");
    s.assert_row_matches(2, r"^.{12}╭─{54}╮");
    s.assert_row_matches(3, r"^.{12}│ Files \(1/4\) +│$");
    // L1 -> L2 -> closed
    s.keys("??");
    s.assert_not_contains("Help ·");
    s.assert_row_matches(8, r"^.{12}╭─{54}╮");
    s.assert_row_matches(9, r"^.{12}│ Files \(1/4\) +│$");
}

/// F-LAYOUT-06: with help panel open modals start at screen row 3 (0-based 2), still horizontally centered.
#[test]
fn f_layout_06_help() {
    let mut s = sim(80, 24);
    s.keys("?");
    s.assert_contains("Help · Diff view");
    s.keys("f");
    s.assert_row_matches(2, r"^.{12}╭─{54}╮");
    s.assert_row_matches(3, r"^.{12}│ Files \(1/4\) +│$");
    s.assert_row_matches(9, r"^.{12}╰─{54}╯");
    s.keys("<Esc>");
    s.keys("C");
    s.assert_row_matches(2, r"^.{12}╭─{54}╮");
    s.assert_row_matches(3, r"^.{12}│ Config +│$");
    s.assert_row_matches(11, r"^.{12}╰─{54}╯");
    s.keys("<Esc>");
    s.keys("q");
    s.assert_contains("Help · Confirm");
    s.assert_row_matches(2, r"^.{29}╭─{20}╮");
    s.assert_row_matches(3, r"^.{29}│ Quit xplain\? \(y/n\) │$");
}

/// F-LAYOUT-07: footer wider than screen at 40 cols: 39 cells + …
#[test]
fn f_layout_07_footer() {
    let mut s = sim(40, 24);
    s.assert_row(-1, "(1-5/5) hjkl move  enter ask  J/K comme…");
    s.keys("/abc");
    s.assert_row(-1, "/abc█(1-5/5) hjkl move  enter ask  J/K …");
}

/// F-LAYOUT-07: header and footer wider than screen: cut to cols-1 cells + … in last cell.
#[test]
fn f_layout_07_header_footer() {
    let s = sim(60, 24);
    s.assert_row(0, "[all] [full] [unified] [solarized] [mcp: off] [1/4] [cursor…");
    s.assert_row(-1, "(1-5/5) hjkl move  enter ask  J/K comments  ? help");
    s.assert_cell(59, 0, C::new().ch('…'));
}

/// F-LAYOUT-07: diff row, picker row and comment box quote row wider than their box end with …
#[test]
fn f_layout_07_rows() {
    let mut s = Sim::builder()
        .size(80, 24)
        .fixture(Fixture::Empty)
        .file(
            "long-directory-name-for-picker/with-a-much-longer-file-name-inside-it.txt",
            "first\n0123456789abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789END\n",
        )
        .build();
    s.assert_row_matches(3, r"^        1 \+▶first");
    s.assert_row(4, "        2 + 0123456789abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJKLMNOPQRSTU…");
    // picker row text `>A <path> +2 -0 *` is wider than the 54-cell inner box
    s.keys("f");
    s.assert_row_matches(12, r"^.{12}│>A long-directory-name-for-picker/with-a-much-longer-…│$");
    s.keys("<Esc>");
    // line selection of the long row; comment box (width cols-1 = 79) quotes it and cuts at the box edge
    s.keys("jVahi<Enter>");
    s.assert_contains("selection L2");
    s.assert_row(7, "│ > 0123456789abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ0…│");
}
