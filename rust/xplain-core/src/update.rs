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
//! After every event: `rows::ensure`.

use crate::effect::Effect;
use crate::event::Event;
use crate::state::State;

/// Apply `event` to `state` and return the effects to run. Pure and deterministic given
/// `state.clock`. Ctrl+C always yields `Effect::Exit { code: 0 }` (F-CLI-05).
pub fn update(_state: &mut State, _event: Event) -> Vec<Effect> {
    todo!("reducer dispatch")
}
