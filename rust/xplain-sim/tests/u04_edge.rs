//! Ported from `e2e/scenarios/u04-edge`.

use xplain_sim::{CellExpect as C, Sim};

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
