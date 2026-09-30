//! Shared assertions for the u07-search and u08-find ports (region text, cell lookup, chmod).
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use regex::Regex;
use xplain_sim::Sim;

/// Text of rows `a..=b` (negative from the bottom), each cut to terminal columns `c0..=c1`, joined by `\n`.
/// Same as the e2e `region` matcher: a row is right-trimmed first, wide char continuation cells are skipped.
#[track_caller]
pub fn region(s: &Sim, a: isize, b: isize, c0: usize, c1: usize) -> String {
    let scr = s.render();
    let n = scr.rows.len() as isize;
    let res = |r: isize| if r < 0 { n + r } else { r };
    let (a, b) = (res(a), res(b));
    assert!(0 <= a && a <= b && b < n, "bad region rows {a}-{b}");
    (a..=b)
        .map(|y| {
            let cells: Vec<(usize, char)> = scr.rows[y as usize]
                .iter()
                .enumerate()
                .filter(|(_, c)| c.width > 0)
                .map(|(x, c)| (x, c.ch))
                .collect();
            let text: String = cells.iter().map(|(_, c)| *c).collect::<String>().trim_end().to_string();
            let keep = text.chars().count();
            cells
                .iter()
                .take(keep)
                .filter(|(x, _)| (c0..=c1).contains(x))
                .map(|(_, c)| *c)
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[track_caller]
pub fn assert_region_contains(s: &Sim, rows: (isize, isize), cols: (usize, usize), want: &str) {
    let t = region(s, rows.0, rows.1, cols.0, cols.1);
    assert!(t.contains(want), "region {rows:?} {cols:?} lacks {want:?}, got {t:?}\n{}", s.dump());
}

#[track_caller]
pub fn assert_region_not_contains(s: &Sim, rows: (isize, isize), cols: (usize, usize), bad: &str) {
    let t = region(s, rows.0, rows.1, cols.0, cols.1);
    assert!(!t.contains(bad), "region {rows:?} {cols:?} has {bad:?}, got {t:?}\n{}", s.dump());
}

#[track_caller]
pub fn assert_region_matches(s: &Sim, rows: (isize, isize), cols: (usize, usize), pattern: &str) {
    let t = region(s, rows.0, rows.1, cols.0, cols.1);
    let re = Regex::new(&format!("(?m){pattern}")).unwrap_or_else(|e| panic!("bad regex /{pattern}/: {e}"));
    assert!(re.is_match(&t), "region {rows:?} {cols:?} does not match /{pattern}/, got {t:?}\n{}", s.dump());
}

#[track_caller]
pub fn assert_row_not_contains(s: &Sim, row: isize, bad: &str) {
    let r = s.row(row);
    assert!(!r.contains(bad), "row {row} has {bad:?}, got {r:?}\n{}", s.dump());
}

/// Cell position of the `nth` occurrence of `text` (within `row` only when given) plus `offset` columns.
#[track_caller]
pub fn cell_pos(s: &Sim, text: &str, nth: usize, offset: usize, row: Option<isize>) -> (usize, isize) {
    let scr = s.render();
    let found = match row {
        Some(r) => {
            let y = if r < 0 { scr.rows.len() as isize + r } else { r } as usize;
            xplain_sim::view::find_in_row(&scr, y, text, nth).map(|x| (x, y))
        }
        None => xplain_sim::view::find(&scr, text, nth).map(|p| (p.x, p.y)),
    };
    match found {
        Some((x, y)) => (x + offset, y as isize),
        None => panic!("text {text:?} (nth {nth}, row {row:?}) not on screen\n{}", s.dump()),
    }
}

/// `chmod` a repo file (real file system).
#[track_caller]
pub fn chmod(s: &Sim, path: &str, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    let p = s.repo().join(path);
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(mode))
        .unwrap_or_else(|e| panic!("chmod {}: {e}", p.display()));
}
