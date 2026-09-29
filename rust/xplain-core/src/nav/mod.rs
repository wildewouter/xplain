//! Cursor, selection and viewport: keys and the primitives other modules use to move the cursor.
//!
//! Spec: F-CURSOR-01..10, F-VISUAL-01/02 (state part), F-NAV-01..04, F-NAV-07, F-NAV-09, F-NAV-10.
//! Oracle: `src/app.tsx` (cur/col/anchor/hoff/off handling, wordMove, fitOff).
//! Owner: component `nav` (B). Submodules registered here up front.
//! Must not: handle `]` `[` Tab (jump.rs), find/goto (find.rs), comment keys (thread.rs/editor.rs), or render.
//! Row heights of comment/editor boxes come from `thread_layout::row_extra_height` (component comments).

pub mod motion;
pub mod viewport;
pub mod visual;
pub mod word;

use crate::comments::PaneSide;
use crate::effect::Fx;
use crate::keys::{Key, KeyEvent};
use crate::screen::Size;
use crate::state::{PaneChoice, State};

/// Viewport height H = max(3, rows-3) (F-LAYOUT-01).
pub fn body_height(size: Size) -> usize {
    usize::from(size.rows).saturating_sub(3).max(3)
}

/// Whether the two-pane cursor is in play: split effective, diff view, a file is shown (`canSide`).
pub fn can_side(state: &State) -> bool {
    crate::rows::effective_split(state)
        && state.browse.is_none()
        && state.files.get(state.nav.file_index).is_some()
}

/// Chosen pane (`p`): always `New` unless [`can_side`].
pub fn side(state: &State) -> PaneSide {
    if can_side(state) { state.nav.pane.into() } else { PaneSide::New }
}

/// Last row index (0 when there are no rows).
pub fn last_row(state: &State) -> usize {
    state.rows.rows.len().saturating_sub(1)
}

/// Cursor row clamped to the rows.
pub fn cursor_row(state: &State) -> usize {
    state.nav.row.min(last_row(state))
}

/// Tab-expanded code text of row `i` for the chosen pane (`tx`).
pub fn row_text(state: &State, i: usize) -> String {
    state.rows.rows.get(i).map(|r| crate::rows::row_code(r, side(state))).unwrap_or_default()
}

/// Clear the pending count prefix (F-CURSOR-05); the shell calls it for keys that never reach `on_key`.
pub fn clear_count(state: &mut State) {
    state.nav.count = 0;
}

/// Handle a key in cursor/visual/browse context: hjkl w b e 0 ^ $ d u Space PgUp PgDn g G arrows, digits
/// (count), `p`, `v` `V`, Esc chain (F-CURSOR-10, selection end). Ctrl combos ignored (F-NAV-07). Returns true
/// when consumed. Runs after overlays and comment focus had their turn. Home/End are unbound (oracle).
/// Keys not handled here (returns false) clear the pending count.
pub fn on_key(state: &mut State, key: KeyEvent, _fx: &mut Fx) -> bool {
    if !key.mods.ctrl
        && let Key::Char(d) = key.key
        && motion::push_count_digit(state, d)
    {
        return true;
    }
    if key.mods.ctrl {
        state.nav.count = 0;
        return true;
    }
    // motion keys unfocus a focused comment, then act (F-COMMENT-06)
    let unfocusing = match key.key {
        Key::Left | Key::Right | Key::Up | Key::Down | Key::PageUp | Key::PageDown => true,
        Key::Char(c) => "hjklwbevVdugG0$^[] p".contains(c),
        _ => false,
    };
    if unfocusing {
        state.nav.focused_comment = None;
    }
    match key.key {
        Key::Esc => {
            state.nav.count = 0;
            if state.thread.picked_block.take().is_some() {
                return true;
            }
            if state.nav.focused_comment.take().is_some() {
                return true;
            }
            visual::end(state);
            true
        }
        Key::Char('v') | Key::Char('V') => {
            state.nav.count = 0;
            visual::toggle(state, key.key == Key::Char('V'));
            true
        }
        Key::Char('i') => {
            state.nav.count = 0;
            true
        }
        _ => {
            if motion::apply(state, key) {
                return true;
            }
            state.nav.count = 0;
            false
        }
    }
}

/// Put the cursor on `row` (clamped), optionally at `col`, clamp col, no selection change, then follow.
/// Used by find/goto/jump/comments/reload.
pub fn place(state: &mut State, row: usize, col: Option<usize>) {
    state.nav.row = row.min(last_row(state));
    if let Some(c) = col {
        state.nav.col = c;
        state.nav.sticky_end = false;
    }
    viewport::follow(state);
}

/// Shown (clamped) 0-based column of the cursor (F-HEADER-03).
pub fn shown_col(state: &State) -> usize {
    let len = row_text(state, cursor_row(state)).chars().count();
    let max = len.saturating_sub(1);
    if state.nav.sticky_end { max } else { state.nav.col.min(max) }
}

/// Pane the cursor really sits in (`rows::pane_of` on the current row and `nav.pane`).
pub fn cursor_pane(state: &State) -> PaneSide {
    match state.rows.rows.get(cursor_row(state)) {
        Some(r) => crate::rows::pane_of(r, side(state)),
        None => PaneSide::New,
    }
}

/// Keep the cursor legal after rows changed (row < len, col clamp, pane).
pub fn clamp_cursor(state: &mut State) {
    state.nav.row = cursor_row(state);
    if !can_side(state) {
        state.nav.pane = PaneChoice::New;
    }
    if let Some(s) = state.nav.selection.as_mut() {
        s.anchor_row = s.anchor_row.min(state.rows.rows.len().saturating_sub(1));
    }
    viewport::clamp_top(state);
}

/// State builder for unit tests of the nav component (`State::new` is not needed).
#[cfg(test)]
pub(crate) mod testutil {
    use super::*;
    use crate::diff::{DiffLine, FileDiff, Hunk, LineKind, Status};
    use crate::state::{LoadState, Overlay};

    /// Bare state at `cols`x`rows` with unified rows built from `lines` (text only, context lines).
    pub fn state_with_lines(cols: u16, rows: u16, lines: &[&str]) -> State {
        let dl: Vec<DiffLine> = lines
            .iter()
            .enumerate()
            .map(|(i, t)| DiffLine {
                kind: LineKind::Context,
                old_no: u32::try_from(i + 1).ok(),
                new_no: u32::try_from(i + 1).ok(),
                text: (*t).to_string(),
                no_newline_marker: false,
            })
            .collect();
        state_with_files(cols, rows, vec![file_of(dl)])
    }

    pub fn file_of(lines: Vec<DiffLine>) -> FileDiff {
        FileDiff {
            path: "f.txt".into(),
            old_path: None,
            status: Status::Modified,
            adds: 0,
            dels: 0,
            hunks: vec![Hunk { header: "@@ -1 +1 @@".into(), lines }],
            note: None,
        }
    }

    pub fn state_with_files(cols: u16, rows: u16, files: Vec<FileDiff>) -> State {
        let mut s = crate::state::testutil::fake_state();
        s.size = Size { cols, rows };
        s.load = LoadState::Ready;
        s.ready = true;
        s.files = files;
        s.files_gen += 1;
        s.overlay = Overlay::None;
        crate::rows::ensure(&mut s);
        s
    }

    pub fn press(s: &mut State, keys: &str) {
        let mut fx = Vec::new();
        for c in keys.chars() {
            on_key(s, KeyEvent::ch(c), &mut fx);
        }
    }

    pub fn key(s: &mut State, k: Key) -> bool {
        on_key(s, KeyEvent::plain(k), &mut Vec::new())
    }
}
