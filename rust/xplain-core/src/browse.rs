//! File viewer (browse): open a file read-only, keys, close.
//!
//! Spec: F-BROWSE-01 (open, NUL detection, line split, `cannot read` note), F-BROWSE-02 (keys: Esc back;
//! `s c m f` no-ops), F-LAYOUT-05 (rows are built by rows.rs), F-HEADER-02.
//! Owner: component `navops` (C). Emits `Effect::ReadFile`; result arrives as `Event::FileRead`.
//! Must not render.

use crate::effect::{Effect, Fx};
use crate::errors::IoReason;
use crate::event::ReqId;
use crate::keys::{Key, KeyEvent};
use crate::state::{Browse, Overlay, Pending, State};

const BINARY_MSG: &str = "binary file, not shown";

/// `<cwd>/<path>` with `--cwd`, else `path` as given.
fn full_path(state: &State, path: &str) -> String {
    match state.options.cwd.as_deref() {
        Some(c) if !c.is_empty() => format!("{}/{path}", c.trim_end_matches('/')),
        _ => path.to_string(),
    }
}

/// Start opening `path` (repo relative) in browse: allocate req, `Pending::Browse`, `Effect::ReadFile`.
pub fn open(state: &mut State, path: &str, fx: &mut Fx) {
    let req = state.alloc_req();
    state.loader.pending.insert(req, Pending::Browse { path: path.to_string() });
    fx.push(Effect::ReadFile { req, path: full_path(state, path) });
}

/// File text as browse lines: NUL in the first 8000 bytes = binary; one trailing `\n` dropped.
fn split_lines(bytes: &[u8]) -> Vec<String> {
    if bytes.iter().take(8000).any(|b| *b == 0) {
        return vec![BINARY_MSG.to_string()];
    }
    let text = String::from_utf8_lossy(bytes);
    let text = text.strip_suffix('\n').unwrap_or(&text);
    text.split('\n').map(str::to_string).collect()
}

/// `Event::FileRead`: stale ids dropped; Err -> note `messages::cannot_read`; binary (NUL) handling;
/// success sets `state.browse`, bumps `files_gen`, cursor to row 0, closes search overlay.
pub fn on_file_read(state: &mut State, req: ReqId, result: Result<Vec<u8>, IoReason>, _fx: &mut Fx) {
    let Some(Pending::Browse { path }) = state.loader.pending.remove(&req) else { return };
    let same = state.browse.as_ref().is_some_and(|b| b.path == path);
    let bytes = match result {
        Ok(b) => b,
        Err(reason) => {
            state.note = Some(crate::messages::cannot_read(&full_path(state, &path), reason));
            return;
        }
    };
    let lines = split_lines(&bytes);
    if matches!(state.overlay, Overlay::Search(_)) {
        state.overlay = Overlay::None;
    }
    if same {
        // same file opened again: cursor stays on its line (F-RELOAD-03)
        let memo = crate::jump::remember(state);
        state.browse = Some(Browse { path, lines });
        state.files_gen += 1;
        crate::jump::restore(state, &memo);
        return;
    }
    state.browse = Some(Browse { path, lines });
    state.files_gen += 1;
    state.nav.focused_comment = None;
    crate::rows::ensure(state);
    crate::jump::initial_position(state);
}

/// Keys that only exist in browse (Esc back to the diff, ignored `s c m f`). True when consumed.
pub fn on_key(state: &mut State, key: KeyEvent, _fx: &mut Fx) -> bool {
    if state.browse.is_none() || key.mods.ctrl {
        return false;
    }
    match key.key {
        // Esc unpicks / unfocuses / ends a selection first (F-CURSOR-10): those steps belong to other modules
        Key::Esc
            if state.nav.focused_comment.is_none()
                && state.nav.selection.is_none()
                && state.thread.picked_block.is_none() =>
        {
            state.nav.count = 0;
            close(state);
            true
        }
        Key::Char('s' | 'c' | 'm' | 'f') | Key::Tab | Key::BackTab if !key.mods.alt => {
            state.nav.count = 0;
            true
        }
        _ => false,
    }
}

