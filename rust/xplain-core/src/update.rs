//! The reducer entry point and key/event routing.
//!
//! Spec: everything interactive; F-CLI-05 (Ctrl+C), F-HELP-02 (context priority), F-NAV-07.
//! Owner: component `shell` (G). Routing only: every behavior lives in the handler modules named below.
//! Must not: perform IO, read the clock, block, or contain feature logic beyond dispatch.
//!
//! Key routing order (first handler that consumes wins; mirrors the `useInput` order in `src/app.tsx`):
//! 1. Ctrl+C -> `Effect::Exit{0}` in every state (after `mcp_ui`-style stop is NOT needed: exit is immediate).
//! 2. `help::on_key` (panel open / `?`).
//! 3. Overlay by kind: Editor -> `editor::on_key`; Find -> `find::on_find_key`; Goto -> `find::on_goto_key`;
//!    DeleteComment -> `thread::on_delete_dialog_key`; Quit -> `quit::on_key`; Search -> `search::on_key`;
//!    Mcp -> `mcp_ui::on_key`; Config -> `config_ui::on_key`; Picker -> `picker::on_key`.
//! 4. Focused comment -> `thread::on_focused_key`.
//! 5. Browse-only keys -> `browse::on_key`.
//! 6. Normal keys, in order: `quit::on_normal_key`, `reload::on_key`, `jump::on_key`, `find::on_normal_key`,
//!    `picker::on_normal_key`, `search::on_normal_key`, `editor::on_cursor_key`, `thread::on_cursor_key`,
//!    `export::on_key`, `config_ui::on_normal_key`, `mcp_ui::on_normal_key`, then `nav::on_key`.
//!
//! Non-key events: `DiffLoaded` -> `reload::on_diff_loaded`; `FilesListed` -> `search::on_files_listed`;
//! `FileRead` -> `browse::on_file_read`; `ConfigSaved` -> `config_ui::on_saved`; `ExportWritten` ->
//! `export::on_written`; `CommandDone`/`McpStarted`/`McpStopped`/`McpHttp`/`McpConnClosed`/`Started` ->
//! `mcp_ui::*`; `Timer(Spinner)` -> `ask::on_spinner`; `Timer(PollTimeout)` -> `mcp_ui::on_timer`; `Paste` ->
//! `editor/find/search::on_paste`; `Resize` -> set size, `rows::ensure`, `nav::clamp_cursor`, `viewport::follow`.
//! After every event: `rows::ensure`, `thread::sync`, `hlcache::sync` (requests highlights near the viewport).
//! `Highlighted` -> `hlcache::on_highlighted`.
//!
//! `FileRead` first goes to `reload::on_browse_reread` (browse re-read on `r`), which claims its own requests.

use crate::effect::{Effect, Fx};
use crate::event::{Event, TimerId};
use crate::keys::KeyEvent;
use crate::state::{Overlay, State};
use crate::{
    ask, browse, config_ui, editor, export, find, help, hlcache, jump, mcp_ui, nav, picker, quit, reload,
    rows, search, thread,
};

/// Apply `event` to `state` and return the effects to run. Pure and deterministic given
/// `state.clock`. Ctrl+C always yields `Effect::Exit { code: 0 }` (F-CLI-05).
pub fn update(state: &mut State, event: Event) -> Vec<Effect> {
    let mut fx: Fx = Vec::new();
    match event {
        Event::Key(key) => {
            if key.is_ctrl_c() {
                return vec![Effect::Exit { code: 0 }];
            }
            on_key(state, key, &mut fx);
        }
        Event::Started => mcp_ui::on_started(state, &mut fx),
        Event::Paste(text) => {
            let _ = editor::on_paste(state, &text)
                || find::on_paste(state, &text)
                || search::on_paste(state, &text);
        }
        Event::Resize(size) => {
            state.size = size;
            rows::ensure(state);
            nav::clamp_cursor(state);
            nav::viewport::follow(state);
        }
        Event::Timer(TimerId::Spinner) => ask::on_spinner(state, &mut fx),
        Event::Timer(id) => mcp_ui::on_timer(state, id, &mut fx),
        Event::DiffLoaded { req, result } => reload::on_diff_loaded(state, req, result, &mut fx),
        Event::FilesListed { req, files } => search::on_files_listed(state, req, files),
        Event::FileRead { req, result } => {
            if !reload::on_browse_reread(state, req, result.clone()) {
                browse::on_file_read(state, req, result, &mut fx);
            }
        }
        Event::ConfigSaved { req, result } => config_ui::on_saved(state, req, result),
        Event::ExportWritten { req, result } => export::on_written(state, req, result),
        Event::CommandDone { req, result } => mcp_ui::on_command_done(state, req, result, &mut fx),
        Event::McpStarted { req, result } => mcp_ui::on_mcp_started(state, req, result, &mut fx),
        Event::McpStopped { req } => mcp_ui::on_mcp_stopped(state, req, &mut fx),
        Event::McpHttp(req) => mcp_ui::on_http(state, req, &mut fx),
        Event::McpConnClosed(conn) => mcp_ui::on_conn_closed(state, conn, &mut fx),
        Event::Highlighted { key, start, runs, end } => {
            hlcache::on_highlighted(state, &key, start, runs, end)
        }
    }
    rows::ensure(state);
    thread::sync(state);
    hlcache::sync(state, &mut fx);
    fx
}

