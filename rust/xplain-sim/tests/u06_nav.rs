//! Ported from `e2e/scenarios/u06-nav`.

use xplain_sim::{CellExpect as C, Fixture, Sim};

/// F-NAV-09: viewport follows cursor (margin 2, clamped to bottom); keys that do not move the cursor (i, Esc)
/// never scroll.
#[test]
fn f_nav_09_scroll() {
    let mut s = Sim::builder().build();
    s.assert_row_matches(
        0,
        r"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/4\] \[cursor L2:C1\] README\.md \+2 -1$",
    );
    s.assert_row_matches(-1, r"^\(1-5/5\) hjkl move  enter ask  J/K comments  \? help$");

    s.keys("<Tab>");
    s.assert_row_matches(0, r"\[2/4\] \[cursor L30:C1\] src/big\.ts ");
    s.assert_row_matches(-1, r"^\(26-62/62\) hjkl move  enter ask  J/K comments  \? help$");

    s.keys("g");
    s.assert_row_matches(0, r"\[cursor r1:C1\] src/big\.ts ");
    s.assert_row_matches(2, r"^@@ ");
    s.assert_row_matches(-1, r"^\(1-37/62\) hjkl move  enter ask  J/K comments  \? help$");

    s.keys("j");
    s.assert_row_matches(0, r"\[cursor L1:C1\] src/big\.ts ");
    s.assert_row_matches(2, r"^@@ ");
    s.assert_row_matches(-1, r"^\(1-37/62\) ");

    // cursor on row 38 of 62 (v30 takes 2 rows); view keeps 2 rows below it
    s.keys("36j");
    s.assert_row_matches(0, r"\[cursor L36:C1\] src/big\.ts ");
    s.assert_row_matches(-1, r"^\(4-40/62\) hjkl move  enter ask  J/K comments  \? help$");

    s.keys("k");
    s.assert_row_matches(0, r"\[cursor L35:C1\] src/big\.ts ");
    s.assert_row_matches(-1, r"^\(4-40/62\) ");

    // i is unbound; cursor, view and footer unchanged
    s.keys("i");
    s.assert_row_matches(0, r"\[cursor L35:C1\] src/big\.ts ");
    s.assert_row_matches(-1, r"^\(4-40/62\) hjkl move  enter ask  J/K comments  \? help$");

    // Esc with nothing to close is a no-op
    s.keys("<Esc>");
    s.assert_row_matches(0, r"\[cursor L35:C1\] src/big\.ts ");
    s.assert_row_matches(-1, r"^\(4-40/62\) ");

    s.keys("G");
    s.assert_row_matches(0, r"\[cursor L60:C1\] src/big\.ts ");
    s.assert_row_matches(-1, r"^\(26-62/62\) hjkl move  enter ask  J/K comments  \? help$");
}

const HEADER_ALL: &str =
    r"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/4\] \[cursor L2:C1\] README\.md";
const NO_CHANGES: &str = "[all] [mcp: off] No changes (m cycles mode, F search, q quits)";
const PICKER_HINT: &str = r"^ {12}│ j/k/↑↓ move d/u half page enter open esc/q close +│$";
const CTRLS_A: &str =
    "<C-a><C-b><C-d><C-e><C-f><C-g><C-k><C-l><C-n><C-o><C-p><C-r><C-t><C-u><C-v><C-w><C-x><C-y>";

/// Asserts the cell at `offset` chars into the first `text` found in `row`.
fn cell_in_row(s: &Sim, row: isize, text: &str, offset: usize, want: C) {
    let found = s.find_in_row(row, text);
    assert!(found.is_some(), "{text:?} not in row {row}\n{}", s.dump());
    if let Some(p) = found {
        s.assert_cell(p.x + offset, row, want);
    }
}

/// Three list rows of a small picker (`a`, `b`, `c` are regex row bodies without the trailing padding).
fn picker_window(title: &str, a: &str, b: &str, c: &str) -> String {
    format!(
        r"│ Files \({title}\) +│[^\n]*\n[^\n]*│{a} +│[^\n]*\n[^\n]*│{b} +│[^\n]*\n[^\n]*│{c} +│[^\n]*\n[^\n]*│ j/k/↑↓ move"
    )
}

/// Standard fixture at 80x12 plus six one-line untracked files n1..n6.
fn twelve_files() -> Sim {
    let mut b = Sim::builder().size(80, 12);
    for i in 1..=6 {
        b = b.file(&format!("n{i}.txt"), "a\n");
    }
    let mut s = b.build();
    s.remove_file("src/a.ts");
    s.git(&["mv", "package.json", "pkg.json"]);
    s.keys("r");
    s
}

/// Empty repo with `m.txt` (40 lines) and `o.txt` committed, then four separate change runs in m.txt, loaded with r.
fn m_repo(args: &[&str]) -> Sim {
    let mut s = Sim::builder().fixture(Fixture::Empty).args(args.iter().copied()).size(80, 16).build();
    let base: String = (1..=40).map(|i| format!("line {i}\n")).collect();
    s.write_file("m.txt", &base);
    s.write_file("o.txt", "a\n");
    s.git(&["add", "-A"]);
    s.git(&["commit", "-qm", "base"]);
    let mut work = String::new();
    for i in 1..=40 {
        match i {
            5 => work.push_str("line 5 changed\n"),
            20 | 21 => work.push_str(&format!("line {i} x\n")),
            30 => {}
            35 => work.push_str("line 35\ninserted\n"),
            _ => work.push_str(&format!("line {i}\n")),
        }
    }
    s.write_file("m.txt", &work);
    s.write_file("o.txt", "b\n");
    s.keys("r");
    s
}

