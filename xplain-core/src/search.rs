//! File search modal (`F`): repo file list, fzf-style filter, open in browse.
//!
//! Spec: F-SEARCH-01 (open, list load, matching, hits), F-SEARCH-02 (keys). Uses `fuzzy::match_paths`.
//! Owner: component `navops` (C). Emits `Effect::ListFiles` and, on Enter, delegates to `browse::open`.
//! Must not render (view draws from `state.overlay`).

use crate::effect::{Effect, Fx};
use crate::event::ReqId;
use crate::fuzzy::{PathHit, match_paths};
use crate::keys::{Key, KeyEvent};
use crate::state::{Overlay, Pending, SearchState, State};
use crate::textinput::{self, NewlinePolicy};

/// Private search state (add fields here).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SearchExt {
    /// Outstanding file-list request; results of any other id are stale.
    pub req: Option<ReqId>,
}

/// `F` in cursor context. True when consumed.
pub fn on_normal_key(state: &mut State, key: KeyEvent, fx: &mut Fx) -> bool {
    if key.key != Key::Char('F') || key.mods.ctrl || key.mods.alt {
        return false;
    }
    state.nav.count = 0;
    let req = state.alloc_req();
    state.loader.pending.insert(req, Pending::FileList);
    state.overlay =
        Overlay::Search(SearchState { ext: SearchExt { req: Some(req) }, ..SearchState::default() });
    fx.push(Effect::ListFiles { req, cwd: state.options.cwd.clone() });
    true
}

fn hit_count(s: &SearchState) -> usize {
    match_paths(&s.query, &s.files).len()
}

pub fn on_key(state: &mut State, key: KeyEvent, fx: &mut Fx) {
    let Overlay::Search(s) = &mut state.overlay else { return };
    let ctrl = key.mods.ctrl;
    match key.key {
        Key::Esc => state.overlay = Overlay::None,
        Key::Enter => {
            let path = hits(state).get(sel_of(state)).map(|h| h.path.clone());
            if let Some(path) = path {
                state.overlay = Overlay::None;
                crate::browse::open(state, &path, fx);
            }
        }
        Key::Down => next(s),
        Key::Char('n') if ctrl => next(s),
        Key::Up => s.sel = s.sel.saturating_sub(1),
        Key::Char('p') if ctrl => s.sel = s.sel.saturating_sub(1),
        Key::Backspace | Key::Delete => {
            s.query.pop();
            s.sel = 0;
        }
        Key::Char(c) if !ctrl && !key.mods.alt => {
            textinput::push(&mut s.query, c.encode_utf8(&mut [0; 4]), NewlinePolicy::Drop);
            s.sel = 0;
        }
        _ => {}
    }
}

fn next(s: &mut SearchState) {
    s.sel = (s.sel + 1).min(hit_count(s).saturating_sub(1));
}

fn sel_of(state: &State) -> usize {
    match &state.overlay {
        Overlay::Search(s) => s.sel,
        _ => 0,
    }
}

pub fn on_paste(state: &mut State, text: &str) -> bool {
    let Overlay::Search(s) = &mut state.overlay else { return false };
    textinput::push(&mut s.query, text, NewlinePolicy::Drop);
    s.sel = 0;
    true
}

/// `Event::FilesListed`: drop if `req` unknown/stale, fill `SearchState::files`.
pub fn on_files_listed(state: &mut State, req: ReqId, files: Vec<String>) {
    if !matches!(state.loader.pending.remove(&req), Some(Pending::FileList)) {
        return;
    }
    if let Overlay::Search(s) = &mut state.overlay
        && s.ext.req == Some(req)
    {
        s.ext.req = None;
        s.files = files;
        let n = hit_count(s);
        s.sel = s.sel.min(n.saturating_sub(1));
    }
}

