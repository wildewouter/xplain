//! Config modal (`C`): state machine and save effects.
//!
//! Spec: F-CFGUI-01 (rows/choices, open), F-CFGUI-02 (keys, preview theme, apply, revert on close),
//! F-CFGUI-03 (save errors -> notes), F-CONFIG-05 (patch via `Effect::SaveConfig`). Oracle:
//! `src/components/ConfigModal.tsx`, `cfgOpen/cfgMove/cfgSelect/cfgClose` in `src/app.tsx`, `src/settings.ts`.
//! Owner: component `shell` (G). Uses `reload::apply_setting_change`. Must not render (`view::modals`).

use crate::config::{ConfigChange, ConfigSaveError};
use crate::effect::{Effect, Fx};
use crate::errors::{IoReason, fail_msg};
use crate::event::ReqId;
use crate::keys::{Key, KeyEvent};
use crate::options::DiffMode;
use crate::reload;
use crate::state::{ConfigModal, Overlay, Pending, State};
use crate::theme::ThemeId;

/// Row labels and choice labels for the view (F-CFGUI-01).
pub struct ConfigRow {
    pub label: &'static str,
    pub choices: Vec<String>,
}

const ROWS: usize = 6;

pub fn rows(_state: &State) -> Vec<ConfigRow> {
    let strs = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    vec![
        ConfigRow { label: "theme", choices: ThemeId::ALL.iter().map(|t| t.as_str().to_string()).collect() },
        ConfigRow { label: "mode", choices: DiffMode::ALL.iter().map(|m| m.as_str().to_string()).collect() },
        ConfigRow { label: "split", choices: strs(&["off", "on"]) },
        ConfigRow { label: "view", choices: strs(&["full", "changes"]) },
        ConfigRow { label: "confirm quit", choices: strs(&["off", "on"]) },
        ConfigRow { label: "mcp on startup", choices: strs(&["off", "on"]) },
    ]
}

/// Choice index of the current value per row.
fn current_cursors(state: &State) -> [usize; ROWS] {
    let s = &state.settings;
    let onoff = |b: bool| usize::from(b);
    [
        ThemeId::ALL.iter().position(|t| *t == s.theme).unwrap_or(0),
        DiffMode::ALL.iter().position(|m| *m == s.mode).unwrap_or(0),
        onoff(s.split),
        onoff(!s.full),
        onoff(s.confirm_quit),
        onoff(s.mcp_autostart),
    ]
}

/// `C` in cursor context. True when consumed.
pub fn on_normal_key(state: &mut State, key: KeyEvent, _fx: &mut Fx) -> bool {
    if key.mods.ctrl || key.key != Key::Char('C') {
        return false;
    }
    state.overlay = Overlay::Config(ConfigModal {
        row: 0,
        cursors: current_cursors(state),
        committed_theme: state.settings.theme,
    });
    true
}

/// Key while `Overlay::Config`.
pub fn on_key(state: &mut State, key: KeyEvent, fx: &mut Fx) {
    if key.mods.ctrl {
        return;
    }
    let Overlay::Config(m) = &mut state.overlay else {
        return;
    };
    match key.key {
        Key::Esc | Key::Char('q') | Key::Char('C') => {
            state.settings.theme = m.committed_theme;
            state.overlay = Overlay::None;
        }
        Key::Char('j') | Key::Down => m.row = (m.row + 1).min(ROWS - 1),
        Key::Char('k') | Key::Up => m.row = m.row.saturating_sub(1),
        Key::Char('h') | Key::Left => move_choice(state, -1),
        Key::Char('l') | Key::Right => move_choice(state, 1),
        Key::Enter | Key::Char(' ') => select(state, fx),
        _ => {}
    }
}

/// `h`/`l`: move the choice cursor; the theme row previews live.
fn move_choice(state: &mut State, d: isize) {
    let Overlay::Config(m) = &mut state.overlay else {
        return;
    };
    let max = match m.row {
        0 => ThemeId::ALL.len(),
        1 => DiffMode::ALL.len(),
        _ => 2,
    } - 1;
    let next = m.cursors[m.row].saturating_add_signed(d).min(max);
    m.cursors[m.row] = next;
    if m.row == 0 {
        state.settings.theme = ThemeId::ALL[next];
    }
}

