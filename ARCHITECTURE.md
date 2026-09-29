# xplain Rust rewrite: architecture

The Rust app lives in `rust/` next to the TypeScript reference app (`src/`, untouched). The contract is
[`spec/SPEC.md`](spec/SPEC.md) (feature IDs `F-<GROUP>-NN`, Test seams, Messages, UNSPEC). The gate is the parity
suite in [`e2e/`](e2e/README.md) run against the Rust binary (see [`PORTING.md`](PORTING.md)). Anything in UNSPEC is
free; everything else must match.

## 1. Model: Elm style

```
        keys, paste, resize, timers, HTTP requests, IO results
                          | Event
                          v
   State --> update(&mut State, Event) -> Vec<Effect> --> Runtime executes effects
     |                                                     (git, fs, HTTP replies, timers, OSC 52,
     v                                                      integration CLIs, exit)
   view(&State) -> Screen  --> Presenter draws                     |
                                                                   +--> result Events fed back
```

- `update` and `view` are pure and synchronous. No IO, no clock, no randomness, no threads in `xplain-core`.
  The runtime writes `state.clock` (wall clock snapshot) before each `update`; MCP requests carry 16 bytes of
  `entropy` for session UUIDs.
- All async work is an `Effect` carrying a `ReqId`; the result comes back as an `Event` with the same `ReqId`.
  Core keeps `State::pending: HashMap<ReqId, Pending>` and drops results whose id is unknown (stale).
- Timers are effects (`SetTimer`) and events (`Timer(TimerId)`); a test can inject `Event::Timer` by hand, so time
  can be faked. Spinner (80 ms) and long-poll waits are `background` timers and never count as pending work.
