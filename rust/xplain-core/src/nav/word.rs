//! Word motions `w` `b` `e` over row texts.
//!
//! Spec: F-CURSOR-04 (word motions cross rows). Oracle: `wordMove` in `src/app.tsx`.
//! Owner: component `nav` (B). Pure over a slice of row texts; no state access.

/// Which motion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordMove {
    Forward,
    Back,
    End,
}

/// From `(row, col)` move `count` times over `texts` (code text per row). Returns new `(row, col)`.
pub fn word_move(
    _texts: &dyn Fn(usize) -> String,
    _rows: usize,
    _from: (usize, usize),
    _kind: WordMove,
    _count: usize,
) -> (usize, usize) {
    todo!("F-CURSOR-04")
}