/// Enter/Space: apply the choice under the cursor and save it.
fn select(state: &mut State, fx: &mut Fx) {
    let Overlay::Config(m) = &state.overlay else {
        return;
    };
    let c = m.cursors[m.row];
    let change = match m.row {
        0 => ConfigChange::Theme(ThemeId::ALL[c.min(ThemeId::ALL.len() - 1)]),
        1 => ConfigChange::Mode(DiffMode::ALL[c.min(DiffMode::ALL.len() - 1)]),
        2 => ConfigChange::Split(c == 1),
        3 => ConfigChange::Full(c == 0),
        4 => ConfigChange::ConfirmQuit(c == 1),
        _ => ConfigChange::McpAutostart(c == 1),
    };
    reload::apply_setting_change(state, change.clone(), fx);
    let req = state.alloc_req();
    state.pending.insert(req, Pending::ConfigSave);
    fx.push(Effect::SaveConfig { req, path: state.env.config_path.clone(), change });
}

/// `Event::ConfigSaved`: clears the note on success, error notes per F-CFGUI-03 / `ConfigSaveError`.
pub fn on_saved(state: &mut State, req: ReqId, result: Result<(), ConfigSaveError>) {
    if state.pending.remove(&req).is_none() {
        return;
    }
    let path = state.env.config_path.clone();
    state.note = match result {
        Ok(()) => None,
        Err(ConfigSaveError::Unreadable) => Some(format!("config unreadable, not saved ({path})")),
        Err(ConfigSaveError::Io(r)) => Some(save_error_note(&path, r)),
    };
}