- `view` returns a `Screen` (grid of styled cells, no escape codes, no terminal crate types). This keeps all
  layout logic testable in core: assert on `screen.row_text(y)` and cell styles. The app's `Presenter` only turns a
  `Screen` into terminal bytes. (Deviation from "ratatui does layout": ratatui is at most the buffer/backend inside
  `Presenter`; layout is core's.)
- Viewport `top`, horizontal shift, cursor and all scrolling are _state_, changed in `update` (F-NAV-09, F-CURSOR-07),
  never in `view`.

### Boundary types (all defined in `xplain-core`, frozen at skeleton)

| type / fn                                                                                        | file                    | role                                         |
| ------------------------------------------------------------------------------------------------ | ----------------------- | -------------------------------------------- |
| `Event`, `ReqId`, `TimerId`                                                                      | `event.rs`              | only input of `update`                       |
| `Effect`                                                                                         | `effect.rs`             | only output of `update` (besides state)      |
| `State` (+ `Nav`, `Overlay`, `Settings`, `Pending`, ...)                                         | `state.rs`              | whole app state, outline of top-level fields |
| `update(&mut State, Event) -> Vec<Effect>`                                                       | `update.rs`             | reducer                                      |
| `view(&State) -> Screen`, `Screen`, `Cell`, `Style`, `Color`, `Size`                             | `view.rs`, `screen.rs`  | render                                       |
| `KeyEvent`, `Key`, `Mods`                                                                        | `keys.rs`               | terminal independent keys                    |
| `FileDiff`, `Hunk`, `DiffLine`, `DiffSpec`, `RawDiff`, `parse_raw`                               | `diff.rs`               | diff model + parser                          |
| `ThemeId`, `Theme`                                                                               | `theme.rs`              | themes                                       |
| `Config`, `ConfigLoad`, `ConfigChange`, `load_config`, `apply_patch`, `resolve_config_path`      | `config.rs`             | config                                       |
| `Options`, `DiffMode`, `EnvInfo`, `Init`                                                         | `options.rs`            | resolved startup inputs                      |
| `HttpRequest`, `HttpResponse`, `ConnId`, `McpState`, `McpEndpoint`, `HubEvent`                   | `mcp.rs`                | MCP protocol as state machine                |
| `Comment`, `Turn`, `Answer`, `PaneSide`                                                          | `comments.rs`           | comment/thread model                         |
| `AgentIntegration`, `CommandSpec`, `CommandResult`, `RegStatus`, `Integrations`                  | `integration.rs`        | integration trait                            |
| `IoReason`, `fail_msg`                                                                           | `errors.rs`             | runtime-neutral error words                  |
| `parse_args`, `Cli`, `USAGE`                                                                     | `xplain-app/src/cli.rs` | argv                                         |
| `run::main_with_args`, `runtime::run_loop`, `Executor`, `Presenter`, `InputDecoder`, `McpServer` | `xplain-app`            | runtime                                      |

Changing a boundary type needs the core lead; everything behind a boundary is free for its owner.

### Where the integration registry lives

Core needs to drive integrations (confirm dialog, busy flag, notes, re-check) without knowing any agent. The trait and
`CommandSpec`/`CommandResult` are in core (`integration.rs`); `State::new(init, integrations)` receives
`Vec<Arc<dyn AgentIntegration>>`. `xplain_integrations::all()` builds the real registry (modal order: Claude Code,
Codex, OpenCode, Copilot); core tests pass fakes. Integrations return data (`CommandSpec`, texts, parsed status), never
run anything.

## 2. Crates and dependency rules

```
xplain-app  --> xplain-core
     \--------> xplain-integrations --> xplain-core
```

- `xplain-core`: pure. Allowed deps: serde, serde_json, unicode-width, syntect (highlighting is pure text -> spans).
  Forbidden: tokio, crossterm, ratatui, std::fs/process/net/time (except types), rand, any agent name.
- `xplain-integrations`: depends on core (trait, shared types) and serde_json. No IO. Only crate with agent names,
  argv shapes and agent-specific texts (memory rule: no leaky abstractions; nothing agent specific in UI code).
- `xplain-app`: everything with side effects. Depends on both. Contains no UI logic and no decisions core can make.
  It never shows OS error text: `IoReason::from_io_error` then core/`fail_msg` words.
- Dependency versions are pinned once in `rust/Cargo.toml` `[workspace.dependencies]`; crates write
  `foo.workspace = true`. Add a dependency to a crate only when used and allowed above.

## 3. Runtime loop and the sync barrier

`xplain-app::runtime::run_loop`:

1. Startup (`run::main_with_args`): parse argv (`cli::parse_args`), resolve config path, read config file,
   `config::load_config` (print warnings to stderr before UI), build `Init`, `State::new` -> initial effects
   (`LoadDiff`). Enter alternate screen, draw first frame (`Loading...`), send `Event::Started` (core starts MCP if
   `autostart`, F-MCPUI-04).
2. Loop: read stdin bytes -> `InputDecoder` -> items (`Key`, `Paste`, `Resize`, `Barrier`). For each key/paste/resize
   and for every result/timer/HTTP event from channels: set `state.clock`, `update`, hand effects to the `Executor` in
   order. After the queue drains: `view` -> `Presenter::draw` (flushed). Then answer barriers.
3. Exit: `Effect::Exit{code}` runs after all earlier effects (including `HttpReply`s and `McpStop`) finished; leave
   alternate screen; exit code. Ctrl+C is handled by core (`Exit{0}`) in every state.

Barrier mapping (SPEC Test seams), with `XPLAIN_SYNC=1` only:

| spec rule                                   | runtime mechanism                                                                                                                                                                                                               |
| ------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| barrier bytes are never keys                | `InputDecoder` emits `Barrier(Idle/Frame)`, never reaches `update`; lone `ESC` directly before a barrier -> `Key(Esc)` first, no escape timeout                                                                                 |
| input before barrier handled                | barrier queued in input order; replied only after all earlier items went through `update`                                                                                                                                       |
| frame fully written                         | reply after `Presenter::draw` returned (flushed) for the state produced by earlier input                                                                                                                                        |
| idle: no pending async work                 | `PendingWork == 0` (every non-background effect increments on dispatch, decrements after its result event was _enqueued_) and event queue empty and no un-run effect; then re-`update` results first, draw, and only then reply |
| frame: pending work ignored                 | reply right after the frame                                                                                                                                                                                                     |
| before ready                                | idle waits for `State::is_ready()` (first frame + initial load); frame waits for first frame                                                                                                                                    |
| spinner not pending                         | `SetTimer{background:true}`                                                                                                                                                                                                     |
| `<n>` `<reqs>` `<done>`                     | barrier counter; `HttpCounters.received` incremented on request arrival (before body read), `done` when response written or connection gone and handler finished                                                                |
| state change before HTTP response written   | natural: `update` runs (state changed) before the `HttpReply` effect is executed                                                                                                                                                |
| in-flight long poll already registered      | poll is registered inside `update` (core state) before the reply; `received` may exceed `done` legally                                                                                                                          |
| reading a request body is pending work      | `PendingWork` held between accept and `Event::McpHttp`                                                                                                                                                                          |
| input after barrier handled after its reply | replies written synchronously when conditions hold, before the next input item is processed                                                                                                                                     |

Without `XPLAIN_SYNC` none of this exists: bytes are ordinary input, no replies.

## 4. Spec groups -> crates/modules

| spec group                                                              | primary crate                                                                          | notes                                                         |
| ----------------------------------------------------------------------- | -------------------------------------------------------------------------------------- | ------------------------------------------------------------- |
| CLI (F-CLI-01..06)                                                      | app `cli.rs`, `run.rs`                                                                 | pure argv parser + orchestration; `--cwd` dir check in `exec` |
| CONFIG (F-CONFIG-01..05)                                                | core `config.rs` (+ app write side in `exec`)                                          | exact warning strings in core                                 |
| MODE (F-MODE-01..05), SCOPE, RELOAD                                     | core `diff.rs` (argv builder, parse), reducer; app `exec` (git run, untracked listing) |                                                               |
| EDGE (F-EDGE-01..08), diff model                                        | core `diff.rs`                                                                         | parser derives status, notes, no-newline rows                 |
| LAYOUT, HEADER, THEME, CURSOR render, VISUAL render, comment box render | core `view`, `theme.rs`                                                                | pure cell grid                                                |
| NAV, CURSOR motions, VISUAL, count, Esc chain                           | core reducer (`update`), keymap per context                                            | viewport follow is state                                      |
| FILES, SEARCH (matcher F-SEARCH-01), BROWSE, FIND, GOTO                 | core                                                                                   | `ListFiles`/`ReadFile` effects                                |
| COMMENT, ASK, EXPORT                                                    | core `comments.rs` + reducer + export markdown                                         | export write = `WriteExport` effect                           |
| HELP (F-HELP-01..04)                                                    | core (context table, layout)                                                           |                                                               |
| QUIT, CFGUI                                                             | core                                                                                   | save = `SaveConfig` effect                                    |
| MCPUI (F-MCPUI-01..04)                                                  | core (modal state + view)                                                              | start/stop = `McpStart`/`McpStop`                             |
| MCPSRV (F-MCPSRV-01..11)                                                | core `mcp.rs` (protocol) + app `http.rs` (sockets, token file)                         | auth/host/origin checks pure in core                          |
| INTEG (F-INTEG-01..06)                                                  | integrations (data) + core (flow) + app `exec` (spawn)                                 |                                                               |
| Test seams                                                              | app `runtime.rs`, `input.rs`, `http.rs`                                                | section 3                                                     |
| Messages                                                                | core `errors.rs`; app maps OS errors to `IoReason`                                     |                                                               |
| Colors, syntax highlighting (UNSPEC-31)                                 | core `theme.rs`, highlight module                                                      | syntect, pure                                                 |

Detailed module tree and ownership inside each crate is decided by the three crate leads (one ownership table per
crate, appended to this file under section 8 when made). Rule for them: modules small and disjoint, each starting with
a doc comment (purpose, spec IDs, what it must not do), shared registries (`lib.rs`, `mod.rs`) listing all modules up
front so workers never edit them.

## 5. Testing strategy

- Unit tests per module in-file (`#[cfg(test)]`). Core tests drive `update` with hand-made `Event`s and assert on
  effects and `view` screens (`Screen::row_text`). No terminal, no tokio needed.
- Runtime tests: `InputDecoder` byte-for-byte; barrier logic with a fake `Executor` and fake `Clock`.
- Integrations: assert exact argv and texts from SPEC F-INTEG-*.
- Gate: parity suite `XPLAIN_BIN=rust/target/release/xplain npm run e2e`. Progress is read per spec ID from
  `--- by spec id ---` (`PASS`/`FAIL F-XXX n/m`). A component is done when its spec IDs pass 100% and unit tests pass.
  `npm run e2e -- --coverage` stays green (spec and suite in step).
- Never edit `spec/`, `e2e/`, `src/`, `tests/` from Rust work. Spec doubts go to the spec owner.
- Regression rule: fix a parity failure by adding a unit test first when it can be reproduced in core.

## 6. Build order

1. Spike (app lead + core lead): `InputDecoder`, `run_loop` with barrier replies, `Presenter`, `State::new`,
   `LoadDiff` effect, `view` producing header/rule/footer + `Loading...` and a plain unified diff. Goal: F-CLI-01..06,
   F-CLI-05, barrier plumbing and first frame pass in e2e; this proves the runtime/barrier design.
2. Core in parallel per spec group: diff parser + git argv, config, reducer navigation/cursor, views (unified/split/
   browse, header/footer, themes), modals (picker, search, config, quit).
3. Comments, visual, find/goto, export, help panel.
4. MCP protocol + HTTP server + MCP modal, integrations, ask flow.
5. Polish: UNSPEC decisions, `cargo clippy`, release binary size, CI stage (`XPLAIN_BIN=...`).

## 7. Conventions

- Rust edition 2024, MSRV 1.85, `rustfmt` (`rust/rustfmt.toml`), `cargo clippy --all-targets` clean (workspace lints:
  `todo` allowed only during skeleton; `unwrap_used`, `expect_used`, `panic`, `print_*` warn; `unsafe` forbidden).
- No `unwrap`/`expect`/`panic!`/indexing-that-can-panic in runtime paths (`xplain-app`, reducer, view). Tests may.
  Bad external input never crashes: it becomes a note, an error screen or an HTTP error.
- Errors: core is infallible where possible (results are `Event`s with `Result` payloads carrying `IoReason` or final
  message strings). Runtime: `Result<_, IoReason>` at the effect boundary; no `anyhow`, no error text of std/libraries
  shown to users (Messages section).
- Naming: modules and files `snake_case`; types `CamelCase`; spec IDs in doc comments as `F-XXX-NN` so
  `grep -rn F-CURSOR-04 rust/` finds implementation and tests. Test names start with the spec id in snake case
  (`f_cursor_04_word_motions_cross_rows`).
- Strings that the spec gives verbatim live in one place per crate (`const`), with the spec ID in a comment.
- Columns/widths: `unicode-width` cells for layout; column unit for cursor/selection is chars (UNSPEC-28; one decision, document it).
- Every module file starts with a `//!` doc: purpose, owned spec IDs, must-not list.
- No commit attribution lines; commit style per repo history.

## 8. Ownership tables

To be added by the crate leads (module path -> spec IDs -> component) once they split their crate.