/// F-FILES-01: box size and centering, title, rows with status/rename/counts/current mark, hint, colors.
#[test]
fn f_files_01_box() {
    let mut s = Sim::builder().size(80, 24).build();
    s.remove_file("src/a.ts");
    s.git(&["mv", "package.json", "pkg.json"]);
    s.keys("r");
    s.assert_not_contains("Files (");
    s.assert_row_matches(0, r"\[1/6\] \[cursor L2:C1\] README\.md");
    s.keys("f");
    s.assert_row_matches(7, r"^ {12}╭─{54}╮$");
    s.assert_row_matches(8, r"^ {12}│ Files \(1/6\) {42}│$");
    s.assert_row_matches(9, r"^ {12}│>M README\.md \+2 -1 \* +│$");
    s.assert_row_matches(10, r"^ {12}│ R package\.json -> pkg\.json \+0 -0 +│$");
    s.assert_row_matches(11, r"^ {12}│ D src/a\.ts \+0 -3 +│$");
    s.assert_row_matches(12, r"^ {12}│ M src/big\.ts \+1 -1 +│$");
    s.assert_row_matches(13, r"^ {12}│ M src/c\.tsx \+1 -1 +│$");
    s.assert_row_matches(14, r"^ {12}│ A x\.bin \+1 -0 +│$");
    s.assert_row_matches(15, PICKER_HINT);
    s.assert_row_matches(16, r"^ {12}╰─{54}╯$");
    cell_in_row(&s, 8, "Files (1/6)", 0, C::new().bold(true));
    // status letter colors (solarized) - M accent, R mode, D dels, A adds
    cell_in_row(&s, 12, "M src/big.ts", 0, C::new().fg("#cb4b16"));
    cell_in_row(&s, 10, "R package.json", 0, C::new().fg("#b58900"));
    cell_in_row(&s, 11, "D src/a.ts", 0, C::new().fg("#dc322f"));
    cell_in_row(&s, 14, "A x.bin", 0, C::new().fg("#859900"));
    // +adds in adds color, -dels in dels color
    cell_in_row(&s, 12, "+1 -1", 0, C::new().fg("#859900"));
    cell_in_row(&s, 12, "+1 -1", 3, C::new().fg("#dc322f"));
    // selected row bg selBg / fg selFg; other rows not
    cell_in_row(&s, 9, ">M README", 0, C::new().bg("#073642"));
    cell_in_row(&s, 9, "README.md", 0, C::new().bg("#073642").fg("#93a1a1"));
    cell_in_row(&s, 12, "src/big.ts", 0, C::new().bg("#002b36"));
    // hint dim
    cell_in_row(&s, 15, "j/k/", 0, C::new().fg("#586e75"));
    s.keys("j");
    // current-file mark stays on README.md, selection moves
    s.assert_row_matches(8, r"│ Files \(2/6\) +│$");
    s.assert_row_matches(9, r"│ M README\.md \+2 -1 \* +│$");
    s.assert_row_matches(10, r"│>R package\.json -> pkg\.json \+0 -0 +│$");
    cell_in_row(&s, 9, "README.md", 0, C::new().bg("#002b36"));
    cell_in_row(&s, 10, "package.json", 0, C::new().bg("#073642"));
}

/// F-FILES-01: f does nothing in browse.
#[test]
fn f_files_01_pre() {
    let mut s = Sim::builder().size(80, 24).build();
    let head = "[browse] [solarized] [mcp: off] [cursor L1:C1] README.md";
    s.keys("FREADME.md<Enter>");
    s.assert_row(0, head);
    s.keys("f");
    s.assert_not_contains("Files (");
    s.assert_row(0, head);
    s.keys("<Esc>f");
    // back in diff view f works
    s.assert_contains(" Files (1/4)");
}

/// F-FILES-01: picker list window is height-4 rows, centered on the selection, clamped to the ends.
#[test]
fn f_files_01_window() {
    let mut s = twelve_files();
    // make src/c.tsx (file 5) current before opening
    s.keys("<Tab><Tab><Tab><Tab>");
    s.assert_row_matches(0, r"\[5/12\] \[cursor L1:C1\] src/c\.tsx");
    s.keys("f");
    s.assert_matches(&picker_window(
        "5/12",
        r" M src/big\.ts \+1 -1",
        r">M src/c\.tsx \+1 -1 \*",
        r" A n1\.txt \+1 -0",
    ));
    s.keys("<Esc>g<Tab><Tab><Tab><Tab><Tab><Tab><Tab>");
    s.assert_row_matches(0, r"\[12/12\] .*x\.bin");
    s.keys("f");
    // last file selected - window clamped to the end
    s.assert_matches(&picker_window(
        "12/12",
        r" A n5\.txt \+1 -0",
        r" A n6\.txt \+1 -0",
        r">A x\.bin \+1 -0 \*",
    ));
    s.keys("<Esc><Tab>");
    s.assert_row_matches(0, r"\[1/12\] .*README\.md");
    s.keys("f");
    // first file selected - window clamped to the start
    s.assert_matches(&picker_window(
        "1/12",
        r">M README\.md \+2 -1 \*",
        r" R package\.json -> pkg\.json \+0 -0",
        r" D src/a\.ts \+0 -3",
    ));
}

/// F-FILES-02: picker Esc / q / f close without change; other and Ctrl keys ignored; ? toggles help; Enter opens.
#[test]
fn f_files_02_close_open() {
    let mut s = Sim::builder().size(80, 24).build();
    s.assert_row_matches(0, HEADER_ALL);
    s.keys("fj");
    s.assert_contains(" Files (2/4)");
    s.assert_matches(r"│>M src/big\.ts \+1 -1 +│");
    s.keys("<Esc>");
    s.assert_not_contains("Files (");
    s.assert_row_matches(0, r"\[1/4\] \[cursor L2:C1\] README\.md");
    // reopening selects the current file again
    s.keys("f");
    s.assert_contains(" Files (1/4)");
    s.assert_matches(r"│>M README\.md \+2 -1 \* +│");
    s.keys("jq");
    s.assert_not_contains("Files (");
    s.assert_row_matches(0, r"\[1/4\] \[cursor L2:C1\] README\.md");
    s.keys("fjjf");
    s.assert_not_contains("Files (");
    s.assert_row_matches(0, r"\[1/4\] \[cursor L2:C1\] README\.md");
    s.keys("fj");
    s.assert_contains(" Files (2/4)");
    // diff-view keys do nothing inside the picker
    s.keys("xtscmgGvai/:F]<Space>");
    s.assert_contains(" Files (2/4)");
    s.assert_matches(r"│>M src/big\.ts \+1 -1 +│");
    s.assert_not_contains("Search (");
    s.assert_not_contains("Config");
    s.assert_row_matches(0, HEADER_ALL);
    s.assert_row(-1, "(1-5/5) hjkl move  enter ask  J/K comments  ? help");
    s.keys("<C-d><C-u><C-n><C-p><C-t>");
    s.assert_contains(" Files (2/4)");
    s.assert_matches(r"│>M src/big\.ts \+1 -1 +│");
    s.keys("?");
    s.assert_contains(" Files (2/4)");
    s.assert_contains("Help · File picker");
    // picker keys still work with help open
    s.keys("j");
    s.assert_contains(" Files (3/4)");
    s.assert_contains("Help · File picker");
    s.assert_matches(r"│>M src/c\.tsx \+1 -1 +│");
    s.keys("<Enter>");
    s.assert_not_contains("Files (");
    s.assert_matches(r"^   1      -▶export const C");
    s.assert_row_matches(0, r"\[3/4\] \[cursor L1:C1\] src/c\.tsx");
}

