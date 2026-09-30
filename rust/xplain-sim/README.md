# xplain-sim

Dev-only crate (`publish = false`): the in-process scenario test harness. It runs the real `xplain-core`
(`update`/`view`) with the real integration registry (`xplain_integrations::all()`), the app's own argv/config
startup (`xplain_app::run::prepare`), real git and files in temp dirs, a manual clock, a fake command runner and
in-process MCP HTTP. No terminal, no sockets, no sleeps.

```rust
use xplain_sim::{Sim, CellExpect as C};

let mut s = Sim::builder().args(["--theme", "solarized"]).build(); // standard fixture, 120x40, settled
s.keys("<Tab>j<C-f>");                                              // each key is followed by settle()
s.assert_row_matches(0, r"\[2/4\] \[cursor L30:C1\] src/big\.ts ");
s.assert_text_cell("[all]", 0, C::new().fg("#b58900"));
```

Run: `cargo test -p xplain-sim` (a scenario takes about 0.1 s). Files: `tests/uNN_<area>.rs` group scenarios by area, test fns are `f_<group>_<nn>_<what>` (spec id `F-NAV-09` -> `f_nav_09_...`).
`tests/harness.rs` tests the harness itself.

## Builder (`Sim::builder()`, then `.build()`)

| method                                   | meaning                                                                                   |
| ---------------------------------------- | ----------------------------------------------------------------------------------------- |
| `fixture(Fixture::Standard/Empty/NoGit)` | Standard = git repo, HEAD = `fixtures/base`, work tree = `fixtures/work` (default)        |
| `args([..])`, `theme(name)`              | argv, parsed by `xplain_app::cli` exactly as the binary does; `theme` prepends `--theme`  |
| `size(cols, rows)`                       | default 120x40                                                                            |
| `env(k, v)`, `env_unset(k)`              | `HOME XDG_CONFIG_HOME XDG_STATE_HOME XPLAIN_CONFIG XPLAIN_MCP_PORT XPLAIN_SYNC COLORTERM` |
| `config(text)`, `config_json(value)`     | writes `${CONFIG}/xplain/config.json` before start                                        |
| `file(path, text)`, `file_bytes(..)`     | files written before start (relative to the repo)                                         |
| `cwd(rel)`                               | app working dir inside the repo (created)                                                 |
| `shim(name)`, `shim_rules(name, [Rule])` | fake integration CLI (see below)                                                          |
| `hold_io()`                              | hold git/file effects (incl. the initial diff load) until `release_io()`                  |
| `busy_port(p)`                           | MCP start on `p` fails with the port-busy message                                         |
| `utc_offset_secs(n)`                     | local offset reported to the app (export file names), default 0                           |

`${TMP} ${REPO} ${HOME} ${CONFIG} ${STATE}` expand in args, env values, file paths (`Sim::expand`). The temp dir
is removed on drop; `XPLAIN_SIM_KEEP=1` keeps it. Temp paths: `tmp() repo() home() config_dir() state_dir()`.
The standard fixture repo is built once per fixture content (cached in the OS temp dir) and copied per Sim.

## Driving

Key notation: literal text = one key per char; tokens `<Esc> <Enter> <Tab> <S-Tab>
<Up> <Down> <Left> <Right> <Home> <End> <PageUp> <PageDown> <Space> <BS> <Del>`, `<C-x>`, `<A-x>`/`<M-x>`,
`<lt>` for `<`. Unknown tokens and a bare `<` panic.

- `keys("jj<Esc><C-f>")`, `paste(text)`, `resize(cols, rows)`: send input, settle after each key. All return `&mut Sim`.
- `settle()`: run every ready effect result until nothing is left (the sync barrier).
- `advance_clock(ms)`: manual clock; fires due timers in order, settling after each. `timers()`, `elapsed_ms()`.
  The wall clock (`state.clock`) starts at `START_UNIX_MS` = 2025-01-02T03:04:05Z.
- `hold_io()` / `release_io()` / `held_io()`: hold and release git and file effects.
- Files and git in the temp repo: `write_file`, `write_bytes`, `append_file`, `remove_file`, `mkdir`, `file(path)`,
  `try_file`, `file_exists`, `list_dir`, `file_mode`, `git(&[..])` (asserts exit 0), `git_code(&[..])`.
  They do not notify the app; press `r` like a user.
- Fake CLIs: `Rule::any()` / `Rule::args([..])` then `.exit(n) .stdout(s) .stderr(s) .error(CommandError) .block()`;
  first matching rule wins, no match = exit 0 silently, unknown program = `CommandError::NotFound`.
  `set_shim(name, rules)`, `remove_shim(name)`, `release(name)` (answers blocked calls, stops blocking),
  `calls(name)` (`Call { program, args, cwd, env }`, `starts_with(&[..])`), `blocked_calls(name)`.