pub fn close(state: &mut State) {
    if state.browse.take().is_none() {
        return;
    }
    state.files_gen += 1;
    state.nav.selection = None;
    state.nav.focused_comment = None;
    crate::rows::ensure(state);
    crate::jump::initial_position(state);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jump::testkit::*;

    fn read(s: &mut State, path: &str, bytes: &[u8]) {
        let mut fx = Vec::new();
        open(s, path, &mut fx);
        let req = ReqId(s.loader.next_req);
        on_file_read(s, req, Ok(bytes.to_vec()), &mut fx);
    }

    fn lines(s: &State) -> Vec<String> {
        s.browse.as_ref().map(|b| b.lines.clone()).unwrap_or_default()
    }

    #[test]
    fn f_browse_01_open_emits_read_with_cwd_join() {
        let mut s = state(Vec::new());
        let mut fx = Vec::new();
        open(&mut s, "a.txt", &mut fx);
        assert!(matches!(fx.as_slice(), [Effect::ReadFile { path, .. }] if path == "a.txt"));
        s.options.cwd = Some("/r/".into());
        fx.clear();
        open(&mut s, "d/a.txt", &mut fx);
        assert!(matches!(fx.as_slice(), [Effect::ReadFile { path, .. }] if path == "/r/d/a.txt"));
        assert_eq!(s.loader.pending.len(), 2);
    }

    #[test]
    fn f_browse_01_line_splitting() {
        assert_eq!(split_lines(b"a\nb\n"), vec!["a", "b"]);
        assert_eq!(split_lines(b"a\n\n"), vec!["a", ""]);
        assert_eq!(split_lines(b""), vec![""]);
        assert_eq!(split_lines(b"\n"), vec![""]);
        assert_eq!(split_lines(b"a\nb"), vec!["a", "b"]);
    }

    #[test]
    fn f_browse_01_nul_in_first_8000_is_binary() {
        assert_eq!(split_lines(b"ab\0cd"), vec![BINARY_MSG]);
        let mut big = vec![b'x'; 8000];
        big.push(0);
        assert_eq!(split_lines(&big).len(), 1);
        assert_ne!(split_lines(&big), vec![BINARY_MSG]);
    }

    #[test]
    fn f_browse_01_success_sets_browse_row0_and_closes_search() {
        let mut s = state(vec![file("a.rs")]);
        s.nav.row = 4;
        s.overlay = Overlay::Search(Default::default());
        let gen0 = s.files_gen;
        read(&mut s, "x.txt", b"l1\nl2\n");
        assert_eq!(lines(&s), vec!["l1", "l2"]);
        assert_eq!(s.overlay, Overlay::None);
        assert!(s.files_gen > gen0);
        assert_eq!((s.nav.row, s.nav.col, s.nav.top), (0, 0, 0));
        assert!(s.loader.pending.is_empty());
    }

    #[test]
    fn f_browse_01_read_error_note_and_no_browse() {
        let mut s = state(Vec::new());
        s.options.cwd = Some("/r".into());
        let mut fx = Vec::new();
        open(&mut s, "a.txt", &mut fx);
        let req = ReqId(s.loader.next_req);
        on_file_read(&mut s, req, Err(IoReason::NotFound), &mut fx);
        assert_eq!(s.note.as_deref(), Some("cannot read /r/a.txt: not found"));
        assert!(s.browse.is_none());
    }

    #[test]
    fn f_browse_01_stale_or_unknown_req_dropped() {
        let mut s = state(Vec::new());
        on_file_read(&mut s, ReqId(42), Ok(b"x".to_vec()), &mut Vec::new());
        assert!(s.browse.is_none());
        assert!(s.note.is_none());
    }

    #[test]
    fn f_browse_01_reopen_same_file_keeps_cursor() {
        let mut s = state(Vec::new());
        read(&mut s, "x.txt", b"a\nb\nc\n");
        s.nav.row = 2;
        read(&mut s, "x.txt", b"x\ny\nz\nw\n");
        assert_eq!(s.nav.row, 2); // same line number, rows replaced
    }

    #[test]
    fn f_browse_02_esc_back_to_diff_position() {
        let mut s = state(vec![file("a.rs")]);
        read(&mut s, "x.txt", b"a\nb\n");
        let mut fx = Vec::new();
        assert!(on_key(&mut s, KeyEvent::plain(Key::Esc), &mut fx));
        assert!(s.browse.is_none());
        assert_eq!(s.nav.row, 3); // first change, scope full
    }

    #[test]
    fn f_browse_02_esc_yields_to_selection_and_focus() {
        let mut s = state(Vec::new());
        read(&mut s, "x.txt", b"a\n");
        s.nav.focused_comment = Some("q1".into());
        assert!(!on_key(&mut s, KeyEvent::plain(Key::Esc), &mut Vec::new()));
        assert!(s.browse.is_some());
    }

    #[test]
    fn f_browse_02_ignored_keys() {
        let mut s = state(Vec::new());
        read(&mut s, "x.txt", b"a\n");
        for c in ['s', 'c', 'm', 'f'] {
            s.nav.count = 3;
            assert!(on_key(&mut s, KeyEvent::ch(c), &mut Vec::new()));
            assert_eq!(s.nav.count, 0);
        }
        assert!(on_key(&mut s, KeyEvent::plain(Key::Tab), &mut Vec::new()));
        assert!(on_key(&mut s, KeyEvent::plain(Key::BackTab), &mut Vec::new()));
        assert!(!on_key(&mut s, KeyEvent::ch('j'), &mut Vec::new()));
        assert!(!on_key(&mut s, KeyEvent::ctrl('f'), &mut Vec::new()));
        let mut d = state(Vec::new());
        assert!(!on_key(&mut d, KeyEvent::ch('f'), &mut Vec::new()));
    }
}