/// Kept for symmetry of io reasons in notes.
pub fn save_error_note(path: &str, reason: IoReason) -> String {
    fail_msg(&format!("config save failed: {path}"), reason)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::testutil::{fake_state, file, k};

    fn open(s: &mut State) {
        assert!(on_normal_key(s, k('C'), &mut Vec::new()));
    }
    fn press(s: &mut State, key: KeyEvent) -> Fx {
        let mut fx = Vec::new();
        on_key(s, key, &mut fx);
        fx
    }
    fn modal(s: &State) -> ConfigModal {
        match &s.overlay {
            Overlay::Config(m) => m.clone(),
            o => panic!("{o:?}"),
        }
    }

    #[test]
    fn f_cfgui_01_rows_and_choices() {
        let s = fake_state();
        let r = rows(&s);
        let labels: Vec<_> = r.iter().map(|x| x.label).collect();
        assert_eq!(labels, ["theme", "mode", "split", "view", "confirm quit", "mcp on startup"]);
        assert_eq!(r[0].choices, ["solarized", "vibrant", "dull", "contrast", "colorblind", "light"]);
        assert_eq!(r[1].choices, ["all", "staged", "unstaged"]);
        assert_eq!(r[2].choices, ["off", "on"]);
        assert_eq!(r[3].choices, ["full", "changes"]);
    }

    #[test]
    fn f_cfgui_01_open_cursors_on_current_values() {
        let mut s = fake_state();
        s.settings.theme = ThemeId::Dull;
        s.settings.mode = DiffMode::Staged;
        s.settings.split = true;
        s.settings.full = false;
        s.settings.confirm_quit = true;
        s.settings.mcp_autostart = false;
        open(&mut s);
        let m = modal(&s);
        assert_eq!(m.cursors, [2, 1, 1, 1, 1, 0]);
        assert_eq!(m.row, 0);
        assert_eq!(m.committed_theme, ThemeId::Dull);
    }

    #[test]
    fn f_cfgui_02_theme_preview_and_revert() {
        let mut s = fake_state();
        open(&mut s);
        press(&mut s, k('l'));
        assert_eq!(s.settings.theme, ThemeId::Vibrant);
        press(&mut s, k('l'));
        assert_eq!(s.settings.theme, ThemeId::Dull);
        press(&mut s, KeyEvent::plain(Key::Esc));
        assert_eq!(s.settings.theme, ThemeId::Solarized);
        assert_eq!(s.overlay, Overlay::None);
    }

    #[test]
    fn f_cfgui_02_select_theme_commits_and_saves() {
        let mut s = fake_state();
        open(&mut s);
        press(&mut s, k('l'));
        let fx = press(&mut s, KeyEvent::plain(Key::Enter));
        assert_eq!(modal(&s).committed_theme, ThemeId::Vibrant);
        assert!(matches!(
            fx.as_slice(),
            [Effect::SaveConfig { path, change: ConfigChange::Theme(ThemeId::Vibrant), .. }] if path == "/cfg/config.json"
        ));
        press(&mut s, k('q'));
        assert_eq!(s.settings.theme, ThemeId::Vibrant);
    }

    #[test]
    fn f_cfgui_02_choice_and_row_clamped() {
        let mut s = fake_state();
        open(&mut s);
        for _ in 0..10 {
            press(&mut s, KeyEvent::plain(Key::Right));
        }
        assert_eq!(modal(&s).cursors[0], 5);
        for _ in 0..10 {
            press(&mut s, k('h'));
        }
        assert_eq!(modal(&s).cursors[0], 0);
        for _ in 0..10 {
            press(&mut s, k('j'));
        }
        assert_eq!(modal(&s).row, 5);
        for _ in 0..10 {
            press(&mut s, KeyEvent::plain(Key::Up));
        }
        assert_eq!(modal(&s).row, 0);
    }

    #[test]
    fn f_cfgui_02_confirm_quit_and_autostart_preference_only() {
        let mut s = fake_state();
        open(&mut s);
        for _ in 0..4 {
            press(&mut s, k('j'));
        }
        press(&mut s, k('h'));
        let fx = press(&mut s, k(' '));
        assert!(!s.settings.confirm_quit);
        assert!(matches!(
            fx.as_slice(),
            [Effect::SaveConfig { change: ConfigChange::ConfirmQuit(false), .. }]
        ));
        press(&mut s, k('j'));
        press(&mut s, k('l'));
        let fx = press(&mut s, KeyEvent::plain(Key::Enter));
        assert!(s.settings.mcp_autostart);
        assert!(matches!(
            fx.as_slice(),
            [Effect::SaveConfig { change: ConfigChange::McpAutostart(true), .. }]
        ));
        assert!(matches!(s.overlay, Overlay::Config(_)));
        assert!(s.mcp.starting.is_none());
    }

    #[test]
    fn f_cfgui_02_mode_select_reloads_when_different() {
        let mut s = fake_state();
        s.files = vec![file("a.txt", 3)];
        open(&mut s);
        press(&mut s, k('j'));
        press(&mut s, k('l'));
        let fx = press(&mut s, KeyEvent::plain(Key::Enter));
        assert_eq!(s.settings.mode, DiffMode::Staged);
        assert!(matches!(fx[0], Effect::LoadDiff { .. }));
        assert!(matches!(fx[1], Effect::SaveConfig { change: ConfigChange::Mode(DiffMode::Staged), .. }));
    }

    #[test]
    fn f_cfgui_02_view_select_reloads_when_different() {
        let mut s = fake_state();
        s.files = vec![file("a.txt", 3)];
        open(&mut s);
        for _ in 0..3 {
            press(&mut s, k('j'));
        }
        press(&mut s, k('l'));
        let fx = press(&mut s, KeyEvent::plain(Key::Enter));
        assert!(!s.settings.full);
        assert!(matches!(fx[0], Effect::LoadDiff { spec: crate::diff::DiffSpec { full: false, .. }, .. }));
    }

    #[test]
    fn f_cfgui_03_saved_notes() {
        let mut s = fake_state();
        s.note = Some("x".into());
        s.pending.insert(ReqId(5), Pending::ConfigSave);
        on_saved(&mut s, ReqId(5), Ok(()));
        assert_eq!(s.note, None);
        s.pending.insert(ReqId(6), Pending::ConfigSave);
        on_saved(&mut s, ReqId(6), Err(ConfigSaveError::Unreadable));
        assert_eq!(s.note.as_deref(), Some("config unreadable, not saved (/cfg/config.json)"));
        s.pending.insert(ReqId(7), Pending::ConfigSave);
        on_saved(&mut s, ReqId(7), Err(ConfigSaveError::Io(IoReason::PermissionDenied)));
        assert_eq!(s.note.as_deref(), Some("config save failed: /cfg/config.json: permission denied"));
    }

    #[test]
    fn f_cfgui_03_stale_saved_ignored() {
        let mut s = fake_state();
        s.note = Some("keep".into());
        on_saved(&mut s, ReqId(9), Ok(()));
        assert_eq!(s.note.as_deref(), Some("keep"));
    }
}