- MCP over HTTP, in-process: `mcp_call(tool, json)`, `mcp_rpc(method, json)`, `http(Http)`, and for long polls
  `http_start(Http) -> Pending`, `http_reply(&Pending) -> Option<HttpReply>`, `http_await(&Pending)`,
  `http_abort(&Pending)` (core gets `McpConnClosed`). `Http` builder: `Http::tool/rpc/json/raw/oversized`,
  `.method .path .header .without_header .session .no_auth .token .remote_port`. Defaults: `POST /mcp`, host
  `127.0.0.1:<port>`, the real bearer token. Requests while the server is stopped panic ("connection refused").
  `mcp_endpoint()` gives url/token/port.

## Assertions and reading

- `screen() -> Vec<String>` (right-trimmed rows), `row(n)` (negative = from the bottom), `text()`, `render()`
  (the raw `Screen`), `dump()`.
- `contains(s)`, `matches(re)` (multiline regex over the screen text), `find(text)`, `find_nth`, `find_in_row`.
- `assert_contains/not_contains/matches`, `assert_row(n, text)`, `assert_row_contains`, `assert_row_matches(n, re)`.
  All failures print the full screen with row numbers. `capture_row(n, re) -> Vec<String>` reads groups off a row.
- Cells: `cell(x, y) -> CellView { ch, fg, bg, bold, dim, italic, underline, reverse }`, `cell_of(text)`,
  `cell_of_in_row(row, text)`; `CellView::fg_is(spec)`, `bg_is(spec)`. Color specs: `"#cb4b16"` (any case),
  `"default"`, palette names (`red`, `gray`, `red-bright`, ...) or palette index text (`"1"`).
  `assert_cell(x, y, CellExpect::new().ch('x').fg("#..").bg("default").bold(true))`,
  `assert_text_cell(text, offset, expect)`.
- Outputs: `clipboard()` (last OSC 52 text), `clipboard_all()`, `exit_code()`, `stdout()`/`stderr()` (startup only),
  `effects()`/`take_effects()` (everything core emitted), `is_ready()`.
- HTTP: `HttpReply { status, headers, body }` with `header(name)`, `text()`, `json()`, `tool_result()` (the JSON in
  `result.content[0].text`, or a string), `rpc_result()`.

## What is faked and what is real

| effect                     | in the sim                                                                                                                                              |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `LoadDiff`, `ListFiles`    | real `git` in the temp repo (hermetic env: no user/system git config), same argv/untracked rules as the app                                             |
| `ReadFile`, `WriteExport`  | real fs                                                                                                                                                 |
| `SaveConfig`               | real read-merge-write of the config file (core `apply_patch`), parents created                                                                          |
| `McpStart` / `McpStop`     | token file `mcp.json` real (dir 0700, file 0600, reuse rule); "listening" is bookkeeping, port 0 = fake port 40001+; `busy_port` simulates a taken port |
| `HttpReply`                | captured per connection id; dropped when the connection was aborted                                                                                     |
| `RunCommand`               | fake runner (scripted result, recorded call, optional block); no process is spawned                                                                     |
| `Highlight`                | real syntect run, result delivered as an event on the next settle                                                                                       |
| `Clipboard`                | recorded (decoded text, no OSC 52 bytes)                                                                                                                |
| `SetTimer` / `CancelTimer` | manual clock; only `advance_clock` fires timers                                                                                                         |
| `Exit`                     | recorded; input after exit panics                                                                                                                       |

Held or in-flight work in the real app is "pending work"; here `settle()` drains everything except held IO,
blocked fake CLIs and armed timers.

## Limitations (not testable in-process)

- Terminal layer: PTY bytes, escape sequences, the presenter diff, DECCKM, raw mode, the input decoder (`ESC`
  timing, bracketed paste framing, UTF-8 splitting), truecolor vs 256 color output, real terminal size probing.
  Cells are compared as core produces them (`Color::Rgb` / named), not as xterm reports them.
- Real sockets: TCP bind/refusal, HTTP parsing and framing by hyper, keep-alive, the 30 s body timeout, request
  body limits at the byte level (`Http::oversized` only sets core's `body_too_large`), `holdPort` collisions
  (`busy_port` fakes the result), request counters of the sync barrier.
- Process level: the binary's exit codes and streams outside startup (`main`), stderr during the run, the tokio
  runtime, `EXIT_GRACE`, real integration CLIs and `passthrough` shims, real timeouts of commands.
- Local git config of the user is ignored on purpose; a test needing it must set it in the repo.
- Highlight and IO run synchronously, so races between overlapping async results cannot be produced except with
  `hold_io` / blocked shims; ordering of completions is deterministic (effect order).
