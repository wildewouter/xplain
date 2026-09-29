//! Diff loading lifecycle and view toggles: load results, error/no-changes screens, mode/scope/split/theme
//! keys, reload, agent-triggered reload.
//!
//! Spec: F-MODE-01..05 (`m` cycle, git error screen, no changes), F-SCOPE-01/02 (`c`), F-LAYOUT-04 (`s` incl.
//! F-NAV-10 top reset), F-THEME-01 (`t`), F-RELOAD-01..03 (`r`, agent reload, cursor kept), F-NAV-10.
//! Oracle: `reload`, `cycleMode`, scope/theme code in `src/app.tsx`, `src/diff/load.ts`.
//! Owner: component `shell` (G). Uses `diff::parse_raw`, `jump::{remember,restore,open_file}`, `rows::ensure`.
//! Must not: render.
//!
//! Only the latest diff request counts (`State::diff_req`); older results are dropped. A mode/scope change
//! parks its cursor rule in `State::diff_nav` until the load it caused lands.

use crate::config::ConfigChange;
use crate::diff::{DiffSpec, RawDiff};
use crate::effect::{Effect, Fx};
use crate::event::ReqId;
use crate::jump;
use crate::keys::{Key, KeyEvent};
use crate::nav;
use crate::nav::viewport;
use crate::options::DiffMode;
use crate::rows;
use crate::state::{DiffLoadKind, DiffNav, LoadState, Overlay, Pending, State};
use crate::theme::ThemeId;

const BINARY_MSG: &str = "binary file, not shown";

/// Emit `Effect::LoadDiff` for the current settings (allocates req, `Pending::Diff`).
pub fn request_load(state: &mut State, fx: &mut Fx) {
    let req = state.alloc_req();
    state.pending.insert(req, Pending::Diff);
    state.diff_req = Some(req);
    let spec = DiffSpec {
        cwd: state.options.cwd.clone(),
        mode: state.settings.mode,
        full: state.settings.full,
        git_args: state.options.git_args.clone(),
    };
    fx.push(Effect::LoadDiff { req, spec });
}

fn request_kind(state: &mut State, kind: DiffLoadKind, fx: &mut Fx) {
    state.diff_kind = kind;
    request_load(state, fx);
}

/// `s c m t r` in cursor context (and browse where the spec says ignore). True when consumed.
pub fn on_key(state: &mut State, key: KeyEvent, fx: &mut Fx) -> bool {
    if key.mods.ctrl {
        return false;
    }
    let Key::Char(c) = key.key else {
        return false;
    };
    let browse = state.browse.is_some();
    match c {
        's' | 'c' | 'm' => {
            if !browse {
                match c {
                    's' => toggle_split(state, !state.settings.split),
                    'c' => change_scope(state, !state.settings.full, fx),
                    _ => change_mode(state, state.settings.mode.next(), fx),
                }
            }
            true
        }
        't' => {
            state.settings.theme = state.settings.theme.next();
            true
        }
        'r' => {
            reload_now(state, fx);
            true
        }
        _ => false,
    }
}

/// `r`: note, diff reload, browse re-read.
fn reload_now(state: &mut State, fx: &mut Fx) {
    request_kind(state, DiffLoadKind::Manual, fx);
    reread_browse(state, fx);
    state.note = Some("reloaded".to_string());
}

fn reread_browse(state: &mut State, fx: &mut Fx) {
    if let Some(b) = &state.browse {
        let path = b.path.clone();
        let full = match &state.options.cwd {
            Some(c) => format!("{}/{path}", c.trim_end_matches('/')),
            None => path.clone(),
        };
        let req = state.alloc_req();
        state.pending.insert(req, Pending::BrowseReload { path });
        fx.push(Effect::ReadFile { req, path: full });
    }
}

/// `Event::FileRead` for a `Pending::BrowseReload` request. True when the request was ours.
pub fn on_browse_reread(
    state: &mut State,
    req: ReqId,
    result: Result<Vec<u8>, crate::errors::IoReason>,
) -> bool {
    let Some(Pending::BrowseReload { path }) = state.pending.get(&req).cloned() else {
        return false;
    };
    state.pending.remove(&req);
    let Ok(bytes) = result else {
        return true;
    };
    let Some(b) = state.browse.as_mut() else {
        return true;
    };
    if b.path != path {
        return true;
    }
    let lines = decode_lines(&bytes);
    if lines != b.lines {
        b.lines = lines;
        state.files_gen += 1;
        rows::ensure(state);
        nav::clamp_cursor(state);
    }
    true
}

