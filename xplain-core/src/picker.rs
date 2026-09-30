//! File picker modal (state + keys). Rendering is `view::modals`.
//!
//! Spec: F-FILES-01 (open `f`, entries, status letters), F-FILES-02 (keys, Enter opens, top reset case).
//! Owner: component `navops` (C). Must not render.

use crate::diff::Status;
use crate::effect::Fx;
use crate::keys::{Key, KeyEvent};
use crate::state::{Overlay, State};

/// Scroll state of the picker window (add fields here).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PickerUi {
    pub top: usize,
}

/// One picker row for the view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerEntry {
    pub status: Status,
    pub label: String,
    pub adds: u32,
    pub dels: u32,
}

pub fn entries(state: &State) -> Vec<PickerEntry> {
    state
        .files
        .iter()
        .map(|f| PickerEntry { status: f.status, label: f.display_path(), adds: f.adds, dels: f.dels })
        .collect()
}

/// List rows of the modal: modal height minus border, title and hint (F-FILES-01).
fn list_rows(state: &State) -> usize {
    let r = usize::from(state.size.rows);
    let h = r.min(5usize.max((state.files.len() + 4).min(r * 6 / 10)));
    h.saturating_sub(4).max(1)
}

fn sync_top(state: &mut State) {
    if let Overlay::Picker { sel } = state.overlay {
        state.picker.top = window(sel, state.files.len(), list_rows(state)).0;
    }
}

/// `f` in cursor context opens with the current file selected. True when consumed.
pub fn on_normal_key(state: &mut State, key: KeyEvent, _fx: &mut Fx) -> bool {
    if key.key != Key::Char('f') || key.mods.ctrl || key.mods.alt || state.browse.is_some() {
        return false;
    }
    state.nav.count = 0;
    let sel = state.nav.file_index.min(state.files.len().saturating_sub(1));
    state.overlay = Overlay::Picker { sel };
    sync_top(state);
    true
}

/// Key while `Overlay::Picker`.
pub fn on_key(state: &mut State, key: KeyEvent, _fx: &mut Fx) {
    let Overlay::Picker { sel } = state.overlay else { return };
    if key.mods.ctrl {
        return;
    }
    let last = state.files.len().saturating_sub(1);
    let half = (crate::nav::viewport::height(state) / 2).max(1);
    let to = match key.key {
        Key::Esc | Key::Char('q') | Key::Char('f') => {
            state.overlay = Overlay::None;
            return;
        }
        Key::Enter => {
            state.overlay = Overlay::None;
            crate::jump::open_file(state, sel);
            return;
        }
        Key::Char('d') => (sel + half).min(last),
        Key::Char('u') => sel.saturating_sub(half),
        Key::Char('j') | Key::Down => (sel + 1).min(last),
        Key::Char('k') | Key::Up => sel.saturating_sub(1),
        _ => return,
    };
    state.overlay = Overlay::Picker { sel: to };
    sync_top(state);
}