/// F-FILES-02: empty picker on the no-changes screen.
#[test]
fn f_files_02_empty() {
    let mut s = Sim::builder().fixture(Fixture::Empty).size(80, 24).build();
    s.assert_row(0, NO_CHANGES);
    s.keys("f");
    s.assert_matches(
        r"│ Files \(1/0\) +│\n[^\n]*│ +│\n[^\n]*│ j/k/↑↓ move d/u half page enter open esc/q close +│",
    );
    s.assert_not_contains("│>");
    s.assert_not_contains("│ M ");
    s.assert_not_contains("│ A ");
    s.keys("<Esc>");
    s.assert_not_contains("Files (");
    s.assert_row(0, NO_CHANGES);
    s.assert_row(-1, "(0-0/0) hjkl move  enter ask  J/K comments  ? help");
}

/// F-FILES-02: picker j/k/Down/Up move 1 and d/u move floor(H/2), all clamped; window follows the selection.
#[test]
fn f_files_02_move() {
    let mut s = twelve_files();
    s.keys("f");
    let readme = r"README\.md \+2 -1";
    let pkg = r"package\.json -> pkg\.json \+0 -0";
    let first =
        picker_window("1/12", &format!(r">M {readme} \*"), &format!(r" R {pkg}"), r" D src/a\.ts \+0 -3");
    s.assert_matches(&first);
    s.keys("k<Up>");
    // clamped at the first file
    s.assert_matches(&first);
    let second =
        picker_window("2/12", &format!(r" M {readme} \*"), &format!(r">R {pkg}"), r" D src/a\.ts \+0 -3");
    s.keys("j");
    s.assert_matches(&second);
    s.keys("<Down>");
    s.assert_matches(&picker_window(
        "3/12",
        &format!(r" R {pkg}"),
        r">D src/a\.ts \+0 -3",
        r" M src/big\.ts \+1 -1",
    ));
    s.keys("<Up>");
    s.assert_matches(&second);
    s.keys("d");
    // 2 + 4 = 6 (n1.txt)
    s.assert_matches(&picker_window(
        "6/12",
        r" M src/c\.tsx \+1 -1",
        r">A n1\.txt \+1 -0",
        r" A n2\.txt \+1 -0",
    ));
    s.keys("d");
    s.assert_matches(&picker_window(
        "10/12",
        r" A n4\.txt \+1 -0",
        r">A n5\.txt \+1 -0",
        r" A n6\.txt \+1 -0",
    ));
    s.keys("d");
    // clamped at the last file
    let last = picker_window("12/12", r" A n5\.txt \+1 -0", r" A n6\.txt \+1 -0", r">A x\.bin \+1 -0");
    s.assert_matches(&last);
    s.keys("j<Down>");
    s.assert_matches(&last);
    s.keys("u");
    s.assert_matches(&picker_window(
        "8/12",
        r" A n2\.txt \+1 -0",
        r">A n3\.txt \+1 -0",
        r" A n4\.txt \+1 -0",
    ));
    s.keys("u");
    s.assert_matches(&picker_window(
        "4/12",
        r" D src/a\.ts \+0 -3",
        r">M src/big\.ts \+1 -1",
        r" M src/c\.tsx \+1 -1",
    ));
    s.keys("u");
    s.assert_matches(&first);
    // moving the selection does not switch the shown file
    s.assert_row_matches(0, r"\[1/12\] \[cursor L2:C1\] README\.md");
}

/// F-NAV-05: ] and [ cross hunk rows in changes scope; no move in browse.
#[test]
fn f_nav_05_changes_browse() {
    let mut s = m_repo(&["--changes-only"]);
    s.assert_row_matches(0, r"\[1/2\] .*m\.txt \+4");
    // Tab away and back so m.txt gets its initial position (first change start)
    s.keys("<Tab><Tab>");
    // changes scope has no full-file start position; cursor on first row (hunk header)
    s.assert_row_matches(0, r"\[changes\] .*\[cursor r1:C1\] m\.txt ");
    s.keys("[");
    s.assert_row_matches(0, r"\[cursor r1:C1\] m\.txt ");
    s.keys("]");
    s.assert_row_matches(0, r"\[cursor L5:C1\] m\.txt ");
    s.assert_matches(r"^   5      -▶line 5\b");
    s.keys("]");
    // crosses the next hunk header row
    s.assert_row_matches(0, r"\[cursor L20:C1\] m\.txt ");
    s.assert_matches(r"^  20      -▶line 20\b");
    s.keys("]]");
    s.assert_row_matches(0, r"\[cursor L35:C1\] m\.txt ");
    s.assert_matches(r"^       35 \+▶inserted");
    s.keys("]");
    s.assert_row_matches(0, r"\[cursor L35:C1\] m\.txt ");
    s.keys("[[[");
    s.assert_row_matches(0, r"\[cursor L5:C1\] m\.txt ");
    // open m.txt in browse via file search
    s.keys("Fm.txt<Enter>");
    s.assert_row_matches(0, r"^\[browse\] .*\[cursor L1:C1\] m\.txt$");
    s.keys("]");
    s.assert_row_matches(0, r"\[cursor L1:C1\] m\.txt$");
    s.keys("4j");
    s.assert_row_matches(0, r"\[cursor L5:C1\] m\.txt$");
    s.keys("]");
    // browse has no change starts
    s.assert_row_matches(0, r"^\[browse\] .*\[cursor L5:C1\] m\.txt$");
    s.keys("[");
    s.assert_row_matches(0, r"^\[browse\] .*\[cursor L5:C1\] m\.txt$");
}