fn decode_lines(bytes: &[u8]) -> Vec<String> {
    let head = &bytes[..bytes.len().min(8000)];
    let text =
        if head.contains(&0) { BINARY_MSG.to_string() } else { String::from_utf8_lossy(bytes).into_owned() };
    let text = text.strip_suffix('\n').unwrap_or(&text);
    text.split('\n').map(str::to_string).collect()
}

/// New mode: top reset + follow now (old rows); file index 0 and cursor rule when the diff arrives (F-MODE-03).
fn change_mode(state: &mut State, mode: DiffMode, fx: &mut Fx) {
    state.settings.mode = mode;
    viewport::reset_top_keep_cursor(state);
    let memo = jump::remember(state);
    state.diff_nav = Some(DiffNav::Mode(memo));
    request_kind(state, DiffLoadKind::Effect, fx);
}

/// Scope change: stay on the same path, cursor + viewport reset per F-NAV-08 now and on arrival (F-SCOPE-02).
fn change_scope(state: &mut State, full: bool, fx: &mut Fx) {
    let keep = state.files.get(state.nav.file_index).map(|f| f.path.clone());
    state.settings.full = full;
    state.diff_nav = Some(DiffNav::Scope(keep));
    if state.browse.is_none() {
        reset_cursor(state);
    }
    request_kind(state, DiffLoadKind::Effect, fx);
}

fn reset_cursor(state: &mut State) {
    state.nav.selection = None;
    state.nav.count = 0;
    rows::ensure(state);
    jump::initial_position(state);
}

/// `s`: toggle split. Effective layout changed -> F-NAV-08 placement, else top reset keeping the cursor (F-NAV-10).
fn toggle_split(state: &mut State, split: bool) {
    let before = rows::effective_split(state);
    state.settings.split = split;
    let after = rows::effective_split(state);
    if before != after && state.browse.is_none() {
        reset_cursor(state);
    } else {
        viewport::reset_top_keep_cursor(state);
    }
}

/// `Event::DiffLoaded`: stale dropped; Ok -> parse, replace files, bump `files_gen`, cursor per F-RELOAD-03 or
/// F-NAV-08 (first load: set `ready`), no-changes state; Err -> `LoadState::Error`.
pub fn on_diff_loaded(state: &mut State, req: ReqId, result: Result<RawDiff, String>, _fx: &mut Fx) {
    if state.pending.remove(&req).is_none() || state.diff_req != Some(req) {
        return;
    }
    state.diff_req = None;
    let action = state.diff_nav.take();
    let kind = if action.is_some() || !state.ready { DiffLoadKind::Effect } else { state.diff_kind };
    state.diff_kind = DiffLoadKind::Effect;
    match result {
        Err(msg) => {
            match kind {
                DiffLoadKind::Effect => state.load = LoadState::Error(msg),
                DiffLoadKind::Manual => {
                    state.note = Some(msg.replace('\n', " ").trim().to_string());
                }
                DiffLoadKind::Agent => {}
            }
            state.ready = true;
        }
        Ok(raw) => {
            let files = crate::diff::parse_raw(&raw);
            apply_files(state, files, action);
            state.load = LoadState::Ready;
            state.ready = true;
        }
    }
}

fn apply_files(state: &mut State, files: Vec<crate::diff::FileDiff>, action: Option<DiffNav>) {
    let was_ready = state.ready && state.load == LoadState::Ready;
    let unchanged = files == state.files;
    if unchanged && action.is_none() && was_ready {
        return;
    }
    let shown = state.files.get(state.nav.file_index).map(|f| f.path.clone());
    let memo = match &action {
        Some(DiffNav::Mode(m)) => m.clone(),
        _ => jump::remember(state),
    };
    let browsing = state.browse.is_some();
    state.files = files;
    state.files_gen += 1;
    state.nav.selection = None;
    let find = |p: &str, files: &[crate::diff::FileDiff]| files.iter().position(|f| f.path == p);
    match action {
        Some(DiffNav::Mode(_)) => {
            state.nav.file_index = 0;
            rows::ensure(state);
            let same = memo.path.as_deref().is_some_and(|p| state.files.first().is_some_and(|f| f.path == p));
            if browsing {
                nav::clamp_cursor(state);
            } else if same {
                jump::restore(state, &memo);
                state.nav.file_index = 0;
            } else {
                jump::initial_position(state);
            }
        }
        Some(DiffNav::Scope(keep)) => {
            state.nav.file_index = keep.as_deref().and_then(|p| find(p, &state.files)).unwrap_or(0);
            rows::ensure(state);
            if browsing {
                nav::clamp_cursor(state);
            } else {
                jump::initial_position(state);
            }
        }
        None => {
            let idx = shown.as_deref().and_then(|p| find(p, &state.files));
            state.nav.file_index = idx.unwrap_or(0);
            rows::ensure(state);
            if browsing {
                nav::clamp_cursor(state);
            } else if idx.is_some() {
                jump::restore(state, &memo);
            } else {
                jump::initial_position(state);
            }
        }
    }
}