/// Key routing (see the module doc for the order).
fn on_key(state: &mut State, key: KeyEvent, fx: &mut Fx) {
    if help::on_key(state, key, fx) {
        nav::clear_count(state);
        return;
    }
    match state.overlay {
        Overlay::Editor(_) => return editor::on_key(state, key, fx),
        Overlay::Find { .. } => return find::on_find_key(state, key, fx),
        Overlay::Goto { .. } => return find::on_goto_key(state, key, fx),
        Overlay::DeleteComment { .. } => return thread::on_delete_dialog_key(state, key, fx),
        Overlay::Quit => return quit::on_key(state, key, fx),
        Overlay::Search(_) => return search::on_key(state, key, fx),
        Overlay::Mcp(_) => return mcp_ui::on_key(state, key, fx),
        Overlay::Config(_) => return config_ui::on_key(state, key, fx),
        Overlay::Picker { .. } => return picker::on_key(state, key, fx),
        Overlay::None => {}
    }
    if state.nav.focused_comment.is_some() && thread::on_focused_key(state, key, fx) {
        return;
    }
    if state.browse.is_some() && browse::on_key(state, key, fx) {
        return;
    }
    // Every shell handler that claims a key clears the pending count (F-CURSOR-05).
    let shell = quit::on_normal_key(state, key, fx)
        || reload::on_key(state, key, fx)
        || jump::on_key(state, key, fx)
        || find::on_normal_key(state, key, fx)
        || picker::on_normal_key(state, key, fx)
        || search::on_normal_key(state, key, fx)
        || editor::on_cursor_key(state, key, fx)
        || thread::on_cursor_key(state, key, fx)
        || export::on_key(state, key, fx)
        || config_ui::on_normal_key(state, key, fx)
        || mcp_ui::on_normal_key(state, key, fx);
    if shell {
        nav::clear_count(state);
    } else {
        let _ = nav::on_key(state, key, fx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::RawDiff;
    use crate::event::ReqId;
    use crate::keys::Key;
    use crate::screen::Size;
    use crate::state::testutil::{fake_state, k, running_state};
    use crate::state::{ConfigModal, EditorKind, EditorState, LoadState, McpModal};
    use crate::theme::ThemeId;

    fn key(s: &mut State, c: char) -> Vec<Effect> {
        update(s, Event::Key(k(c)))
    }

    fn overlays() -> Vec<Overlay> {
        vec![
            Overlay::None,
            Overlay::Find { text: "x".into() },
            Overlay::Goto { text: "1".into() },
            Overlay::Editor(EditorState {
                kind: EditorKind::New,
                text: String::new(),
                caret: 0,
                ask_mode: false,
                ext: Default::default(),
            }),
            Overlay::Picker { sel: 0 },
            Overlay::Search(Default::default()),
            Overlay::Config(ConfigModal { row: 0, cursors: [0; 6], committed_theme: ThemeId::Solarized }),
            Overlay::Mcp(McpModal::default()),
            Overlay::DeleteComment { id: "q1".into() },
            Overlay::Quit,
        ]
    }

    #[test]
    fn f_cursor_05_shell_keys_clear_count() {
        for c in ['?', 'r', 'M', 'F', 'f', 'C', 'E', 'n', 'N', 'q'] {
            let mut s = fake_state();
            s.nav.count = 3;
            key(&mut s, c);
            assert_eq!(s.nav.count, 0, "key {c}");
        }
    }

    #[test]
    fn f_cli_05_ctrl_c_exits_everywhere() {
        for o in overlays() {
            let mut s = fake_state();
            s.overlay = o;
            let fx = update(&mut s, Event::Key(KeyEvent::ctrl('c')));
            assert_eq!(fx, vec![Effect::Exit { code: 0 }]);
        }
        let mut s = fake_state();
        s.load = LoadState::Error("x".into());
        assert_eq!(update(&mut s, Event::Key(KeyEvent::ctrl('c'))), vec![Effect::Exit { code: 0 }]);
        let mut s = running_state();
        // immediate: no MCP stop
        assert_eq!(update(&mut s, Event::Key(KeyEvent::ctrl('c'))), vec![Effect::Exit { code: 0 }]);
    }

    #[test]
    fn f_cli_05_q_while_loading_quits_when_confirm_off() {
        let mut s = fake_state();
        s.settings.confirm_quit = false;
        assert_eq!(key(&mut s, 'q'), vec![Effect::Exit { code: 0 }]);
    }

    #[test]
    fn f_quit_01_confirm_then_y() {
        let mut s = fake_state();
        assert!(key(&mut s, 'q').is_empty());
        assert_eq!(s.overlay, Overlay::Quit);
        assert_eq!(key(&mut s, 'y'), vec![Effect::Exit { code: 0 }]);
    }

    #[test]
    fn f_quit_01_running_mcp_stops_before_exit() {
        let mut s = running_state();
        s.settings.confirm_quit = false;
        let fx = key(&mut s, 'q');
        let stop = fx.iter().position(|e| matches!(e, Effect::McpStop { .. }));
        let exit = fx.iter().position(|e| matches!(e, Effect::Exit { .. }));
        assert!(stop.is_some() && exit.is_some() && stop < exit);
        assert_eq!(exit, Some(fx.len() - 1));
        let last_reply = fx.iter().rposition(|e| matches!(e, Effect::HttpReply { .. }));
        assert!(last_reply.is_none_or(|r| Some(r) < stop));
        assert!(!s.mcp.running);
    }

    #[test]
    fn started_event_autostarts_mcp() {
        let mut s = fake_state();
        s.settings.mcp_autostart = true;
        let fx = update(&mut s, Event::Started);
        assert!(matches!(fx.as_slice(), [Effect::McpStart { .. }]));
    }

    #[test]
    fn f_mode_05_diff_loaded_via_update_builds_rows() {
        let mut s = fake_state();
        let raw = RawDiff {
            tracked: "diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@ -1 +1 @@\n-y\n+z\n".into(),
            untracked: vec![],
        };
        update(&mut s, Event::DiffLoaded { req: ReqId(1), result: Ok(raw) });
        assert!(s.is_ready());
        assert!(s.rows.key.is_some());
        assert!(!s.rows.rows.is_empty());
    }

    #[test]
    fn stale_results_are_ignored() {
        let mut s = fake_state();
        let before = s.note.clone();
        update(
            &mut s,
            Event::CommandDone { req: ReqId(50), result: Err(crate::integration::CommandError::NotFound) },
        );
        update(&mut s, Event::McpStarted { req: ReqId(51), result: Ok(crate::state::testutil::ep()) });
        update(&mut s, Event::ConfigSaved { req: ReqId(52), result: Ok(()) });
        update(&mut s, Event::DiffLoaded { req: ReqId(53), result: Err("x".into()) });
        assert!(!s.mcp.running);
        assert_eq!(s.load, LoadState::Loading);
        assert_eq!(s.note, before);
    }

    #[test]
    fn resize_sets_size() {
        let mut s = fake_state();
        update(&mut s, Event::Resize(Size { cols: 120, rows: 40 }));
        assert_eq!(s.size, Size { cols: 120, rows: 40 });
    }

    #[test]
    fn f_mode_05_keys_route_to_shell_handlers() {
        let mut s = fake_state();
        update(&mut s, Event::DiffLoaded { req: ReqId(1), result: Ok(RawDiff::default()) });
        let fx = key(&mut s, 'm');
        assert!(matches!(fx.as_slice(), [Effect::LoadDiff { .. }]));
        key(&mut s, 'M');
        assert!(matches!(s.overlay, Overlay::Mcp(_)));
        update(&mut s, Event::Key(KeyEvent::plain(Key::Esc)));
        key(&mut s, 'C');
        assert!(matches!(s.overlay, Overlay::Config(_)));
    }
}