/// F-NAV-05: ] and [ keep the desired column and ignore (and clear) a count.
#[test]
fn f_nav_05_column_count() {
    let mut s = m_repo(&[]);
    s.assert_row_matches(0, r"\[1/2\] .*m\.txt \+4 -4$");
    // Tab away and back so m.txt gets its initial position (first change start)
    s.keys("<Tab><Tab>");
    s.assert_row_matches(0, r"\[cursor L5:C1\] m\.txt ");
    s.assert_matches(r"^   5      -▶line 5\b");
    s.keys("j10l");
    s.assert_row_matches(0, r"\[cursor L5:C11\] m\.txt ");
    s.assert_matches(r"^        5 \+▶line 5 changed");
    s.keys("]");
    // desired column 11 shown clamped to 'line 20' (7 chars)
    s.assert_row_matches(0, r"\[cursor L20:C7\] m\.txt ");
    s.assert_matches(r"^  20      -▶line 20\b");
    s.keys("]");
    s.assert_row_matches(0, r"\[cursor L30:C7\] m\.txt ");
    s.keys("]");
    s.assert_row_matches(0, r"\[cursor L35:C8\] m\.txt ");
    s.assert_matches(r"^       35 \+▶inserted");
    s.keys("[[[");
    s.assert_row_matches(0, r"\[cursor L5:C6\] m\.txt ");
    s.assert_matches(r"^   5      -▶line 5\b");
    s.keys("j");
    // desired column survived the jumps
    s.assert_row_matches(0, r"\[cursor L5:C11\] m\.txt ");
    s.assert_matches(r"^        5 \+▶line 5 changed");
    s.keys("2]");
    // count ignored - one jump only (a second would reach L30)
    s.assert_row_matches(0, r"\[cursor L20:C7\] m\.txt ");
    s.assert_matches(r"^  20      -▶line 20\b");
    s.keys("j");
    // count was cleared by ] - j moves one row (del L21), not two (add L20)
    s.assert_row_matches(0, r"\[cursor L21:C7\] m\.txt ");
    s.assert_matches(r"^  21      -▶line 21\b");
    s.keys("3[");
    // one jump back to the start of this run (row 22), not three
    s.assert_row_matches(0, r"\[cursor L20:C7\] m\.txt ");
    s.assert_matches(r"^  20      -▶line 20\b");
    s.keys("k");
    // count cleared by [ - k moves one row
    s.assert_row_matches(0, r"\[cursor L19:C7\] m\.txt ");
    s.assert_matches(r"^  19   19  ▶line 19\b");
}

/// F-NAV-05: ] and [ jump the cursor between change starts, stop at the ends, viewport follows.
#[test]
fn f_nav_05_jump() {
    let mut s = m_repo(&[]);
    s.assert_row_matches(0, r"\[1/2\] .*m\.txt \+4 -4$");
    // Tab away and back so m.txt gets its initial position (first change start)
    s.keys("<Tab><Tab>");
    let at = |s: &Sim, line: &str, cursor: &str, view: &str| {
        s.assert_matches(line);
        s.assert_row_matches(0, cursor);
        s.assert_row_matches(-1, &format!(r"(^|\| )\({view}\) "));
    };
    s.assert_matches(r"^   5      -▶line 5\b");
    s.assert_row_matches(0, r"\[1/2\] \[cursor L5:C1\] m\.txt ");
    s.assert_row_matches(-1, r"(^|\| )\(3-15/45\) ");
    s.keys("[");
    // no change start before the first one; nothing moves
    at(&s, r"^   5      -▶line 5\b", r"\[cursor L5:C1\] m\.txt ", "3-15/45");
    s.keys("]");
    // row 23 is the second del row of the same run, not a change start; ] lands on row 22
    at(&s, r"^  20      -▶line 20\b", r"\[cursor L20:C1\] m\.txt ", "12-24/45");
    s.keys("]");
    at(&s, r"^  30      -▶line 30\b", r"\[cursor L30:C1\] m\.txt ", "24-36/45");
    s.keys("]");
    // pure insertion is a change start too (add row, new number 35)
    at(&s, r"^       35 \+▶inserted\b", r"\[cursor L35:C1\] m\.txt ", "30-42/45");
    s.keys("]");
    // no change start after the last one; nothing moves
    at(&s, r"^       35 \+▶inserted\b", r"\[cursor L35:C1\] m\.txt ", "30-42/45");
    s.keys("[");
    // row 34 still inside the margin of the current view; top stays
    at(&s, r"^  30      -▶line 30\b", r"\[cursor L30:C1\] m\.txt ", "30-42/45");
    s.keys("[");
    at(&s, r"^  20      -▶line 20\b", r"\[cursor L20:C1\] m\.txt ", "20-32/45");
    s.keys("[");
    at(&s, r"^   5      -▶line 5\b", r"\[cursor L5:C1\] m\.txt ", "4-16/45");
}

/// F-NAV-06: Tab / S-Tab ignored in browse (diff file kept for Esc back).
#[test]
fn f_nav_06_browse() {
    let mut s = Sim::builder().size(80, 24).build();
    s.keys("<Tab>");
    s.assert_row_matches(0, r"\[2/4\] \[cursor L30:C1\] src/big\.ts ");
    s.keys("FREADME.md<Enter>");
    s.assert_row_matches(0, r"^\[browse\] \[solarized\] \[mcp: off\] \[cursor L1:C1\] README\.md$");
    s.keys("j<Tab>");
    s.assert_row_matches(0, r"^\[browse\] .*\[cursor L2:C1\] README\.md$");
    s.keys("<S-Tab><S-Tab>");
    s.assert_row_matches(0, r"^\[browse\] .*\[cursor L2:C1\] README\.md$");
    s.keys("<Esc>");
    // back on the same diff file; Tab in browse did not switch it
    s.assert_row_matches(0, r"^\[all\] .*\[2/4\] \[cursor L30:C1\] src/big\.ts ");
}