/// Visible window `[start, end)` of `entries` for a list of `height` visible rows (modal height minus
/// border, title, hint), keeping `sel` in view: centered on the selection, clamped to the ends.
pub fn window(sel: usize, total: usize, height: usize) -> (usize, usize) {
    let vis = height.max(1);
    let start = sel.saturating_sub(vis / 2).min(total.saturating_sub(vis));
    (start, (start + vis).min(total))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jump::testkit::*;

    fn press(s: &mut State, k: KeyEvent) {
        on_key(s, k, &mut Vec::new());
    }

    fn many(n: usize) -> State {
        state((0..n).map(|i| file(&format!("f{i}.rs"))).collect())
    }

    #[test]
    fn f_files_01_entries_status_and_counts() {
        let mut f = file("new.rs");
        f.old_path = Some("old.rs".into());
        f.status = Status::Renamed;
        let s = state(vec![f, file("b.rs")]);
        let e = entries(&s);
        assert_eq!(e[0].label, "old.rs -> new.rs");
        assert_eq!(e[0].status, Status::Renamed);
        assert_eq!((e[1].adds, e[1].dels, e[1].status), (1, 1, Status::Modified));
    }

    #[test]
    fn f_files_01_open_selects_current() {
        let mut s = many(3);
        s.nav.file_index = 2;
        s.nav.count = 5;
        assert!(on_normal_key(&mut s, KeyEvent::ch('f'), &mut Vec::new()));
        assert_eq!(s.overlay, Overlay::Picker { sel: 2 });
        assert_eq!(s.nav.count, 0);
        s.overlay = Overlay::None;
        assert!(!on_normal_key(&mut s, KeyEvent::ctrl('f'), &mut Vec::new()));
        s.browse = Some(crate::state::Browse { path: "x".into(), lines: vec![] });
        assert!(!on_normal_key(&mut s, KeyEvent::ch('f'), &mut Vec::new()));
    }

    #[test]
    fn f_files_01_empty_list_opens() {
        let mut s = state(Vec::new());
        assert!(on_normal_key(&mut s, KeyEvent::ch('f'), &mut Vec::new()));
        assert_eq!(s.overlay, Overlay::Picker { sel: 0 });
    }

    #[test]
    fn f_files_02_move_clamped_and_half_page() {
        let mut s = many(40);
        s.overlay = Overlay::Picker { sel: 0 };
        press(&mut s, KeyEvent::plain(Key::Up));
        assert_eq!(s.overlay, Overlay::Picker { sel: 0 });
        press(&mut s, KeyEvent::ch('j'));
        press(&mut s, KeyEvent::plain(Key::Down));
        assert_eq!(s.overlay, Overlay::Picker { sel: 2 });
        // 80x24: H = 21, half = 10
        press(&mut s, KeyEvent::ch('d'));
        assert_eq!(s.overlay, Overlay::Picker { sel: 12 });
        press(&mut s, KeyEvent::ch('u'));
        press(&mut s, KeyEvent::ch('u'));
        assert_eq!(s.overlay, Overlay::Picker { sel: 0 });
        s.overlay = Overlay::Picker { sel: 35 };
        press(&mut s, KeyEvent::ch('d'));
        assert_eq!(s.overlay, Overlay::Picker { sel: 39 });
    }

    #[test]
    fn f_files_02_close_keys_and_ctrl() {
        for k in [KeyEvent::plain(Key::Esc), KeyEvent::ch('q'), KeyEvent::ch('f')] {
            let mut s = many(2);
            s.overlay = Overlay::Picker { sel: 1 };
            press(&mut s, k);
            assert_eq!(s.overlay, Overlay::None);
            assert_eq!(s.nav.file_index, 0);
        }
        let mut s = many(2);
        s.overlay = Overlay::Picker { sel: 1 };
        for k in [KeyEvent::ctrl('f'), KeyEvent::ctrl('q'), KeyEvent::ctrl('d'), KeyEvent::ch('x')] {
            press(&mut s, k);
        }
        assert_eq!(s.overlay, Overlay::Picker { sel: 1 });
    }

    #[test]
    fn f_files_02_enter_opens_selected() {
        let mut s = many(3);
        s.overlay = Overlay::Picker { sel: 2 };
        press(&mut s, KeyEvent::plain(Key::Enter));
        assert_eq!(s.overlay, Overlay::None);
        assert_eq!(s.nav.file_index, 2);
        assert_eq!(s.nav.row, 3);
    }

    #[test]
    fn f_files_01_window_centered_and_clamped() {
        assert_eq!(window(0, 30, 10), (0, 10));
        assert_eq!(window(15, 30, 10), (10, 20));
        assert_eq!(window(29, 30, 10), (20, 30));
        assert_eq!(window(1, 3, 10), (0, 3));
        assert_eq!(window(0, 0, 10), (0, 0));
    }
}
