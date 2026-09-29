# xplain-core modules

Pure crate. Boundary files (frozen): `event, effect, keys, screen, options, integration, errors`, the outline of
`state`, `theme` ids, `config`/`diff` types. Registries (`lib.rs`, `nav/mod.rs`, `mcp/mod.rs`, `view.rs`) list every
module up front. Handlers share one shape: `fn(&mut State, KeyEvent, &mut Fx) -> bool` (consumed) or `fn(&mut State, ..., &mut Fx)`.
`Fx = Vec<Effect>`. Private per-module state lives in `Ext`/`Ui` structs already embedded in `State` (only `shell` edits `state.rs`).

## Ownership

| component      | modules                                                                                  | spec IDs                                                                                                                                             |
| -------------- | ---------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| A `parse`      | `diff.rs`, `config.rs`, `theme.rs`, `fuzzy.rs`, `messages.rs`, `errors.rs`, `options.rs` | F-EDGE-01..08, F-MODE-01/02 (argv, untracked), F-SCOPE-01, F-CONFIG-01..05, F-THEME-02, Colors, F-SEARCH-01 (matcher), Messages                      |
| B `nav`        | `rows.rs`, `textutil.rs`, `nav/{mod,motion,word,viewport,visual}.rs`                     | F-CURSOR-01..10, F-VISUAL-01/02, F-NAV-01..04/07/09/10, F-LAYOUT-03..05 (row model), F-HEADER-03 (tag), F-LAYOUT-01 (H)                              |
| C `navops`     | `jump.rs`, `find.rs`, `picker.rs`, `search.rs`, `browse.rs`, `help.rs`                   | F-NAV-05/06/08, F-RELOAD-03 (cursor memo), F-FIND-01..03, F-GOTO-01/02, F-FILES-01/02, F-SEARCH-01/02, F-BROWSE-01/02, F-HELP-01..04                 |
| D `comments`   | `comments.rs`, `editor.rs`, `thread.rs`, `thread_layout.rs`                              | F-COMMENT-01..10, F-VISUAL-03, F-ASK-03 (editor), F-ASK-05..08 (thread body layout)                                                                  |
| E `agent`      | `mcp/{mod,http,rpc,tools,hub,token}.rs`, `ask.rs`, `export.rs`                           | F-MCPSRV-01..11, F-ASK-01/02/04/09, F-EXPORT-01/02, F-RELOAD-02 (trigger), Test seams (port)                                                         |
| G `shell`      | `state.rs`, `update.rs`, `reload.rs`, `quit.rs`, `config_ui.rs`, `mcp_ui.rs`             | F-CLI-05, F-MODE-03..05, F-SCOPE-02, F-THEME-01, F-RELOAD-01/03, F-QUIT-01, F-CFGUI-01..03, F-MCPUI-01..04, F-INTEG-02..06 (flow), F-LAYOUT-04 (`s`) |
| F1 `viewframe` | `canvas.rs`, `view.rs`, `view/{header,modals,help_panel}.rs`                             | F-LAYOUT-01/02/06/07/08, F-HEADER-01/02, F-MODE-04/05 screens, F-HELP-01 (panel), modal render of F-FILES/SEARCH/CFGUI/MCPUI/QUIT/COMMENT-08         |
| F2 `viewrows`  | `highlight.rs`, `view/{rows,thread_box}.rs`                                              | F-LAYOUT-03..05 render, F-CURSOR-02, F-VISUAL-02, F-FIND-02 render, F-EDGE-06/08, F-COMMENT-04/10 render, F-ASK-05..08 render, UNSPEC-31             |

## Interfaces (who calls whom)

- `update` (G) routes per the order in its doc; all feature code is behind the fns named there.
- Cursor: everyone moves the cursor via `nav::place(state,row,col)` (B); viewport via `nav::viewport::*` (B). Rows: `rows::ensure(state)` after
  changing `file_index`/`split`/`browse`/`files`; readers use `state.rows.rows` and `rows::{row_no,row_code,pane_of,...}`.
- Viewport needs comment box heights: `thread_layout::row_extra_height` (D). View draws boxes from `thread_layout::boxes_at` (D) via `view::thread_box` (F2).
- Comments (D): `comments::{from_cursor,insert,alloc_id,anchored_at,...}`; editor submit in ask mode -> `ask::ask_comment` (E).
- MCP (E): `McpState::{handle_http,enqueue,stop,poll_timeout,conn_closed}` return `McpOutput`; `ask::apply_output` folds it into state (answers, notes,
  clients, files_changed -> `reload::on_files_changed`). `mcp_ui` (G) calls `handle_http` and `ask::apply_output`; `ask` calls `state.mcp.enqueue`.
- Help (C): `help::footer_hints` used by `view::header`; `help::{help_ctx,entries_for}` by `view::help_panel`.
- Reload (G): `jump::{remember,restore,open_file}` (C), `diff::parse_raw` (A).
- Modals: state in `state.overlay`; behavior in `picker/search/config_ui/mcp_ui/quit/thread`; drawing only in `view::modals` (F1).
- Notes: any module sets `state.note = Some(..)`; strings from `messages.rs` (A) or local consts.

## Rules

- Spec ID in doc comments and test names (`f_nav_05_...`). Unit tests in-file. No unwrap/expect/panic in runtime paths.
- No agent names. No IO. Fill `todo!()` only; never edit files of another component. Need a signature change -> report it.

## Design moves during implementation (add-only)

- State: diff_req/diff_kind/diff_nav/last_token/autostart_req; DiffLoadKind, DiffNav; IntegrationState.keep_note; Pending::BrowseReload{path}.
- ThreadUi: heads, chosen, num_go, num_last, seen.
- update runs rows::ensure and thread::sync after every event; FileRead goes to reload::on_browse_reread first.
- MCP stop: thread::on_mcp_stopped + ask::cancel_live; McpState::reset_hub on new start.
- HttpResponse headers already include content-type; runtime must not add it.
- config.rs has private ordered JSON writer (key order stable).
- highlight.rs uses syntect; toml/ini plain.
