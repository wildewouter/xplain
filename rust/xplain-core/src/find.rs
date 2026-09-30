//! Find (`/`, `n`, `N`) and goto-line (`:`).
//!
//! Spec: F-FIND-01..03, F-GOTO-01/02.
//! Owner: component `navops` (C). Uses `rows::{find_all, search_texts}` and `nav::place`. Must not render
//! (highlights are drawn by view rows from `state.find.term`).

use crate::comments::PaneSide;
use crate::effect::Fx;
use crate::jump::{nearest, new_nos};
use crate::keys::{Key, KeyEvent};
use crate::rows;
use crate::state::{Overlay, PaneChoice, State};
use crate::textinput::{self, NewlinePolicy};

fn plain(key: &KeyEvent) -> bool {
    !key.mods.ctrl && !key.mods.alt
}

/// `/` `n` `N` `:` in cursor context (opens the input or jumps). True when consumed.
pub fn on_normal_key(state: &mut State, key: KeyEvent, _fx: &mut Fx) -> bool {
    if !plain(&key) {
        return false;
    }
    match key.key {
        Key::Char('/') => {
            state.nav.count = 0;
            state.overlay = Overlay::Find { text: String::new() };
            true
        }
        Key::Char(':') => {
            state.nav.count = 0;
            state.overlay = Overlay::Goto { text: String::new() };
            true
        }
        Key::Char(c @ ('n' | 'N')) => {
            state.nav.count = 0;
            let Some(term) = state.find.term.clone() else { return false };
            find_next(state, c == 'n', &term);
            true
        }
        _ => false,
    }
}

/// Rows matching `term` (smart-case substring over the row's search texts).
fn rows_matching(state: &State, term: &str) -> Vec<usize> {
    if term.is_empty() {
        return Vec::new();
    }
    state
        .rows
        .rows
        .iter()
        .enumerate()
        .filter(|(_, r)| rows::search_texts(r).iter().any(|t| !rows::find_all(t, term).is_empty()))
        .map(|(i, _)| i)
        .collect()
}

fn not_found(state: &mut State, term: &str) {
    state.set_note(format!("pattern not found: {term}"));
}

/// Cursor to `row`, column of the first hit (cursor pane text, else first row text), selection cleared.
fn go_row(state: &mut State, row: usize, term: &str) {
    let side = PaneSide::from(state.nav.pane);
    let col = state
        .rows
        .rows
        .get(row)
        .and_then(|r| {
            let code = rows::row_code(r, side);
            let hit = rows::find_all(&code, term).first().map(|h| h.0);
            hit.or_else(|| {
                let texts = rows::search_texts(r);
                texts.first().and_then(|t| rows::find_all(t, term).first().map(|h| h.0))
            })
        })
        .unwrap_or(0);
    state.nav.selection = None;
    state.nav.sticky_end = false;
    crate::nav::place(state, row, Some(col));
}

/// `n` (forward) / `N` (backward) with wrap.
fn find_next(state: &mut State, forward: bool, term: &str) {
    let ms = rows_matching(state, term);
    if ms.is_empty() {
        return not_found(state, term);
    }
    let cur = crate::nav::cursor_row(state);
    let row = if forward {
        ms.iter().copied().find(|x| *x > cur).unwrap_or(ms[0])
    } else {
        ms.iter().rev().copied().find(|x| *x < cur).unwrap_or(ms[ms.len() - 1])
    };
    go_row(state, row, term);
}

/// Key while `Overlay::Find` (typing, backspace, Enter jump, Esc cancel).
pub fn on_find_key(state: &mut State, key: KeyEvent, _fx: &mut Fx) {
    let Overlay::Find { text } = &mut state.overlay else { return };
    match key.key {
        Key::Esc => state.overlay = Overlay::None,
        Key::Enter => {
            let text = std::mem::take(text);
            state.overlay = Overlay::None;
            if text.is_empty() {
                state.find.term = None;
                return;
            }
            state.find.term = Some(text.clone());
            let ms = rows_matching(state, &text);
            if ms.is_empty() {
                return not_found(state, &text);
            }
            let cur = crate::nav::cursor_row(state);
            let row = ms.iter().copied().find(|x| *x >= cur).unwrap_or(ms[0]);
            go_row(state, row, &text);
        }
        Key::Backspace | Key::Delete => {
            text.pop();
        }
        Key::Char(c) if plain(&key) => {
            textinput::push(text, c.encode_utf8(&mut [0; 4]), NewlinePolicy::Collapse)
        }
        _ => {}
    }
}

