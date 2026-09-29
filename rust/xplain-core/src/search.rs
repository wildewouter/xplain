//! File search modal (`F`): repo file list, fzf-style filter, open in browse.
//!
//! Spec: F-SEARCH-01 (open, list load, matching, hits), F-SEARCH-02 (keys). Uses `fuzzy::match_paths`.
//! Owner: component `navops` (C). Emits `Effect::ListFiles` and, on Enter, delegates to `browse::open`.
//! Must not render (view draws from `state.overlay`).

use crate::effect::Fx;
use crate::event::ReqId;
use crate::fuzzy::PathHit;
use crate::keys::KeyEvent;
use crate::state::State;

/// Private search state (add fields here).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SearchExt {}

/// `F` in cursor context. True when consumed.
pub fn on_normal_key(_state: &mut State, _key: KeyEvent, _fx: &mut Fx) -> bool {
    todo!("F-SEARCH-01")
}

pub fn on_key(_state: &mut State, _key: KeyEvent, _fx: &mut Fx) {
    todo!("F-SEARCH-02")
}

pub fn on_paste(_state: &mut State, _text: &str) -> bool {
    todo!("F-SEARCH-02")
}

/// `Event::FilesListed`: drop if `req` unknown/stale, fill `SearchState::files`.
pub fn on_files_listed(_state: &mut State, _req: ReqId, _files: Vec<String>) {
    todo!("F-SEARCH-01")
}

/// Current hits for the view (`fuzzy::match_paths` over `SearchState::files`).
pub fn hits(_state: &State) -> Vec<PathHit> {
    todo!("F-SEARCH-01")
}
