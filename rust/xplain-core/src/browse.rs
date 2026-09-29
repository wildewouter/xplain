//! File viewer (browse): open a file read-only, keys, close.
//!
//! Spec: F-BROWSE-01 (open, NUL detection, line split, `cannot read` note), F-BROWSE-02 (keys: Esc back;
//! `s c m f` no-ops), F-LAYOUT-05 (rows are built by rows.rs), F-HEADER-02.
//! Owner: component `navops` (C). Emits `Effect::ReadFile`; result arrives as `Event::FileRead`.
//! Must not render.

use crate::effect::Fx;
use crate::errors::IoReason;
use crate::event::ReqId;
use crate::keys::KeyEvent;
use crate::state::State;

/// Start opening `path` (repo relative) in browse: allocate req, `Pending::Browse`, `Effect::ReadFile`.
pub fn open(_state: &mut State, _path: &str, _fx: &mut Fx) {
    todo!("F-BROWSE-01")
}

/// `Event::FileRead`: stale ids dropped; Err -> note `messages::cannot_read`; binary (NUL) handling;
/// success sets `state.browse`, bumps `files_gen`, cursor to row 0, closes search overlay.
pub fn on_file_read(_state: &mut State, _req: ReqId, _result: Result<Vec<u8>, IoReason>, _fx: &mut Fx) {
    todo!("F-BROWSE-01")
}

/// Keys that only exist in browse (Esc back to the diff, ignored `s c m f`). True when consumed.
pub fn on_key(_state: &mut State, _key: KeyEvent, _fx: &mut Fx) -> bool {
    todo!("F-BROWSE-02")
}

pub fn close(_state: &mut State) {
    todo!("F-BROWSE-02")
}