/// Key while `Overlay::Goto`.
pub fn on_goto_key(state: &mut State, key: KeyEvent, _fx: &mut Fx) {
    let Overlay::Goto { text } = &mut state.overlay else { return };
    match key.key {
        Key::Esc => state.overlay = Overlay::None,
        Key::Enter => {
            let text = std::mem::take(text);
            state.overlay = Overlay::None;
            if !text.is_empty() {
                go_line(state, text.trim());
            }
        }
        Key::Backspace | Key::Delete => {
            text.pop();
        }
        Key::Char(c) if plain(&key) => textinput::push(text, c.encode_utf8(&mut [0; 4]), NewlinePolicy::Drop),
        _ => {}
    }
}

/// `goLine`: jump to new-side line `s` (nearest row when not in view).
fn go_line(state: &mut State, s: &str) {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        state.set_note(format!("not a line number: {s}"));
        return;
    }
    let n = s.parse::<u64>().unwrap_or(u64::MAX);
    if n < 1 {
        state.set_note(format!("not a line number: {s}"));
        return;
    }
    let nos = new_nos(&state.rows.rows);
    let Some(i) = nearest(&nos, n) else {
        state.set_note("no lines in view");
        return;
    };
    let top = nos.iter().flatten().copied().max().unwrap_or(0);
    if (state.settings.full || state.browse.is_some()) && n > u64::from(top) {
        state.set_note(format!("line {n} out of range (1-{top})"));
        return;
    }
    if let Some(found) = nos[i].filter(|v| u64::from(*v) != n) {
        state.set_note(format!("line {n} not in view, nearest L{found}"));
    }
    state.nav.focused_comment = None;
    state.nav.pane = PaneChoice::New;
    state.nav.selection = None;
    state.nav.sticky_end = false;
    crate::nav::place(state, i, Some(0));
}

/// Paste into the open find/goto input (newline runs collapse). False if neither is open.
pub fn on_paste(state: &mut State, text: &str) -> bool {
    match &mut state.overlay {
        Overlay::Find { text: t } => {
            textinput::push(t, text, NewlinePolicy::Collapse);
            true
        }
        Overlay::Goto { text: t } => {
            textinput::push(t, text, NewlinePolicy::Drop);
            true
        }
        _ => false,
    }
}

