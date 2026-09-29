//! Diff loading lifecycle and view toggles: load results, error/no-changes screens, mode/scope/split/theme
//! keys, reload, agent-triggered reload.
//!
//! Spec: F-MODE-01..05 (`m` cycle, git error screen, no changes), F-SCOPE-01/02 (`c`), F-LAYOUT-04 (`s` incl.
//! F-NAV-10 top reset), F-THEME-01 (`t`), F-RELOAD-01..03 (`r`, agent reload, cursor kept), F-NAV-10.
//! Oracle: `reload`, `cycleMode`, scope/theme code in `src/app.tsx`, `src/diff/load.ts`.
//! Owner: component `shell` (G). Uses `diff::parse_raw`, `jump::{remember,restore,open_file}`, `rows::ensure`.
//! Must not: render.

use crate::diff::RawDiff;
use crate::effect::Fx;
use crate::event::ReqId;
use crate::keys::KeyEvent;
use crate::state::State;

/// Emit `Effect::LoadDiff` for the current settings (allocates req, `Pending::Diff`).
pub fn request_load(_state: &mut State, _fx: &mut Fx) {
    todo!("F-MODE-01")
}

/// `s c m t r` in cursor context (and browse where the spec says ignore). True when consumed.
pub fn on_key(_state: &mut State, _key: KeyEvent, _fx: &mut Fx) -> bool {
    todo!("F-MODE-03, F-SCOPE-02, F-LAYOUT-04, F-THEME-01, F-RELOAD-01")
}

/// `Event::DiffLoaded`: stale dropped; Ok -> parse, replace files, bump `files_gen`, cursor per F-RELOAD-03 or
/// F-NAV-08 (first load: set `ready`), no-changes state; Err -> `LoadState::Error`.
pub fn on_diff_loaded(_state: &mut State, _req: ReqId, _result: Result<RawDiff, String>, _fx: &mut Fx) {
    todo!("F-MODE-04/05, F-RELOAD-03")
}

/// Agent `files_changed` (F-RELOAD-02): reload silently, keeping cursor.
pub fn on_files_changed(_state: &mut State, _paths: &[String], _fx: &mut Fx) {
    todo!("F-RELOAD-02")
}

/// Apply a setting change made from the config modal that affects the view (mode/split/full/theme),
/// including reloads and top reset rules (F-CFGUI-02).
pub fn apply_setting_change(_state: &mut State, _change: crate::config::ConfigChange, _fx: &mut Fx) {
    todo!("F-CFGUI-02, F-MODE-03, F-SCOPE-02")
}