/// F-NAV-06: Tab / S-Tab switch file with wrap; each file lands on its first change; Left/Right never switch.
#[test]
fn f_nav_06_cycle() {
    let mut s = Sim::builder().size(80, 24).build();
    s.assert_matches(r"^   2      -▶hello");
    s.assert_row_matches(0, r"\[1/4\] \[cursor L2:C1\] README\.md");
    s.keys("<Right>");
    // Right moves the char cursor, file unchanged
    s.assert_row_matches(0, r"\[1/4\] \[cursor L2:C2\] README\.md ");
    s.keys("<Left>");
    s.assert_row_matches(0, r"\[1/4\] \[cursor L2:C1\] README\.md ");
    s.keys("<Tab>");
    s.assert_matches(r"^  30      -▶const v30 = 1;");
    s.assert_row_matches(0, r"\[2/4\] \[cursor L30:C1\] src/big\.ts");
    s.assert_row_matches(-1, r"(^|\| )\(28-48/62\) ");
    s.keys("<Tab>");
    s.assert_matches(r"^   1      -▶export const C");
    s.assert_row_matches(0, r"\[3/4\] \[cursor L1:C1\] src/c\.tsx");
    s.keys("<Tab>");
    s.assert_matches(r"^        1 \+▶x");
    s.assert_row_matches(0, r"\[4/4\] \[cursor L1:C1\] x\.bin");
    s.keys("<Tab>");
    // wraps to the first file
    s.assert_matches(r"^   2      -▶hello");
    s.assert_row_matches(0, r"\[1/4\] \[cursor L2:C1\] README\.md ");
    s.keys("<S-Tab>");
    // S-Tab wraps to the last file
    s.assert_row_matches(0, r"\[4/4\] \[cursor L1:C1\] x\.bin ");
    s.keys("<S-Tab>");
    s.assert_row_matches(0, r"\[3/4\] \[cursor L1:C1\] src/c\.tsx ");
    s.keys("<S-Tab>");
    s.assert_row_matches(0, r"\[2/4\] \[cursor L30:C1\] src/big\.ts ");
    s.keys("<S-Tab>");
    s.assert_row_matches(0, r"\[1/4\] \[cursor L2:C1\] README\.md ");
}

/// F-NAV-06: Tab / S-Tab on the no-changes screen are no-ops.
#[test]
fn f_nav_06_no_files() {
    let mut s = Sim::builder().fixture(Fixture::Empty).size(80, 24).build();
    s.assert_row(0, NO_CHANGES);
    s.assert_row_matches(-1, r"^\(0-0/0\) hjkl move");
    s.keys("<Tab><S-Tab><Tab>");
    s.assert_row(0, NO_CHANGES);
    s.assert_row_matches(-1, r"^\(0-0/0\) hjkl move  enter ask  J/K comments  \? help$");
}

/// F-NAV-06: Tab / S-Tab with one file keep the same file and cursor.
#[test]
fn f_nav_06_one_file() {
    let mut s = Sim::builder().args(["HEAD", "--", "src/big.ts"]).size(80, 24).build();
    s.assert_row_matches(0, r"\[1/1\] \[cursor L30:C1\] src/big\.ts ");
    s.keys("5j3l");
    s.assert_matches(r"^  34   34  ▶const v34");
    s.assert_row_matches(0, r"\[1/1\] \[cursor L34:C4\] src/big\.ts ");
    s.keys("<Tab>");
    s.assert_row_matches(0, r"\[1/1\] \[cursor L34:C4\] src/big\.ts ");
    s.keys("<S-Tab>");
    s.assert_row_matches(0, r"\[1/1\] \[cursor L34:C4\] src/big\.ts ");
    s.keys("j");
    // still the same rows; j continues from L34
    s.assert_row_matches(0, r"\[1/1\] \[cursor L35:C4\] src/big\.ts ");
}

/// F-NAV-07: Ctrl combos are ignored in diff view, file picker and browse.
#[test]
fn f_nav_07_diff_picker_browse() {
    let mut s = Sim::builder().size(80, 24).build();
    let big =
        r"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[2/4\] \[cursor L30:C1\] src/big\.ts ";
    let foot = "(28-48/62) hjkl move  enter ask  J/K comments  ? help";
    s.keys("<Tab>");
    s.assert_row_matches(0, big);
    s.assert_row(-1, foot);
    s.keys(CTRLS_A);
    // no reload note, no cursor/viewport move, no visual, no theme, no picker/help/editor
    for t in ["Files (", "Help ·", "Search (", "╭"] {
        s.assert_not_contains(t);
    }
    s.assert_row_matches(0, big);
    s.assert_row(-1, foot);
    s.keys("f");
    s.assert_contains(" Files (2/4)");
    s.assert_matches(r"│>M src/big\.ts \+1 -1 \*");
    s.keys("<C-d><C-n><C-e><C-f><C-u><C-p><C-t><C-b><C-g><C-k><C-l><C-o><C-r><C-v><C-w><C-x><C-y><C-a>");
    // picker still open (Ctrl+F does not close it), selection unchanged, theme unchanged
    s.assert_contains(" Files (2/4)");
    s.assert_matches(r"│>M src/big\.ts \+1 -1 \*");
    s.assert_row_matches(0, r"\[solarized\] .*\[2/4\] \[cursor L30:C1\] src/big\.ts ");
    s.keys("<Esc>");
    s.assert_not_contains("Files (");
    s.keys("FREADME.md<Enter>");
    let browse = "[browse] [solarized] [mcp: off] [cursor L1:C1] README.md";
    s.assert_row(0, browse);
    s.assert_row(-1, "(1-3/3) hjkl move  enter ask  J/K comments  ? help");
    s.keys(CTRLS_A);
    for t in ["Files (", "Help ·", "Search (", "╭"] {
        s.assert_not_contains(t);
    }
    s.assert_row(0, browse);
    s.assert_row(-1, "(1-3/3) hjkl move  enter ask  J/K comments  ? help");
    // digits still act as a count in browse
    s.keys("2j");
    s.assert_row(0, "[browse] [solarized] [mcp: off] [cursor L3:C1] README.md");
    // Ctrl+C is the exception - exits at once
    s.keys("<C-c>");
    assert_eq!(s.exit_code(), Some(0));
}