/// Rows whose search texts contain the active term (F-FIND-02).
pub fn match_rows(state: &State) -> Vec<usize> {
    state.find.term.as_deref().map(|t| rows_matching(state, t)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jump::testkit::*;

    fn fx() -> Fx {
        Vec::new()
    }

    fn type_text(s: &mut State, t: &str) {
        for c in t.chars() {
            on_find_key(s, KeyEvent::ch(c), &mut fx());
        }
    }

    // rows: 0 hunk, 1 "one", 2 "two", 3 del "three", 4 add "THREE", 5 "four", 6 "five"
    fn st() -> State {
        state(vec![file("a.rs")])
    }

    #[test]
    fn f_find_01_open_type_backspace_esc() {
        let mut s = st();
        s.nav.count = 3;
        assert!(on_normal_key(&mut s, KeyEvent::ch('/'), &mut fx()));
        assert_eq!(s.overlay, Overlay::Find { text: String::new() });
        assert_eq!(s.nav.count, 0);
        type_text(&mut s, "ab?");
        on_find_key(&mut s, KeyEvent::plain(Key::Backspace), &mut fx());
        on_find_key(&mut s, KeyEvent::ctrl('x'), &mut fx());
        on_find_key(&mut s, KeyEvent::plain(Key::Tab), &mut fx());
        assert_eq!(s.overlay, Overlay::Find { text: "ab".into() });
        s.find.term = Some("keep".into());
        on_find_key(&mut s, KeyEvent::plain(Key::Esc), &mut fx());
        assert_eq!(s.overlay, Overlay::None);
        assert_eq!(s.find.term.as_deref(), Some("keep"));
    }

    #[test]
    fn f_find_01_paste_collapses_newline_runs() {
        let mut s = st();
        assert!(!on_paste(&mut s, "x"));
        s.overlay = Overlay::Find { text: String::new() };
        assert!(on_paste(&mut s, "a\r\n\nb\nc"));
        assert_eq!(s.overlay, Overlay::Find { text: "a b c".into() });
        s.overlay = Overlay::Goto { text: String::new() };
        assert!(on_paste(&mut s, "1\n2"));
        assert_eq!(s.overlay, Overlay::Goto { text: "12".into() });
    }

    #[test]
    fn f_find_01_enter_empty_clears_term() {
        let mut s = st();
        s.find.term = Some("x".into());
        s.overlay = Overlay::Find { text: String::new() };
        on_find_key(&mut s, KeyEvent::plain(Key::Enter), &mut fx());
        assert_eq!(s.find.term, None);
        assert_eq!(s.overlay, Overlay::None);
        assert_eq!(s.nav.row, 0);
    }

    #[test]
    fn f_find_01_no_match_keeps_term_and_notes() {
        let mut s = st();
        s.overlay = Overlay::Find { text: "zzz".into() };
        on_find_key(&mut s, KeyEvent::plain(Key::Enter), &mut fx());
        assert_eq!(s.find.term.as_deref(), Some("zzz"));
        assert_eq!(s.note.as_deref(), Some("pattern not found: zzz"));
    }

    #[test]
    fn f_find_02_smart_case_and_match_rows() {
        let mut s = st();
        s.find.term = Some("three".into());
        assert_eq!(match_rows(&s), vec![3, 4]);
        s.find.term = Some("THREE".into());
        assert_eq!(match_rows(&s), vec![4]);
        s.find.term = None;
        assert!(match_rows(&s).is_empty());
    }

    #[test]
    fn f_find_03_enter_jumps_to_first_at_or_after_cursor_with_wrap() {
        let mut s = st();
        s.nav.row = 4;
        s.overlay = Overlay::Find { text: "o".into() };
        on_find_key(&mut s, KeyEvent::plain(Key::Enter), &mut fx());
        // rows with o: one(1) two(2) four(5) -> first >= 4 is 5
        assert_eq!(s.nav.row, 5);
        s.nav.row = 6;
        s.overlay = Overlay::Find { text: "ne".into() };
        on_find_key(&mut s, KeyEvent::plain(Key::Enter), &mut fx());
        assert_eq!(s.nav.row, 1);
        assert_eq!(s.nav.col, 1);
        assert_eq!(s.note, None);
    }

    #[test]
    fn f_find_03_n_and_shift_n_wrap() {
        let mut s = st();
        s.find.term = Some("o".into());
        s.nav.row = 1;
        assert!(on_normal_key(&mut s, KeyEvent::ch('n'), &mut fx()));
        assert_eq!(s.nav.row, 2);
        s.nav.row = 5;
        on_normal_key(&mut s, KeyEvent::ch('n'), &mut fx());
        assert_eq!(s.nav.row, 1);
        on_normal_key(&mut s, KeyEvent::ch('N'), &mut fx());
        assert_eq!(s.nav.row, 5);
        s.find.term = Some("qq".into());
        on_normal_key(&mut s, KeyEvent::ch('n'), &mut fx());
        assert_eq!(s.note.as_deref(), Some("pattern not found: qq"));
    }

    #[test]
    fn f_find_03_n_without_term_not_consumed() {
        let mut s = st();
        s.nav.count = 2;
        assert!(!on_normal_key(&mut s, KeyEvent::ch('n'), &mut fx()));
        assert_eq!(s.nav.count, 0);
        assert!(!on_normal_key(&mut s, KeyEvent::ctrl('n'), &mut fx()));
    }

    #[test]
    fn f_goto_01_input_keys() {
        let mut s = st();
        on_normal_key(&mut s, KeyEvent::ch(':'), &mut fx());
        assert_eq!(s.overlay, Overlay::Goto { text: String::new() });
        for c in "1x2".chars() {
            on_goto_key(&mut s, KeyEvent::ch(c), &mut fx());
        }
        on_goto_key(&mut s, KeyEvent::plain(Key::Backspace), &mut fx());
        assert_eq!(s.overlay, Overlay::Goto { text: "1x".into() });
        on_goto_key(&mut s, KeyEvent::plain(Key::Esc), &mut fx());
        assert_eq!(s.overlay, Overlay::None);
    }

    #[test]
    fn f_goto_02_exact_and_nearest() {
        let mut s = st();
        s.overlay = Overlay::Goto { text: " 4 ".into() };
        on_goto_key(&mut s, KeyEvent::plain(Key::Enter), &mut fx());
        assert_eq!(s.nav.row, 5);
        assert_eq!(s.nav.col, 0);
        assert_eq!(s.note, None);
        s.settings.full = false;
        go_line(&mut s, "9");
        assert_eq!(s.note.as_deref(), Some("line 9 not in view, nearest L5"));
        assert_eq!(s.nav.row, 6);
    }

    #[test]
    fn f_goto_02_errors() {
        let mut s = st();
        for bad in ["abc", "0", "1a", "-3", ""] {
            go_line(&mut s, bad);
            assert_eq!(s.note, Some(format!("not a line number: {bad}")));
        }
        s.settings.full = true;
        go_line(&mut s, "99");
        assert_eq!(s.note.as_deref(), Some("line 99 out of range (1-5)"));
        assert_eq!(s.nav.row, 0);
        // spaces only: note with empty text
        s.overlay = Overlay::Goto { text: "  ".into() };
        on_goto_key(&mut s, KeyEvent::plain(Key::Enter), &mut fx());
        assert_eq!(s.note.as_deref(), Some("not a line number: "));
        // no numbered rows
        let mut e = state(Vec::new());
        go_line(&mut e, "1");
        assert_eq!(e.note.as_deref(), Some("no lines in view"));
    }
}