/// Current hits for the view (`fuzzy::match_paths` over `SearchState::files`).
pub fn hits(state: &State) -> Vec<PathHit> {
    match &state.overlay {
        Overlay::Search(s) => match_paths(&s.query, &s.files),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jump::testkit::*;

    fn open(s: &mut State) -> (ReqId, Fx) {
        let mut fx = Vec::new();
        assert!(on_normal_key(s, KeyEvent::ch('F'), &mut fx));
        let req = ReqId(s.loader.next_req);
        (req, fx)
    }

    fn search(s: &State) -> SearchState {
        match &s.overlay {
            Overlay::Search(x) => x.clone(),
            _ => SearchState::default(),
        }
    }

    fn press(s: &mut State, k: KeyEvent) {
        on_key(s, k, &mut Vec::new());
    }

    #[test]
    fn f_search_01_open_emits_list_files_with_pending() {
        let mut s = state(Vec::new());
        s.options.cwd = Some("/repo".into());
        s.nav.count = 3;
        let (req, fx) = open(&mut s);
        assert_eq!(fx, vec![Effect::ListFiles { req, cwd: Some("/repo".into()) }]);
        assert_eq!(s.loader.pending.get(&req), Some(&Pending::FileList));
        assert_eq!(s.nav.count, 0);
        assert!(search(&s).files.is_empty());
    }

    #[test]
    fn f_search_01_files_arrive_then_hits() {
        let mut s = state(Vec::new());
        let (req, _) = open(&mut s);
        on_files_listed(&mut s, req, vec!["a.txt".into(), "README.md".into(), "readme.md".into()]);
        assert!(s.loader.pending.is_empty());
        assert_eq!(hits(&s).len(), 3);
        for c in "rm".chars() {
            press(&mut s, KeyEvent::ch(c));
        }
        assert_eq!(hits(&s).len(), 2);
    }

    #[test]
    fn f_search_01_stale_result_dropped() {
        let mut s = state(Vec::new());
        let (old, _) = open(&mut s);
        s.overlay = Overlay::None;
        let (new, _) = open(&mut s);
        on_files_listed(&mut s, old, vec!["old".into()]);
        assert!(search(&s).files.is_empty());
        on_files_listed(&mut s, ReqId(999), vec!["unknown".into()]);
        assert!(search(&s).files.is_empty());
        on_files_listed(&mut s, new, vec!["new".into()]);
        assert_eq!(search(&s).files, vec!["new".to_string()]);
    }

    #[test]
    fn f_search_01_result_after_close_dropped() {
        let mut s = state(Vec::new());
        let (req, _) = open(&mut s);
        press(&mut s, KeyEvent::plain(Key::Esc));
        on_files_listed(&mut s, req, vec!["x".into()]);
        assert_eq!(s.overlay, Overlay::None);
        assert!(s.loader.pending.is_empty());
    }

    #[test]
    fn f_search_02_typing_backspace_resets_selection() {
        let mut s = state(Vec::new());
        let (req, _) = open(&mut s);
        on_files_listed(&mut s, req, vec!["a".into(), "ab".into(), "abc".into()]);
        press(&mut s, KeyEvent::plain(Key::Down));
        assert_eq!(search(&s).sel, 1);
        for c in "j?q ".chars() {
            press(&mut s, KeyEvent::ch(c));
        }
        assert_eq!(search(&s).query, "j?q ");
        assert_eq!(search(&s).sel, 0);
        press(&mut s, KeyEvent::plain(Key::Backspace));
        press(&mut s, KeyEvent::plain(Key::Delete));
        press(&mut s, KeyEvent::ctrl('x'));
        press(&mut s, KeyEvent::plain(Key::Left));
        assert_eq!(search(&s).query, "j?");
    }

    #[test]
    fn f_search_02_up_down_ctrl_np_clamped() {
        let mut s = state(Vec::new());
        let (req, _) = open(&mut s);
        on_files_listed(&mut s, req, vec!["a".into(), "b".into()]);
        press(&mut s, KeyEvent::ctrl('n'));
        press(&mut s, KeyEvent::plain(Key::Down));
        assert_eq!(search(&s).sel, 1);
        press(&mut s, KeyEvent::ctrl('p'));
        assert_eq!(search(&s).sel, 0);
        press(&mut s, KeyEvent::plain(Key::Up));
        assert_eq!(search(&s).sel, 0);
        // plain n/p type
        press(&mut s, KeyEvent::ch('n'));
        assert_eq!(search(&s).query, "n");
    }

    #[test]
    fn f_search_02_enter_opens_hit_in_browse() {
        let mut s = state(Vec::new());
        let (req, _) = open(&mut s);
        on_files_listed(&mut s, req, vec!["a.txt".into(), "b.txt".into()]);
        press(&mut s, KeyEvent::plain(Key::Down));
        let mut fx = Vec::new();
        on_key(&mut s, KeyEvent::plain(Key::Enter), &mut fx);
        assert_eq!(s.overlay, Overlay::None);
        assert!(matches!(fx.as_slice(), [Effect::ReadFile { path, .. }] if path == "b.txt"));
    }

    #[test]
    fn f_search_02_enter_without_hits_and_esc() {
        let mut s = state(Vec::new());
        open(&mut s);
        let mut fx = Vec::new();
        on_key(&mut s, KeyEvent::plain(Key::Enter), &mut fx);
        assert!(fx.is_empty());
        assert!(matches!(s.overlay, Overlay::Search(_)));
        press(&mut s, KeyEvent::plain(Key::Esc));
        assert_eq!(s.overlay, Overlay::None);
    }

    #[test]
    fn f_search_02_paste_strips_newlines() {
        let mut s = state(Vec::new());
        assert!(!on_paste(&mut s, "x"));
        open(&mut s);
        assert!(on_paste(&mut s, "ab\ncd"));
        assert_eq!(search(&s).query, "abcd");
    }
}
