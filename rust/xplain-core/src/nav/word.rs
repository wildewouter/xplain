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

use crate::textutil::char_class;

fn chars_of(texts: &dyn Fn(usize) -> String, r: usize) -> Vec<char> {
    texts(r).chars().collect()
}

fn cls(t: &[char], i: usize) -> u8 {
    char_class(t.get(i).copied())
}

/// From `(row, col)` move `count` times over `texts` (code text per row). Returns new `(row, col)`.
pub fn word_move(
    texts: &dyn Fn(usize) -> String,
    rows: usize,
    from: (usize, usize),
    kind: WordMove,
    count: usize,
) -> (usize, usize) {
    let last = rows.saturating_sub(1);
    let (mut r, mut c) = from;
    for _ in 0..count {
        let before = (r, c);
        (r, c) = match kind {
            WordMove::Forward => forward(texts, last, r, c),
            WordMove::Back => back(texts, r, c),
            WordMove::End => end(texts, last, r, c),
        };
        if (r, c) == before {
            break;
        }
    }
    (r, c)
}

fn forward(texts: &dyn Fn(usize) -> String, last: usize, mut r: usize, mut c: usize) -> (usize, usize) {
    let mut t = chars_of(texts, r);
    let k0 = cls(&t, c);
    if k0 != 0 {
        while c < t.len() && cls(&t, c) == k0 {
            c += 1;
        }
    }
    loop {
        while c < t.len() && cls(&t, c) == 0 {
            c += 1;
        }
        if c < t.len() {
            break;
        }
        if r >= last {
            c = t.len().saturating_sub(1);
            break;
        }
        r += 1;
        t = chars_of(texts, r);
        c = 0;
        if t.is_empty() {
            break;
        }
    }
    (r, c)
}

fn back(texts: &dyn Fn(usize) -> String, mut r: usize, c: usize) -> (usize, usize) {
    let mut c0 = c as isize - 1;
    let mut done = false;
    loop {
        while c0 < 0 {
            if r == 0 {
                c0 = 0;
                done = true;
                break;
            }
            r -= 1;
            let len = chars_of(texts, r).len();
            c0 = len as isize - 1;
            if len == 0 {
                c0 = 0;
                done = true;
                break;
            }
        }
        if done {
            break;
        }
        let t = chars_of(texts, r);
        if cls(&t, c0 as usize) == 0 {
            c0 -= 1;
        } else {
            break;
        }
    }
    let mut c0 = c0.max(0) as usize;
    if !done {
        let t = chars_of(texts, r);
        let k0 = cls(&t, c0);
        while c0 > 0 && cls(&t, c0 - 1) == k0 {
            c0 -= 1;
        }
    }
    (r, c0)
}

fn end(texts: &dyn Fn(usize) -> String, last: usize, mut r: usize, c: usize) -> (usize, usize) {
    let mut c0 = c + 1;
    let mut t = chars_of(texts, r);
    'outer: loop {
        while c0 >= t.len() {
            if r >= last {
                c0 = t.len().saturating_sub(1);
                break 'outer;
            }
            r += 1;
            t = chars_of(texts, r);
            c0 = 0;
        }
        if c0 < t.len() && cls(&t, c0) == 0 {
            c0 += 1;
        } else {
            break;
        }
    }
    let k0 = cls(&t, c0);
    if k0 != 0 {
        while c0 + 1 < t.len() && cls(&t, c0 + 1) == k0 {
            c0 += 1;
        }
    }
    (r, c0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mv(lines: &[&str], from: (usize, usize), k: WordMove, n: usize) -> (usize, usize) {
        let v: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
        word_move(&|i| v.get(i).cloned().unwrap_or_default(), v.len(), from, k, n)
    }

    #[test]
    fn f_cursor_04_w_next_word_start() {
        let l = ["foo bar.baz"];
        assert_eq!(mv(&l, (0, 0), WordMove::Forward, 1), (0, 4));
        assert_eq!(mv(&l, (0, 4), WordMove::Forward, 1), (0, 7));
        assert_eq!(mv(&l, (0, 0), WordMove::Forward, 3), (0, 8));
    }

    #[test]
    fn f_cursor_04_w_crosses_rows_stops_at_empty_and_last() {
        let l = ["foo", "  bar", "", "x"];
        assert_eq!(mv(&l, (0, 0), WordMove::Forward, 1), (1, 2));
        assert_eq!(mv(&l, (1, 2), WordMove::Forward, 1), (2, 0));
        assert_eq!(mv(&l, (2, 0), WordMove::Forward, 1), (3, 0));
        assert_eq!(mv(&l, (3, 0), WordMove::Forward, 5), (3, 0));
        assert_eq!(mv(&["ab cd"], (0, 3), WordMove::Forward, 1), (0, 4));
    }

    #[test]
    fn f_cursor_04_b_previous_word_start() {
        let l = ["foo bar", "  baz"];
        assert_eq!(mv(&l, (0, 6), WordMove::Back, 1), (0, 4));
        assert_eq!(mv(&l, (0, 4), WordMove::Back, 1), (0, 0));
        assert_eq!(mv(&l, (1, 2), WordMove::Back, 1), (0, 4));
        assert_eq!(mv(&l, (0, 0), WordMove::Back, 1), (0, 0));
    }

    #[test]
    fn f_cursor_04_b_stops_at_empty_row() {
        let l = ["foo", "", "bar"];
        assert_eq!(mv(&l, (2, 0), WordMove::Back, 1), (1, 0));
        assert_eq!(mv(&l, (1, 0), WordMove::Back, 1), (0, 0));
    }

    #[test]
    fn f_cursor_04_e_end_of_word() {
        let l = ["foo bar", "x"];
        assert_eq!(mv(&l, (0, 0), WordMove::End, 1), (0, 2));
        assert_eq!(mv(&l, (0, 2), WordMove::End, 1), (0, 6));
        assert_eq!(mv(&l, (0, 6), WordMove::End, 1), (1, 0));
        assert_eq!(mv(&l, (1, 0), WordMove::End, 1), (1, 0));
    }

    #[test]
    fn f_cursor_04_e_terminates_on_trailing_blank() {
        assert_eq!(mv(&["a "], (0, 0), WordMove::End, 1), (0, 1));
    }

    #[test]
    fn f_cursor_05_count_repeats() {
        let l = ["a b c d"];
        assert_eq!(mv(&l, (0, 0), WordMove::Forward, 2), (0, 4));
        assert_eq!(mv(&l, (0, 0), WordMove::Forward, 99999), (0, 6));
    }
}
