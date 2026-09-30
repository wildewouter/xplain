//! Ported from `e2e/scenarios/u06-nav`.

use xplain_sim::Sim;

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
