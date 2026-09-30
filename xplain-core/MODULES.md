# xplain-core modules

Pure crate: no IO, clock, async or agent names. Handlers share one shape: `fn(&mut State, KeyEvent, &mut Fx) -> bool`
(consumed) or `fn(&mut State, ..., &mut Fx)`. `Fx = Vec<Effect>`. One `State` (Elm style, `&mut State` handlers by design);
per-feature bookkeeping lives in sub-structs (`Loader`, `McpUi`, `ThreadUi`, `AskState`, `PickerUi`, `HlCache`, ...).
`lib.rs` registers every module up front. Only the boundary API is public: `event, effect, keys, screen, options,
integration, errors, state, config, diff, mcp, theme` plus root re-exports (`State, update, view, Event, Effect, Screen,
PaneSide, HlKey, highlight_lines`). `comments`, `highlight`, `hlcache`, `view` are crate-private.

## Layers (a module may use modules of its own layer or below; never above)

```
L5 view      canvas  view  view/{header,modals/{picker,config,mcp,dialog},help_panel,body,thread_box,layout}
               |  reads &State, produces Screen
L4 shell     update  reload  quit  config_ui  mcp_ui
               |  routing + lifecycle (diff load, config, MCP start/stop, integrations flow)
L3 features  nav/{motion,word,viewport,visual}  jump  find  picker  search  browse  help
             comments  editor  thread  thread_layout/{text,body,window,boxes}  ask  export  hlcache  rows
               |  one feature each, mutate State through small APIs
L2 state     state  (Loader, McpUi, Overlay, Nav, ...)  mcp/{mod,hub,rpc,tools,http,text,token}
               |  data + `Overlay::route_key` dispatch, `State::{current_path,alloc_req,set_note}`
L1 parse     diff  config  theme  fuzzy  highlight  textutil  textinput  messages
               |
L0 boundary  event  effect  keys  screen  errors  options  integration
```

`state` calls feature handlers only in `Overlay::route_key`; everything else in `state` is data. `Pending` and `Loader`
map outstanding `ReqId`s back to their purpose, stale ids are dropped.

## Ownership

## Ownership

| component      | modules                                                                                                     | spec IDs                                                                                                                                             |
| -------------- | ----------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| A `parse`      | `diff.rs`, `config.rs`, `json_ordered.rs`, `theme.rs`, `fuzzy.rs`, `messages.rs`, `errors.rs`, `options.rs` | F-EDGE-01..08, F-MODE-01/02 (argv, untracked), F-SCOPE-01, F-CONFIG-01..05, F-THEME-02, Colors, F-SEARCH-01 (matcher), Messages                      |
| B `nav`        | `rows.rs`, `textutil.rs`, `nav/{mod,motion,word,viewport,visual}.rs`                                        | F-CURSOR-01..10, F-VISUAL-01/02, F-NAV-01..04/07/09/10, F-LAYOUT-03..05 (row model), F-HEADER-03 (tag), F-LAYOUT-01 (H)                              |
| C `navops`     | `jump.rs`, `find.rs`, `picker.rs`, `search.rs`, `browse.rs`, `help.rs`, `textinput.rs`                      | F-NAV-05/06/08, F-RELOAD-03 (cursor memo), F-FIND-01..03, F-GOTO-01/02, F-FILES-01/02, F-SEARCH-01/02, F-BROWSE-01/02, F-HELP-01..04                 |
| D `comments`   | `comments.rs`, `editor.rs`, `thread.rs`, `thread_layout/`                                                   | F-COMMENT-01..10, F-VISUAL-03, F-ASK-03 (editor), F-ASK-05..08 (thread body layout)                                                                  |
| E `agent`      | `mcp/{mod,http,rpc,tools,hub,text,token}.rs`, `ask.rs`, `export.rs`                                         | F-MCPSRV-01..11, F-ASK-01/02/04/09, F-EXPORT-01/02, F-RELOAD-02 (trigger), Environment (port)                                                         |
| G `shell`      | `state.rs`, `update.rs`, `reload.rs`, `quit.rs`, `config_ui.rs`, `mcp_ui.rs`                                | F-CLI-05, F-MODE-03..05, F-SCOPE-02, F-THEME-01, F-RELOAD-01/03, F-QUIT-01, F-CFGUI-01..03, F-MCPUI-01..04, F-INTEG-02..06 (flow), F-LAYOUT-04 (`s`) |
| F1 `viewframe` | `canvas.rs`, `screen.rs`, `view.rs`, `view/{header,modals/,help_panel,layout}.rs`                           | F-LAYOUT-01/02/06/07/08, F-HEADER-01/02, F-MODE-04/05 screens, F-HELP-01 (panel), modal render of F-FILES/SEARCH/CFGUI/MCPUI/QUIT/COMMENT-08         |
| F2 `viewrows`  | `highlight.rs`, `view/{body,thread_box}.rs`                                                                 | F-LAYOUT-03..05 render, F-CURSOR-02, F-VISUAL-02, F-FIND-02 render, F-EDGE-06/08, F-COMMENT-04/10 render, F-ASK-05..08 render, UNSPEC-31             |