/// F-NAV-07: Ctrl combos are not typed into find, goto, file search and comment editor.
#[test]
fn f_nav_07_text_inputs() {
    let mut s = Sim::builder().size(80, 24).build();
    s.keys("/ab<C-a><C-w>c<C-u>");
    s.assert_row_matches(-1, r"^/abc█\(1-5/5\) ");
    s.keys("<Esc>:1<C-a><C-u>2<C-k>");
    s.assert_row_matches(-1, r"^:12█\(1-5/5\) ");
    s.keys("<Esc>FRE<C-a><C-w><C-u>AD");
    s.assert_contains(" Search (");
    s.assert_matches(r"> READ \s*│");
    s.keys("<Esc>ax<C-a><C-w><C-u>y");
    s.assert_matches(r"│ xy \s*│");
    s.keys("<Esc>");
    s.assert_not_contains("│ xy");
    s.assert_not_contains("Search (");
    s.assert_not_contains("█");
}

/// F-NAV-08: file switch and browse Esc land on the first change with top = firstChange-3 then follow (80x8, H=5).
#[test]
fn f_nav_08_browse_back() {
    let mut s = Sim::builder().size(80, 8).build();
    s.keys("<Tab>");
    // top = 30-3 = 27, then follow (margin 2 needs row 32 visible) -> 28
    s.assert_row_matches(0, r"\[2/4\] \[cursor L30:C1\] src/big\.ts ");
    s.assert_row_matches(4, r"^  30      -▶const v30 = 1;");
    s.assert_row(-1, "(29-33/62) hjkl move  enter ask  J/K comments  ? help");
    s.keys("G");
    s.assert_row_matches(0, r"\[cursor L60:C1\] src/big\.ts ");
    s.assert_row(-1, "(58-62/62) hjkl move  enter ask  J/K comments  ? help");
    s.keys("FREADME.md<Enter>");
    // browse open is a placement too - row 1, top 0
    s.assert_row(0, "[browse] [solarized] [mcp: off] [cursor L1:C1] README.md");
    s.assert_row(-1, "(1-3/3) hjkl move  enter ask  J/K comments  ? help");
    s.keys("<Esc>");
    // back on src/big.ts, not on L60 where it was left
    s.assert_row_matches(0, r"^\[all\] .*\[2/4\] \[cursor L30:C1\] src/big\.ts ");
    s.assert_row_matches(4, r"^  30      -▶const v30 = 1;");
    s.assert_row(-1, "(29-33/62) hjkl move  enter ask  J/K comments  ? help");
}

/// F-NAV-08: m landing on another path places the cursor on that file's first change.
#[test]
fn f_nav_08_mode() {
    let mut s = Sim::builder().size(96, 24).build();
    s.git(&["add", "src/c.tsx"]);
    s.keys("<Tab>G");
    s.assert_row_matches(0, r"^\[all\] .*\[2/4\] \[cursor L60:C1\] src/big\.ts ");
    s.keys("m");
    // staged has only src/c.tsx
    s.assert_matches(r"^   1      -▶export const C");
    s.assert_row_matches(0, r"^\[staged\] .*\[1/1\] \[cursor L1:C1\] src/c\.tsx");
    s.assert_row_matches(-1, r"^\(1-3/3\) ");
    s.keys("m");
    // unstaged shows README.md first
    s.assert_matches(r"^   2      -▶hello");
    s.assert_row_matches(0, r"^\[unstaged\] .*\[1/2\] \[cursor L2:C1\] README\.md");
}

/// F-NAV-08: full scope file without change rows (rename only) lands on row 1, top 0.
#[test]
fn f_nav_08_no_change_rows() {
    let mut s = Sim::builder().size(100, 24).build();
    s.git(&["mv", "package.json", "pkg.json"]);
    s.keys("r");
    s.assert_row_matches(0, r"\[1/5\] \[cursor L2:C1\] README\.md");
    s.keys("2l<Tab>");
    s.assert_row_matches(0, r"\[2/5\] \[cursor r1:C1\] package\.json -> pkg\.json \+0 -0$");
    s.assert_row_contains(2, "Renamed, no content changes");
    s.assert_row_matches(-1, r"(^|\| )\(1-1/1\) ");
}

/// F-NAV-08: picker Enter on another file places the cursor like a file switch (selection cleared, col 1).
#[test]
fn f_nav_08_picker() {
    let mut s = Sim::builder().size(80, 8).build();
    s.keys("2lv");
    s.assert_row_matches(0, r"\[1/4\] \[visual L2:C3\] README\.md");
    s.keys("fj<Enter>");
    s.assert_not_contains("Files (");
    s.assert_row_matches(0, r"\[2/4\] \[cursor L30:C1\] src/big\.ts ");
    s.assert_row_matches(4, r"^  30      -▶const v30 = 1;");
    s.assert_row(-1, "(29-33/62) hjkl move  enter ask  J/K comments  ? help");
}

/// F-NAV-08: placement resets column to 1, pane to new, clears selection and pending count (split, 120x10).
#[test]
fn f_nav_08_reset_state() {
    let mut s = Sim::builder().args(["--split"]).size(120, 10).build();
    s.assert_row_matches(0, r"\[1/4\] \[cursor new L2:C1\] README\.md ");
    s.assert_row_matches(-1, r"p pane  \? help$");
    s.keys("p3lv");
    s.assert_row_matches(0, r"\[1/4\] \[visual old L2:C4\] README\.md ");
    // pending count 5, then file switch
    s.keys("5<Tab>");
    // split rows 61; top = 27, follow keeps 27 (H=7)
    s.assert_row_matches(0, r"\[2/4\] \[cursor new L30:C1\] src/big\.ts ");
    s.assert_row_matches(-1, r"^\(28-34/61\) ");
    s.keys("k");
    // count 5 was cleared by the switch - k moves one row
    s.assert_row_matches(0, r"\[2/4\] \[cursor new L29:C1\] src/big\.ts ");
}

