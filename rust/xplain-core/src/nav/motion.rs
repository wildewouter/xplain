//! Cursor motions: vertical/horizontal moves, word motions dispatch, counts, pane switch.
//!
//! Spec: F-CURSOR-03 (vertical, col clamp, sticky end), F-CURSOR-04 (horizontal, wraps?), F-CURSOR-05 (count),
//! F-CURSOR-06 (`p`), F-CURSOR-08/09 (global keys, start position), F-NAV-01..04.
//! Owner: component `nav` (B). Must not: touch viewport `top` except via `viewport::follow`.

use crate::keys::{Key, KeyEvent};
use crate::state::{PaneChoice, State};

use super::word::{WordMove, word_move};
use super::{body_height, can_side, cursor_row, last_row, row_text, shown_col, viewport, visual};

const MAX_COUNT: u32 = 99999;

enum Motion {
    Left,
    Right,
    LineStart,
    FirstNonBlank,
    LineEnd,
    Word(WordMove),
    Down,
    Up,
    HalfDown,
    HalfUp,
    PageDown,
    PageUp,
    Top,
    Bottom,
    Pane,
}

fn classify(key: KeyEvent) -> Option<Motion> {
    if key.mods.ctrl {
        return None;
    }
    Some(match key.key {
        Key::Left | Key::Char('h') => Motion::Left,
        Key::Right | Key::Char('l') => Motion::Right,
        Key::Down | Key::Char('j') => Motion::Down,
        Key::Up | Key::Char('k') => Motion::Up,
        Key::PageDown | Key::Char(' ') => Motion::PageDown,
        Key::PageUp => Motion::PageUp,
        Key::Char('0') => Motion::LineStart,
        Key::Char('^') => Motion::FirstNonBlank,
        Key::Char('$') => Motion::LineEnd,
        Key::Char('w') => Motion::Word(WordMove::Forward),
        Key::Char('b') => Motion::Word(WordMove::Back),
        Key::Char('e') => Motion::Word(WordMove::End),
        Key::Char('d') => Motion::HalfDown,
        Key::Char('u') => Motion::HalfUp,
        Key::Char('g') => Motion::Top,
        Key::Char('G') => Motion::Bottom,
        Key::Char('p') => Motion::Pane,
        _ => return None,
    })
}

/// One motion key with the pending count. Returns true when the key was a motion (consumed). The count is
/// taken (cleared) only when the key is a motion; `nav::on_key` clears it for every other key.
pub fn apply(state: &mut State, key: KeyEvent) -> bool {
    let Some(m) = classify(key) else {
        return false;
    };
    let count = state.nav.count as usize;
    state.nav.count = 0;
    let n = count.max(1);
    let h = body_height(state.size);
    match m {
        Motion::Left => {
            let c = shown_col(state);
            set_col(state, c.saturating_sub(n));
        }
        Motion::Right => {
            let c = shown_col(state);
            let max = row_text(state, cursor_row(state)).chars().count().saturating_sub(1);
            set_col(state, (c + n).min(max));
        }
        Motion::LineStart => set_col(state, 0),
        Motion::FirstNonBlank => {
            let t = row_text(state, cursor_row(state));
            let c = t.chars().position(|c| !c.is_whitespace()).unwrap_or(0);
            set_col(state, c);
        }
        Motion::LineEnd => state.nav.sticky_end = true,
        Motion::Word(k) => {
            let from = (cursor_row(state), shown_col(state));
            let rows = state.rows.rows.len();
            let (r, c) = {
                let st = &*state;
                word_move(&|i| row_text(st, i), rows, from, k, n)
            };
            state.nav.row = r;
            set_col(state, c);
        }
        Motion::Down => move_rows(state, n as isize),
        Motion::Up => move_rows(state, -(n as isize)),
        Motion::HalfDown => move_rows(state, (n * (h / 2).max(1)) as isize),
        Motion::HalfUp => move_rows(state, -((n * (h / 2).max(1)) as isize)),
        Motion::PageDown => move_rows(state, (n * (h - 1)) as isize),
        Motion::PageUp => move_rows(state, -((n * (h - 1)) as isize)),
        Motion::Top => state.nav.row = 0,
        Motion::Bottom => {
            let last = last_row(state);
            state.nav.row = if count > 0 { (count - 1).min(last) } else { last };
        }
        Motion::Pane => {
            if can_side(state) {
                visual::end(state);
                state.nav.pane = match state.nav.pane {
                    PaneChoice::New => PaneChoice::Old,
                    PaneChoice::Old => PaneChoice::New,
                };
            }
        }
    }
    viewport::follow(state);
    true
}