## Interfaces (who calls whom)

- `update` (G) routes per the order in its doc (overlay keys via `Overlay::route_key`); all feature code is behind the fns named there.
- Cursor: everyone moves the cursor via `nav::place(state,row,col)` (B); viewport via `nav::viewport::*` (B). Rows: `rows::ensure(state)` after
  changing `file_index`/`split`/`browse`/`files`; readers use `state.rows.rows` and `rows::{row_no,row_code,pane_of,...}`.
- Viewport needs comment box heights: `thread_layout::row_extra_height` (D). View draws boxes from `thread_layout::boxes_at` (D) via `view::thread_box` (F2).
- Comments (D): `comments::{from_cursor,insert,alloc_id,anchored_at,...}`; editor submit in ask mode -> `ask::ask_comment` (E).
- MCP (E): `McpState::{handle_http,enqueue,stop,poll_timeout,conn_closed}` return `McpOutput`; `ask::apply_output` folds it into state (answers, notes,
  clients, files_changed -> `reload::on_files_changed`). `mcp_ui` (G) calls `handle_http` and `ask::apply_output`; `ask` calls `state.mcp.enqueue`.
- Help (C): `help::footer_hints` used by `view::header`; `help::{help_ctx,entries_for}` by `view::help_panel`.
- Reload (G): `jump::{remember,restore,open_file}` (C), `diff::parse_raw` (A).
- Modals: state in `state.overlay`; behavior in `picker/search/config_ui/mcp_ui/quit/thread`; drawing only in `view::modals` (F1).
- Notes: any module calls `state.set_note(..)`; shared strings (`MCP_OFF`, follow-up notes) live in `messages.rs` (A), single-use ones stay local consts.
- Text inputs: find, goto, search and the comment editor share `textinput` (insert/backspace/push, explicit `NewlinePolicy`: Collapse for find/editor, Drop for goto/search). `ask::ask_focused` is the only `a` handler; `comments::{can_reply,edit_message,append_turn}` own the reply rule and turn edits.

## Rules

- Spec ID in doc comments and test names (`f_nav_05_...`). Unit tests in-file. No unwrap/expect/panic in runtime paths.
- No agent names. No IO. `todo!()` is denied by lint. Need a signature change in another component -> report it.

## Design moves during implementation (add-only)

- State: `loader` (pending, next_req, diff_req, diff_kind, diff_nav), `mcp_ui` (last_token, autostart_req); DiffLoadKind, DiffNav; IntegrationState.keep_note; Pending::BrowseReload{path}.
- ThreadUi: heads, chosen, num_go, num_last, seen.
- update runs rows::ensure and thread::sync after every event; FileRead for a `Pending::BrowseReload` request goes to reload::on_browse_reread, others to browse::on_file_read.
- MCP stop: thread::on_mcp_stopped + ask::cancel_live; McpState::stop resets the hub (server lifecycle is `mcp::ServerState`: Stopped{last_error} / Starting(req) / Running(endpoint), read via is_running/starting/endpoint/start_error).
- HttpResponse headers already include content-type; runtime must not add it.
- config.rs keeps user key order via `json_ordered.rs` (order-preserving JSON parse/print); `mcp/text.rs` holds sanitize/cap_chars shared by hub, rpc, tools.
- highlight.rs uses syntect; toml/ini plain.
- highlight cache: `hlcache/` (F2; `mod.rs` cache + shell hook, `ranges.rs` per-side ranges, `code_lru.rs` thread code LRU, `plan.rs` pure planner over `&State` whose plans `HlCache` applies). `update` runs `hlcache::sync` after every event; it emits `Effect::Highlight` for uncovered lines within +-100 rows of the viewport (trigger +-50) and `Event::Highlighted` fills `state.hl`. `view` only reads `HlCache::{runs,code_runs}` (token classes, colors from `highlight::run_style` per theme). Cache key = path + side + content hash, LRU current file + 3, files > 50k lines plain, thread code lines LRU 500 computed once on thread change.

- Text/width: `textutil` is the single source for wrap (`wrap_text`, F-ASK-05 rules), char/cell width (`char_width`, `cell_width`; canvas draws and truncates with them). `view.rs::wrap_hard` and `help_panel::wrap` stay separate on purpose (different rules: keep-spaces error screen, split-on-space help text).
- View: `Seg`/`seg`/`fg`/`bold` live in `screen.rs`; `view/layout.rs` holds shared modal geometry; `view/modals/` = `mod` (Look, place, dispatch) + `picker` (files + search) + `config` + `mcp` + `dialog`; `view/body.rs` draws the viewport body.
- thread_layout split: `mod` (Tone/Span/BoxLine/ThreadBox, consts, `box_width`), `text` (BodyLine model, fences, `rich_lines`), `body` (`thread_body`), `window` (`window_body`, heights, `thread_info`), `boxes` (frames, hints, `comment_box`, `editor_box`, `boxes_at`, `row_extra_height`). Focused height is arithmetic (`focused_height`), a test pins all height fns to the drawn line counts.