/// F-NAV-08: c (scope) and s (effective layout, cols >= 100) re-place the cursor.
#[test]
fn f_nav_08_scope_split() {
    let mut s = Sim::builder().size(120, 8).build();
    s.keys("<Tab>G");
    s.assert_row_matches(0, r"\[full\] \[unified\] .*\[2/4\] \[cursor L60:C1\] src/big\.ts ");
    s.assert_row_matches(-1, r"^\(58-62/62\) ");
    s.keys("c");
    // changes scope - first row (hunk header), top 0
    s.assert_row_matches(0, r"\[changes\] .*\[2/4\] \[cursor r1:C1\] src/big\.ts ");
    s.assert_row_matches(2, r"^@@ ");
    s.assert_row_matches(-1, r"^\(1-5/9\) ");
    s.keys("G");
    s.assert_row_matches(0, r"\[changes\] .*\[cursor L33:C1\] src/big\.ts ");
    s.keys("c");
    s.assert_row_matches(0, r"\[full\] .*\[cursor L30:C1\] src/big\.ts ");
    s.assert_row_matches(-1, r"^\(29-33/62\) ");
    s.keys("G");
    s.assert_row_matches(0, r"\[cursor L60:C1\] src/big\.ts ");
    s.keys("s");
    s.assert_row_matches(0, r"\[split\] .*\[cursor new L30:C1\] src/big\.ts ");
    s.assert_row_matches(-1, r"^\(29-33/61\) ");
    s.keys("G");
    s.assert_row_matches(0, r"\[cursor new L60:C1\] src/big\.ts ");
    s.keys("s");
    s.assert_row_matches(0, r"\[unified\] .*\[cursor L30:C1\] src/big\.ts ");
    s.assert_row_matches(-1, r"^\(29-33/62\) ");
}

/// F-NAV-09: comment boxes count in the follow; a row block is never cut at the viewport top (R=12: H=9, margin 2).
#[test]
fn f_nav_09_comment_blocks() {
    let mut s = Sim::builder().size(80, 12).build();
    s.keys("<Tab>gj");
    s.assert_row_matches(0, r"\[cursor L1:C1\] src/big\.ts ");
    s.keys("ahi there<Enter>");
    s.assert_contains("hi there");
    s.assert_row_matches(3, r"^   1    1  ▶const v1");
    s.assert_row_matches(4, r"^╭");
    s.keys("j");
    // rows 0..4 need 1+5+3 = 9 lines - top stays 0
    s.assert_row_matches(0, r"\[cursor L2:C1\] src/big\.ts ");
    s.assert_row_matches(2, r"^@@ ");
    s.assert_row_matches(-1, r"(^|\| )\(1-9/62\) ");
    s.keys("j");
    // rows 0..5 need 10 lines - top 1, whole block of row 1 at the top
    s.assert_row_matches(0, r"\[cursor L3:C1\] src/big\.ts ");
    s.assert_row_matches(2, r"^   1    1   const v1");
    s.assert_row_matches(3, r"^╭");
    s.assert_row_matches(-1, r"(^|\| )\(2-10/62\) ");
    s.keys("j");
    // rows 1..6 need 10 lines - top skips the whole block of row 1, no partial box at the top
    s.assert_row_matches(0, r"\[cursor L4:C1\] src/big\.ts ");
    s.assert_row_matches(2, r"^   2    2   const v2");
    s.assert_row_matches(-1, r"(^|\| )\(3-11/62\) ");
    s.keys("k");
    // cursor row 3 above top+margin - top = 1
    s.assert_row_matches(0, r"\[cursor L3:C1\] src/big\.ts ");
    s.assert_row_matches(2, r"^   1    1   const v1");
    s.assert_row_matches(3, r"^╭");
    s.assert_row_matches(-1, r"(^|\| )\(2-10/62\) ");
}

/// F-NAV-09: with the comment editor open the margin is 0 and the editor box counts under the cursor row.
#[test]
fn f_nav_09_editor() {
    let mut s = Sim::builder().size(80, 12).build();
    s.keys("<Tab>g6j");
    s.assert_row_matches(0, r"\[cursor L6:C1\] src/big\.ts ");
    s.assert_row(-1, "(1-9/62) hjkl move  enter ask  J/K comments  ? help");
    s.keys("a");
    // rows 0..6 (7 lines) + editor (4) > 9 - top 2; with margin 2 it would be 4
    s.assert_row_matches(6, r"^   6    6  ▶const v6");
    s.assert_row_matches(7, r"^╭");
    s.assert_row_matches(10, r"^╰");
    s.assert_row_matches(-1, r"^\(3-11/62\) ");
}

/// F-NAV-09: margin is min(2, floor((H-1)/2)) - R=7 (H=4) gives margin 1.
#[test]
fn f_nav_09_small_margin() {
    let mut s = Sim::builder().size(80, 7).build();
    s.keys("<Tab>");
    // top 27, follow with margin 1 -> 28
    s.assert_row_matches(0, r"\[2/4\] \[cursor L30:C1\] src/big\.ts ");
    s.assert_row_matches(4, r"^  30      -▶const v30");
    s.assert_row(-1, "(29-32/62) hjkl move  enter ask  J/K comments  ? help");
    s.keys("j");
    s.assert_row_matches(4, r"^       30 \+▶const v30 = 2;");
    s.assert_row_matches(-1, r"^\(30-33/62\) ");
    s.keys("k");
    // one row above the cursor is enough - top stays (margin 2 would scroll)
    s.assert_row_matches(3, r"^  30      -▶const v30");
    s.assert_row_matches(-1, r"^\(30-33/62\) ");
    s.keys("k");
    s.assert_row_matches(0, r"\[cursor L29:C1\] src/big\.ts ");
    s.assert_row_matches(3, r"^  29   29  ▶const v29");
    s.assert_row_matches(-1, r"^\(29-32/62\) ");
}

/// F-NAV-09: spec example - 62 rows, R=24 (H=21) - 20j then 19k.
#[test]
fn f_nav_09_spec_example() {
    let mut s = Sim::builder().size(80, 24).build();
    s.keys("<Tab>g");
    s.assert_row_matches(0, r"\[2/4\] \[cursor r1:C1\] src/big\.ts ");
    s.assert_row(-1, "(1-21/62) hjkl move  enter ask  J/K comments  ? help");
    s.keys("20j");
    s.assert_row_matches(0, r"\[cursor L20:C1\] src/big\.ts ");
    s.assert_row_matches(20, r"^  20   20  ▶const v20 = 1;");
    s.assert_row(-1, "(3-23/62) hjkl move  enter ask  J/K comments  ? help");
    s.keys("19k");
    s.assert_row_matches(0, r"\[cursor L1:C1\] src/big\.ts ");
    s.assert_row_matches(2, r"^@@ ");
    s.assert_row(-1, "(1-21/62) hjkl move  enter ask  J/K comments  ? help");
}

