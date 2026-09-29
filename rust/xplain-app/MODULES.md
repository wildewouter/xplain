# xplain-app modules

IO crate. Core decides, app executes. All modules registered in `src/lib.rs` up front; workers never edit `lib.rs`,
`Cargo.toml` (ask the lead), or files of another component. Every file has a `//!` doc with spec IDs and must-nots.

## Module tree and ownership

| module | responsibility | spec IDs | component |
| --- | --- | --- | --- |
| `main.rs` | calls `run::main_with_args`, exits. No logic | - | (lead, done) |
| `cli.rs` | pure argv parser, `USAGE`, error texts | F-CLI-01..04, F-CLI-06 | B |
| `env.rs` | `RawEnv` snapshot, config path env, state dir, sync/truecolor flags, `EnvInfo` | F-CONFIG-01, F-MCPSRV-01 (dir), Test seams (env) | B |
| `config_io.rs` | read config file (`ConfigFile`), `SaveConfig` read-merge-write | F-CONFIG-01/04/05, F-CFGUI-03 | B |
| `git.rs` | `LoadDiff` (cwd check, git, untracked), `ListFiles` | F-MODE-01/02/04, F-CLI-06, F-FILES/F-SEARCH listing, UNSPEC-37 | B |
| `fsio.rs` | `ReadFile`, `WriteExport` | F-BROWSE-01, F-COMMENT-09, F-EXPORT-01 | B |
| `run.rs` | `prepare` (argv -> config -> `State`) + `main_with_args` orchestration, stderr warnings, exit codes | F-CLI-01..05, F-CONFIG-03/04 | B |
| `input.rs` | byte decoder: keys, paste, barriers | Test seams, F-NAV-07, F-CLI-05, UNSPEC-8/26 | A |
| `barrier.rs` | pure barrier queue (numbering, ordered release) | Test seams | A |
| `runtime.rs` | `drive` loop, `Clock` trait, `barrier_reply`, `run_loop` wiring, handles Clipboard/SetTimer/CancelTimer/Exit | Test seams, F-CLI-05, F-RELOAD-02 | A |
| `present.rs` | `Screen` -> bytes (`encode_frame`), alt screen enter/leave | F-CLI-05, F-LAYOUT-01, Colors | A |
| `term.rs` | raw mode, size, stdin reader thread, SIGWINCH | F-CLI-05, F-LAYOUT-01 | A |
| `timers.rs` | `RealClock`: wall clock + tokio timers, pending accounting | Test seams (background timers), F-ASK-05, F-MCPSRV-06 | A |
| `clipboard.rs` | OSC 52 bytes | F-ASK-08, PORTING clipboard | A |
| `exec.rs` | `Executor` trait, `PendingWork`, `RealExecutor` dispatcher | Test seams (pending), all IO effects | C |
| `http.rs` | MCP server sockets, `HttpCounters`, `McpServer` | F-MCPSRV-01/02 (socket), F-MCPSRV-06 (drop), F-MCPUI-03 (drain), Test seams (reqs/done) | C |
| `token.rs` | `mcp.json` token file IO | F-MCPSRV-01 | C |
| `proc.rs` | integration CLI runner with timeout | F-INTEG-01..04, UNSPEC-37 | C |

## Interfaces between modules (fixed signatures in the files)

- `run::prepare(args, &RawEnv, abs_cwd, Size, read_config) -> Startup` (B). `Startup::Ui` -> `runtime::run_loop(state, effects, RuntimeConfig{sync, truecolor})` (A).
- `runtime::drive(...)` (A) takes injected `Executor`, `Clock`, channels, output writer: unit-testable with fakes.
- `Executor::dispatch(Effect)` (C) returns immediately; results -> `Event` via channel. `PendingWork::begin` before spawn, `end` after result event sent.
- Runtime handles `Clipboard` (uses `clipboard::osc52`), `SetTimer`/`CancelTimer` (via `Clock`), `Exit`. Everything else goes to `Executor`. Exec ignores those four.
- `Exit{code}`: runtime waits `PendingWork == 0` (so `HttpReply`s written, `McpStop` done), leaves alt screen, returns.
- Exec calls B's functions: `git::load_diff`, `git::list_files`, `fsio::read_file`, `fsio::write_export`, `config_io::save_config`; C's own: `proc::run_command`, `token::ensure_token` (called inside `McpServer::start`), `http::McpServer`.
- `http::HttpCounters` is created by `run_loop`, shared with `RealExecutor::new`, `McpServer::start`, `runtime::barrier_reply`.
- `timers::RealClock::new(tx, pending)`; `Clock::now` fills `state.clock` before every `update`.
- Errors: only `IoReason::from_io_error` / `fail_msg`; never OS text.
- Sync: `Barrier` items from `input` -> `barrier::BarrierQueue::push`; after each settle `due(Settle{frame_written, idle})` -> `barrier_reply` bytes written after the frame flush.

## Components

- A `runtime`: input, barrier, runtime, present, term, timers, clipboard.
- B `startup-io`: cli, env, config_io, git, fsio, run.
- C `mcp-exec`: exec, http, token, proc.
