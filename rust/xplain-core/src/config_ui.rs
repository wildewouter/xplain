//! Config modal (`C`): state machine and save effects.
//!
//! Spec: F-CFGUI-01 (rows/choices, open), F-CFGUI-02 (keys, preview theme, apply, revert on close),
//! F-CFGUI-03 (save errors -> notes), F-CONFIG-05 (patch via `Effect::SaveConfig`). Oracle:
//! `src/components/ConfigModal.tsx`, `cfgOpen/cfgMove/cfgSelect/cfgClose` in `src/app.tsx`, `src/settings.ts`.
//! Owner: component `shell` (G). Uses `reload::apply_setting_change`. Must not render (`view::modals`).

use crate::effect::Fx;
use crate::errors::IoReason;
use crate::event::ReqId;
use crate::keys::KeyEvent;
use crate::state::State;

/// Row labels and choice labels for the view (F-CFGUI-01).
pub struct ConfigRow {
    pub label: &'static str,
    pub choices: Vec<String>,
}

pub fn rows(_state: &State) -> Vec<ConfigRow> {
    todo!("F-CFGUI-01")
}

/// `C` in cursor context. True when consumed.
pub fn on_normal_key(_state: &mut State, _key: KeyEvent, _fx: &mut Fx) -> bool {
    todo!("F-CFGUI-01")
}

/// Key while `Overlay::Config`.
pub fn on_key(_state: &mut State, _key: KeyEvent, _fx: &mut Fx) {
    todo!("F-CFGUI-02")
}

/// `Event::ConfigSaved`: clears the note on success, error notes per F-CFGUI-03 / `ConfigSaveError`.
pub fn on_saved(_state: &mut State, _req: ReqId, _result: Result<(), crate::config::ConfigSaveError>) {
    todo!("F-CONFIG-05, F-CFGUI-03")
}

/// Kept for symmetry of io reasons in notes.
pub fn save_error_note(_path: &str, _reason: IoReason) -> String {
    todo!("F-CFGUI-03")
}