fn set_col(state: &mut State, col: usize) {
    state.nav.col = col;
    state.nav.sticky_end = false;
}

fn move_rows(state: &mut State, delta: isize) {
    let cur = cursor_row(state) as isize;
    let last = last_row(state) as isize;
    state.nav.row = (cur + delta).clamp(0, last) as usize;
}

/// Digit handling for count prefix (`1-9` start, `0` continues) (F-CURSOR-05). Returns true when consumed.
pub fn push_count_digit(state: &mut State, digit: char) -> bool {
    let Some(d) = digit.to_digit(10) else {
        return false;
    };
    if digit == '0' && state.nav.count == 0 {
        return false;
    }
    state.nav.count = (state.nav.count.saturating_mul(10).saturating_add(d)).min(MAX_COUNT);
    true
}

#[cfg(test)]
mod tests {
    use super::super::testutil::*;
    use super::*;

    fn lines(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("line {i}")).collect()
    }

    fn st(n: usize) -> State {
        let l = lines(n);
        let r: Vec<&str> = l.iter().map(String::as_str).collect();
        state_with_lines(80, 24, &r)
    }

    #[test]
    fn f_cursor_03_vertical_moves_and_clamp() {
        let mut s = st(10); // rows: hunk + 10 lines = 11
        press(&mut s, "j");
        assert_eq!(s.nav.row, 1);
        press(&mut s, "5j");
        assert_eq!(s.nav.row, 6);
        press(&mut s, "99j");
        assert_eq!(s.nav.row, 10);
        press(&mut s, "k");
        assert_eq!(s.nav.row, 9);
        press(&mut s, "g");
        assert_eq!(s.nav.row, 0);
        press(&mut s, "G");
        assert_eq!(s.nav.row, 10);
        press(&mut s, "5G");
        assert_eq!(s.nav.row, 4);
        press(&mut s, "999G");
        assert_eq!(s.nav.row, 10);
        assert_eq!(s.nav.count, 0);
    }

    #[test]
    fn f_cursor_03_half_and_page() {
        let mut s = st(100); // H = 21, half 10, page 20
        press(&mut s, "d");
        assert_eq!(s.nav.row, 10);
        press(&mut s, "2d");
        assert_eq!(s.nav.row, 30);
        press(&mut s, "u");
        assert_eq!(s.nav.row, 20);
        press(&mut s, " ");
        assert_eq!(s.nav.row, 40);
        key(&mut s, Key::PageDown);
        assert_eq!(s.nav.row, 60);
        key(&mut s, Key::PageUp);
        assert_eq!(s.nav.row, 40);
        key(&mut s, Key::Down);
        key(&mut s, Key::Up);
        key(&mut s, Key::Up);
        assert_eq!(s.nav.row, 39);
    }

    #[test]
    fn f_cursor_03_desired_column_kept() {
        let mut s = state_with_lines(80, 24, &["abcdefgh", "ab", "abcdefgh"]);
        press(&mut s, "j");
        press(&mut s, "5l");
        assert_eq!(shown_col(&s), 5);
        press(&mut s, "j");
        assert_eq!(shown_col(&s), 1);
        press(&mut s, "j");
        assert_eq!(shown_col(&s), 5);
    }

    #[test]
    fn f_cursor_04_horizontal_moves() {
        let mut s = state_with_lines(80, 24, &["  hello"]);
        press(&mut s, "j");
        press(&mut s, "l");
        assert_eq!(shown_col(&s), 1);
        press(&mut s, "3l");
        assert_eq!(shown_col(&s), 4);
        press(&mut s, "99l");
        assert_eq!(shown_col(&s), 6);
        press(&mut s, "h");
        assert_eq!(shown_col(&s), 5);
        press(&mut s, "0");
        assert_eq!(shown_col(&s), 0);
        press(&mut s, "^");
        assert_eq!(shown_col(&s), 2);
        press(&mut s, "$");
        assert_eq!(shown_col(&s), 6);
        key(&mut s, Key::Left);
        assert_eq!(shown_col(&s), 5);
        key(&mut s, Key::Right);
        assert_eq!(shown_col(&s), 6);
    }

    #[test]
    fn f_cursor_04_caret_first_nonblank_all_blank() {
        let mut s = state_with_lines(80, 24, &["    "]);
        press(&mut s, "j$^");
        assert_eq!(shown_col(&s), 0);
    }

    #[test]
    fn f_cursor_04_dollar_is_sticky_until_horizontal() {
        let mut s = state_with_lines(80, 24, &["abcdef", "ab", "abcdefgh"]);
        press(&mut s, "j$");
        assert_eq!(shown_col(&s), 5);
        press(&mut s, "j");
        assert_eq!(shown_col(&s), 1);
        press(&mut s, "j");
        assert_eq!(shown_col(&s), 7);
        press(&mut s, "h");
        assert_eq!(shown_col(&s), 6);
        assert!(!s.nav.sticky_end);
        press(&mut s, "k");
        assert_eq!(shown_col(&s), 1);
    }

    #[test]
    fn f_cursor_04_word_motions_via_keys() {
        let mut s = state_with_lines(80, 24, &["foo bar", "baz"]);
        press(&mut s, "jw");
        assert_eq!((s.nav.row, shown_col(&s)), (1, 4));
        press(&mut s, "w");
        assert_eq!((s.nav.row, shown_col(&s)), (2, 0));
        press(&mut s, "b");
        assert_eq!((s.nav.row, shown_col(&s)), (1, 4));
        press(&mut s, "e");
        assert_eq!((s.nav.row, shown_col(&s)), (1, 6));
    }

    #[test]
    fn f_cursor_05_count_digits() {
        let mut s = st(30);
        press(&mut s, "1");
        assert_eq!(s.nav.count, 1);
        press(&mut s, "0");
        assert_eq!(s.nav.count, 10);
        press(&mut s, "j");
        assert_eq!(s.nav.row, 10);
        assert_eq!(s.nav.count, 0);
        press(&mut s, "0");
        assert_eq!(s.nav.count, 0); // plain `0` is a motion
        press(&mut s, "123456");
        assert_eq!(s.nav.count, 99999);
    }

    #[test]
    fn f_cursor_05_any_other_key_clears_count() {
        let mut s = st(30);
        press(&mut s, "3");
        let mut fx = Vec::new();
        assert!(!on_key_pub(&mut s, KeyEvent::ch('?'), &mut fx));
        press(&mut s, "j");
        assert_eq!(s.nav.row, 1);
        press(&mut s, "3");
        on_key_pub(&mut s, KeyEvent::ctrl('x'), &mut fx);
        press(&mut s, "j");
        assert_eq!(s.nav.row, 2);
        press(&mut s, "3i");
        press(&mut s, "j");
        assert_eq!(s.nav.row, 3);
    }

    fn on_key_pub(s: &mut State, k: KeyEvent, fx: &mut Vec<crate::Effect>) -> bool {
        super::super::on_key(s, k, fx)
    }

    #[test]
    fn f_nav_07_ctrl_ignored() {
        let mut s = st(30);
        let mut fx = Vec::new();
        assert!(on_key_pub(&mut s, KeyEvent::ctrl('d'), &mut fx));
        assert_eq!(s.nav.row, 0);
    }

    #[test]
    fn f_cursor_06_pane_toggle_only_in_split() {
        use crate::diff::LineKind::*;
        let mut s = st(3);
        press(&mut s, "p");
        assert_eq!(s.nav.pane, PaneChoice::New);
        let f = file_of(vec![
            crate::diff::DiffLine {
                kind: Del,
                old_no: Some(1),
                new_no: None,
                text: "x".into(),
                no_newline_marker: false,
            },
            crate::diff::DiffLine {
                kind: Add,
                old_no: None,
                new_no: Some(1),
                text: "y".into(),
                no_newline_marker: false,
            },
        ]);
        let mut s = state_with_files(120, 24, vec![f]);
        s.settings.split = true;
        crate::rows::ensure(&mut s);
        press(&mut s, "jv");
        assert!(s.nav.selection.is_some());
        press(&mut s, "p");
        assert_eq!(s.nav.pane, PaneChoice::Old);
        assert!(s.nav.selection.is_none());
        press(&mut s, "p");
        assert_eq!(s.nav.pane, PaneChoice::New);
    }
}