/// F-NAV-10: config select of the current view / split value keeps the cursor and resets top.
#[test]
fn f_nav_10_config() {
    let mut s = Sim::builder().args(["HEAD", "--", "src/big.ts"]).size(80, 12).build();
    s.keys("G10k");
    s.assert_row_matches(-1, r"^\(50-58/62\) ");
    // view row, Enter on the committed value (full)
    s.keys("Cjjj");
    s.assert_matches(r"> view +\[full\]");
    s.keys("<Enter>");
    s.assert_matches(r"> view +\[full\]");
    s.assert_row_matches(0, r"\[full\] .*\[cursor L50:C1\] src/big\.ts ");
    s.assert_row_matches(-1, r"(^|\| )\(46-54/62\) ");
    s.keys("<Esc>G10k");
    s.assert_row_matches(-1, r"^\(50-58/62\) ");
    // split row (k clamps at the top row first), Enter on the committed value (off)
    s.keys("Ckkkkkkjj");
    s.assert_matches(r"> split +\[off\]");
    s.keys("<Enter>");
    s.assert_row_matches(0, r"\[unified\] .*\[cursor L50:C1\] src/big\.ts ");
    s.assert_row_matches(-1, r"(^|\| )\(46-54/62\) ");
}

/// F-NAV-10: m landing on the same path from another file index keeps the cursor and resets top then follows.
#[test]
fn f_nav_10_mode_other_index() {
    let mut s = Sim::builder().size(96, 12).build();
    s.git(&["add", "src/big.ts"]);
    s.keys("r<Tab>");
    s.assert_row_matches(0, r"^\[all\] .*\[2/4\] .*src/big\.ts ");
    s.keys("G10k");
    s.assert_row_matches(0, r"^\[all\] .*\[2/4\] \[cursor L50:C1\] src/big\.ts");
    s.assert_row_matches(-1, r"(^|\| )\(50-58/62\) ");
    s.keys("m");
    s.assert_row_matches(0, r"^\[staged\] .*\[1/1\] \[cursor L50:C1\] src/big\.ts");
    s.assert_row_matches(8, r"^  50   50  ▶const v50");
    s.assert_row_matches(-1, r"(^|\| )\(46-54/62\) ");
}

/// F-NAV-10: m landing on the same path keeps the cursor and resets top then follows.
#[test]
fn f_nav_10_mode() {
    let mut s = Sim::builder().size(96, 12).build();
    s.git(&["checkout", "--", "README.md"]);
    s.git(&["add", "src/big.ts"]);
    s.keys("r");
    s.assert_row_matches(0, r"^\[all\] .*\[1/3\] .*src/big\.ts ");
    s.keys("G10k");
    s.assert_row_matches(0, r"^\[all\] .*\[1/3\] \[cursor L50:C1\] src/big\.ts");
    s.assert_row_matches(-1, r"(^|\| )\(50-58/62\) ");
    s.keys("m");
    s.assert_row_matches(0, r"^\[staged\] .*\[1/1\] \[cursor L50:C1\] src/big\.ts");
    s.assert_row_matches(8, r"^  50   50  ▶const v50");
    s.assert_row_matches(-1, r"(^|\| )\(46-54/62\) ");
}

/// F-NAV-10: s at cols < 100 keeps the cursor and resets top (rows unchanged).
#[test]
fn f_nav_10_split_narrow() {
    let mut s = Sim::builder().args(["HEAD", "--", "src/big.ts"]).size(80, 12).build();
    s.keys("G10k2l");
    s.assert_row_matches(0, r"\[cursor L50:C3\] src/big\.ts ");
    s.assert_row(-1, "(50-58/62) hjkl move  enter ask  J/K comments  ? help");
    s.keys("s");
    s.assert_row_matches(0, r"\[cursor L50:C3\] src/big\.ts ");
    s.assert_row_matches(8, r"^  50   50  ▶const v50");
    s.assert_row(-1, "too narrow for split | (46-54/62) hjkl move  enter ask  J/K comments  ? help");
}

/// F-NAV-10: spec example - one file, Tab / S-Tab / picker Enter on the current file reset top, keep the cursor.
#[test]
fn f_nav_10_tab_picker() {
    let mut s = Sim::builder().args(["HEAD", "--", "src/big.ts"]).size(80, 12).build();
    s.keys("G10k3l");
    s.assert_row_matches(0, r"\[1/1\] \[cursor L50:C4\] src/big\.ts ");
    s.assert_row(-1, "(50-58/62) hjkl move  enter ask  J/K comments  ? help");
    s.keys("<Tab>");
    // top 0 then follow - cursor lands 2 rows above the viewport bottom
    s.assert_row_matches(0, r"\[1/1\] \[cursor L50:C4\] src/big\.ts ");
    s.assert_row_matches(8, r"^  50   50  ▶const v50");
    s.assert_row(-1, "(46-54/62) hjkl move  enter ask  J/K comments  ? help");
    s.keys("G10k");
    s.assert_row_matches(-1, r"^\(50-58/62\) ");
    s.keys("<S-Tab>");
    s.assert_row_matches(0, r"\[1/1\] \[cursor L50:C4\] src/big\.ts ");
    s.assert_row_matches(-1, r"^\(46-54/62\) ");
    s.keys("G10k");
    s.assert_row_matches(-1, r"^\(50-58/62\) ");
    s.keys("f");
    s.assert_contains(" Files (1/1)");
    s.keys("<Enter>");
    s.assert_not_contains("Files (");
    s.assert_row_matches(0, r"\[1/1\] \[cursor L50:C4\] src/big\.ts ");
    s.assert_row_matches(-1, r"^\(46-54/62\) ");
    // cursor row 7 (1-based) <= H-margin - top resets to 0
    s.keys("G55k");
    s.assert_row_matches(0, r"\[cursor L6:C4\] src/big\.ts ");
    s.assert_row_matches(-1, r"^\(5-13/62\) ");
    s.keys("<Tab>");
    s.assert_row_matches(0, r"\[cursor L6:C4\] src/big\.ts ");
    s.assert_row_matches(2, r"^@@ ");
    s.assert_row_matches(-1, r"^\(1-9/62\) ");
}