/// Agent `files_changed` (F-RELOAD-02): reload silently, keeping cursor.
pub fn on_files_changed(state: &mut State, _paths: &[String], fx: &mut Fx) {
    request_kind(state, DiffLoadKind::Agent, fx);
    reread_browse(state, fx);
}

/// Apply a setting change made from the config modal that affects the view (mode/split/full/theme),
/// including reloads and top reset rules (F-CFGUI-02).
pub fn apply_setting_change(state: &mut State, change: ConfigChange, fx: &mut Fx) {
    match change {
        ConfigChange::Theme(t) => set_theme(state, t),
        ConfigChange::Mode(m) => {
            if m != state.settings.mode {
                change_mode(state, m, fx);
            } else if !state.files.is_empty() {
                jump::open_file(state, 0);
            }
        }
        ConfigChange::Split(v) => toggle_split(state, v),
        ConfigChange::Full(v) => {
            if v != state.settings.full {
                change_scope(state, v, fx);
            } else {
                viewport::reset_top_keep_cursor(state);
            }
        }
        ConfigChange::ConfirmQuit(v) => state.settings.confirm_quit = v,
        ConfigChange::McpAutostart(v) => state.settings.mcp_autostart = v,
    }
}

fn set_theme(state: &mut State, t: ThemeId) {
    state.settings.theme = t;
    if let Overlay::Config(c) = &mut state.overlay {
        c.committed_theme = t;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::options::Options;
    use crate::state::Browse;
    use crate::state::testutil::{fake_state, fake_state_with, file, k};

    const DIFF: &str = "diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@ -1,2 +1,2 @@\n x\n-y\n+z\n";

    fn raw() -> RawDiff {
        RawDiff { tracked: DIFF.to_string(), untracked: vec![] }
    }

    fn load_req(fx: &Fx) -> ReqId {
        match fx.last() {
            Some(Effect::LoadDiff { req, .. }) => *req,
            e => panic!("{e:?}"),
        }
    }

    fn key(s: &mut State, c: char) -> Fx {
        let mut fx = Vec::new();
        assert!(on_key(s, k(c), &mut fx));
        fx
    }

    #[test]
    fn f_cli_03_new_applies_flags_and_loads() {
        let opts = Options {
            mode: Some(DiffMode::Staged),
            split: Some(true),
            full: Some(false),
            theme: Some(ThemeId::Light),
            git_args: vec!["main".into()],
            cwd: Some("/x".into()),
            ..Options::default()
        };
        let init_cfg = Config::default();
        let s = fake_state_with(opts, init_cfg);
        assert_eq!(s.settings.mode, DiffMode::Staged);
        assert!(s.settings.split);
        assert!(!s.settings.full);
        assert_eq!(s.settings.theme, ThemeId::Light);
        assert_eq!(s.load, LoadState::Loading);
        assert!(!s.is_ready());
        assert_eq!(s.integration_state.len(), 2);
        assert_eq!(s.pending.len(), 1);
    }

    #[test]
    fn f_mode_01_initial_effect() {
        let opts = Options { git_args: vec!["main".into()], cwd: Some("/x".into()), ..Options::default() };
        let init = crate::options::Init {
            options: opts,
            config: Config::default(),
            env: Default::default(),
            size: crate::screen::Size { cols: 80, rows: 24 },
        };
        let (s, fx) = State::new(init, vec![]);
        assert_eq!(
            fx,
            vec![Effect::LoadDiff {
                req: ReqId(1),
                spec: DiffSpec {
                    cwd: Some("/x".into()),
                    mode: DiffMode::All,
                    full: true,
                    git_args: vec!["main".into()]
                }
            }]
        );
        assert_eq!(s.pending.get(&ReqId(1)), Some(&Pending::Diff));
    }

    #[test]
    fn f_mode_03_m_cycles_mode_and_reloads() {
        let mut s = fake_state();
        s.files = vec![file("a.txt", 3)];
        let mut seen = vec![];
        for _ in 0..3 {
            let fx = key(&mut s, 'm');
            let Effect::LoadDiff { spec, .. } = &fx[0] else { panic!() };
            seen.push(spec.mode);
            assert_eq!(s.settings.mode, spec.mode);
        }
        assert_eq!(seen, [DiffMode::Staged, DiffMode::Unstaged, DiffMode::All]);
    }

    #[test]
    fn f_scope_02_c_toggles_scope() {
        let mut s = fake_state();
        s.files = vec![file("a.txt", 3)];
        let fx = key(&mut s, 'c');
        assert!(!s.settings.full);
        assert!(matches!(&fx[0], Effect::LoadDiff { spec, .. } if !spec.full));
        assert_eq!(s.diff_nav, Some(DiffNav::Scope(Some("a.txt".into()))));
    }

    #[test]
    fn f_layout_04_s_toggles_split_setting() {
        let mut s = fake_state();
        s.files = vec![file("a.txt", 3)];
        let fx = key(&mut s, 's');
        assert!(s.settings.split);
        assert!(fx.is_empty());
        key(&mut s, 's');
        assert!(!s.settings.split);
    }

    #[test]
    fn f_theme_01_t_cycles_and_wraps() {
        let mut s = fake_state();
        let mut seen = vec![];
        for _ in 0..6 {
            key(&mut s, 't');
            seen.push(s.settings.theme);
        }
        assert_eq!(
            seen,
            [
                ThemeId::Vibrant,
                ThemeId::Dull,
                ThemeId::Contrast,
                ThemeId::Colorblind,
                ThemeId::Light,
                ThemeId::Solarized
            ]
        );
    }

    #[test]
    fn f_mode_03_smc_ignored_in_browse_t_works() {
        let mut s = fake_state();
        s.browse = Some(Browse { path: "a.txt".into(), lines: vec!["x".into()] });
        for c in ['s', 'c', 'm'] {
            let fx = key(&mut s, c);
            assert!(fx.is_empty());
        }
        assert_eq!(s.settings, fake_state().settings);
        key(&mut s, 't');
        assert_eq!(s.settings.theme, ThemeId::Vibrant);
    }

    #[test]
    fn keys_not_ours_pass_and_ctrl_ignored() {
        let mut s = fake_state();
        assert!(!on_key(&mut s, k('x'), &mut Vec::new()));
        assert!(!on_key(&mut s, KeyEvent::ctrl('r'), &mut Vec::new()));
        assert!(!on_key(&mut s, KeyEvent::plain(Key::Tab), &mut Vec::new()));
    }

    #[test]
    fn f_reload_01_r_notes_and_reloads() {
        let mut s = fake_state();
        let fx = key(&mut s, 'r');
        assert_eq!(s.note.as_deref(), Some("reloaded"));
        assert!(matches!(fx.as_slice(), [Effect::LoadDiff { .. }]));
        assert_eq!(s.diff_kind, DiffLoadKind::Manual);
    }

    #[test]
    fn f_reload_01_r_rereads_browse_file() {
        let mut s =
            fake_state_with(Options { cwd: Some("/w/".into()), ..Options::default() }, Config::default());
        s.browse = Some(Browse { path: "a.txt".into(), lines: vec!["old".into()] });
        let fx = key(&mut s, 'r');
        assert_eq!(fx.len(), 2);
        let Effect::ReadFile { req, path } = &fx[1] else { panic!() };
        assert_eq!(path, "/w/a.txt");
        // read error ignored, old text kept
        assert!(on_browse_reread(&mut s, *req, Err(crate::errors::IoReason::NotFound)));
        assert_eq!(s.browse.as_ref().map(|b| b.lines.clone()), Some(vec!["old".to_string()]));
        assert_eq!(s.note.as_deref(), Some("reloaded"));
    }

    #[test]
    fn f_reload_01_browse_reread_replaces_text() {
        let mut s = fake_state();
        s.browse = Some(Browse { path: "a.txt".into(), lines: vec!["old".into()] });
        let fx = key(&mut s, 'r');
        let Effect::ReadFile { req, .. } = &fx[1] else { panic!() };
        let gen0 = s.files_gen;
        on_browse_reread(&mut s, *req, Ok(b"new\nlines\n".to_vec()));
        assert_eq!(
            s.browse.as_ref().map(|b| b.lines.clone()),
            Some(vec!["new".to_string(), "lines".to_string()])
        );
        assert_eq!(s.files_gen, gen0 + 1);
        assert!(!on_browse_reread(&mut s, ReqId(999), Ok(vec![])));
    }

    #[test]
    fn decode_binary_and_trailing_newline() {
        assert_eq!(decode_lines(b"a\0b"), vec!["binary file, not shown".to_string()]);
        assert_eq!(decode_lines(b"a\n\n"), vec!["a".to_string(), String::new()]);
        assert_eq!(decode_lines(b""), vec![String::new()]);
    }

    #[test]
    fn f_mode_05_first_load_ok_sets_ready_and_files() {
        let (mut s, fx) = {
            let s = fake_state();
            (
                s,
                vec![Effect::LoadDiff {
                    req: ReqId(1),
                    spec: DiffSpec { cwd: None, mode: DiffMode::All, full: true, git_args: vec![] },
                }],
            )
        };
        let req = load_req(&fx);
        on_diff_loaded(&mut s, req, Ok(raw()), &mut Vec::new());
        assert!(s.is_ready());
        assert_eq!(s.load, LoadState::Ready);
        assert_eq!(s.files.len(), 1);
        assert_eq!(s.files[0].path, "a.txt");
        assert!(s.pending.is_empty());
    }

    #[test]
    fn f_mode_05_no_changes_is_ready_with_no_files() {
        let mut s = fake_state();
        on_diff_loaded(&mut s, ReqId(1), Ok(RawDiff::default()), &mut Vec::new());
        assert!(s.is_ready());
        assert_eq!(s.load, LoadState::Ready);
        assert!(s.files.is_empty());
    }

    #[test]
    fn f_mode_04_first_load_error_screen() {
        let mut s = fake_state();
        on_diff_loaded(&mut s, ReqId(1), Err("fatal: nope\n".into()), &mut Vec::new());
        assert_eq!(s.load, LoadState::Error("fatal: nope\n".into()));
        assert!(s.is_ready());
    }

    #[test]
    fn f_reload_01_stale_result_dropped() {
        let mut s = fake_state();
        let mut fx = Vec::new();
        request_load(&mut s, &mut fx);
        // result of the first (superseded) request
        on_diff_loaded(&mut s, ReqId(1), Ok(raw()), &mut Vec::new());
        assert_eq!(s.load, LoadState::Loading);
        assert!(!s.is_ready());
        assert!(s.files.is_empty());
        // unknown id
        on_diff_loaded(&mut s, ReqId(42), Ok(raw()), &mut Vec::new());
        assert_eq!(s.load, LoadState::Loading);
        on_diff_loaded(&mut s, ReqId(2), Ok(raw()), &mut Vec::new());
        assert_eq!(s.load, LoadState::Ready);
    }

    #[test]
    fn f_reload_01_manual_error_becomes_single_line_note() {
        let mut s = fake_state();
        on_diff_loaded(&mut s, ReqId(1), Ok(raw()), &mut Vec::new());
        let fx = key(&mut s, 'r');
        let req = load_req(&fx);
        on_diff_loaded(&mut s, req, Err("  fatal: a\nline two\n".into()), &mut Vec::new());
        assert_eq!(s.note.as_deref(), Some("fatal: a line two"));
        assert_eq!(s.load, LoadState::Ready);
        assert_eq!(s.files.len(), 1);
    }

    #[test]
    fn f_reload_01_manual_error_keeps_error_screen() {
        let mut s = fake_state();
        on_diff_loaded(&mut s, ReqId(1), Err("boom".into()), &mut Vec::new());
        let fx = key(&mut s, 'r');
        on_diff_loaded(&mut s, load_req(&fx), Err("still boom".into()), &mut Vec::new());
        assert_eq!(s.load, LoadState::Error("boom".into()));
    }

    #[test]
    fn f_reload_01_success_restores_from_error_screen() {
        let mut s = fake_state();
        on_diff_loaded(&mut s, ReqId(1), Err("boom".into()), &mut Vec::new());
        let fx = key(&mut s, 'r');
        on_diff_loaded(&mut s, load_req(&fx), Ok(raw()), &mut Vec::new());
        assert_eq!(s.load, LoadState::Ready);
        assert_eq!(s.files.len(), 1);
    }

    #[test]
    fn f_reload_02_agent_reload_silent() {
        let mut s = fake_state();
        on_diff_loaded(&mut s, ReqId(1), Ok(raw()), &mut Vec::new());
        s.note = None;
        let mut fx = Vec::new();
        on_files_changed(&mut s, &["a.txt".into()], &mut fx);
        assert!(s.note.is_none());
        assert!(matches!(fx.as_slice(), [Effect::LoadDiff { .. }]));
        on_diff_loaded(&mut s, load_req(&fx), Err("x".into()), &mut Vec::new());
        assert!(s.note.is_none());
        assert_eq!(s.load, LoadState::Ready);
    }

    #[test]
    fn f_reload_01_identical_result_changes_nothing() {
        let mut s = fake_state();
        on_diff_loaded(&mut s, ReqId(1), Ok(raw()), &mut Vec::new());
        let gen0 = s.files_gen;
        let fx = key(&mut s, 'r');
        on_diff_loaded(&mut s, load_req(&fx), Ok(raw()), &mut Vec::new());
        assert_eq!(s.files_gen, gen0);
    }

    #[test]
    fn f_mode_03_mode_load_goes_to_first_file() {
        let mut s = fake_state();
        on_diff_loaded(&mut s, ReqId(1), Ok(raw()), &mut Vec::new());
        let fx = key(&mut s, 'm');
        assert!(matches!(s.diff_nav, Some(DiffNav::Mode(_))));
        on_diff_loaded(&mut s, load_req(&fx), Ok(RawDiff::default()), &mut Vec::new());
        assert!(s.files.is_empty());
        assert_eq!(s.nav.file_index, 0);
        assert!(s.diff_nav.is_none());
    }

    #[test]
    fn f_mode_04_mode_load_error_replaces_screen() {
        let mut s = fake_state();
        on_diff_loaded(&mut s, ReqId(1), Ok(raw()), &mut Vec::new());
        let fx = key(&mut s, 'm');
        on_diff_loaded(&mut s, load_req(&fx), Err("bad rev".into()), &mut Vec::new());
        assert_eq!(s.load, LoadState::Error("bad rev".into()));
        assert!(s.diff_nav.is_none());
    }

    #[test]
    fn f_scope_02_scope_load_keeps_path() {
        let mut s = fake_state();
        let two =
            format!("{DIFF}diff --git a/b.txt b/b.txt\n--- a/b.txt\n+++ b/b.txt\n@@ -1 +1 @@\n-p\n+q\n");
        on_diff_loaded(
            &mut s,
            ReqId(1),
            Ok(RawDiff { tracked: two.clone(), untracked: vec![] }),
            &mut Vec::new(),
        );
        s.nav.file_index = 1;
        let fx = key(&mut s, 'c');
        on_diff_loaded(
            &mut s,
            load_req(&fx),
            Ok(RawDiff { tracked: two, untracked: vec![] }),
            &mut Vec::new(),
        );
        assert_eq!(s.nav.file_index, 1);
        assert_eq!(s.files[1].path, "b.txt");
    }

    #[test]
    fn f_cfgui_02_apply_setting_changes() {
        let mut s = fake_state();
        s.files = vec![file("a.txt", 3)];
        let mut fx = Vec::new();
        apply_setting_change(&mut s, ConfigChange::ConfirmQuit(false), &mut fx);
        apply_setting_change(&mut s, ConfigChange::McpAutostart(true), &mut fx);
        apply_setting_change(&mut s, ConfigChange::Theme(ThemeId::Dull), &mut fx);
        assert!(fx.is_empty());
        assert!(!s.settings.confirm_quit && s.settings.mcp_autostart);
        assert_eq!(s.settings.theme, ThemeId::Dull);
        apply_setting_change(&mut s, ConfigChange::Full(false), &mut fx);
        assert!(matches!(fx.as_slice(), [Effect::LoadDiff { .. }]));
    }
}
