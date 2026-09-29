# Porting xplain

[`spec/SPEC.md`](spec/SPEC.md) is the source of truth for xplain's observable behavior. The parity suite in
[`e2e/`](e2e/README.md) checks that a build of the app meets it. The suite is black box: it launches a binary in a
pseudo terminal, feeds it keys, mirrors its output into a headless xterm and asserts on the screen, stdout/stderr,
exit codes, files, MCP HTTP responses and calls to fake external CLIs. It never imports app code, so it works the
same way for the TypeScript app, the bun standalone binary or a rewrite in another language.

The TypeScript app (run through `tsx`) is the reference implementation. Every scenario passes against it.

## Running the suite against a build

```sh
npm ci                                        # once: installs the harness (node 22+, git needed)
XPLAIN_BIN=/path/to/xplain-rs npm run e2e     # all scenarios against your binary
XPLAIN_BIN=/path/to/xplain-rs npm run e2e -- F-NAV    # one feature area (spec id prefix)
XPLAIN_BIN="/path/to/xplain-rs --some-flag" npm run e2e   # shell words; scenario args are appended
```

`XPLAIN_BIN` is a command line. A relative program path is resolved against the directory you run from (the repo
root under `npm run`); the app itself runs inside a temporary fixture repo. Without `XPLAIN_BIN` the suite runs the
TypeScript source through `tsx`.

The existing gates show how it fits together:

| script                      | what it runs                                                               |
| --------------------------- | -------------------------------------------------------------------------- |
| `npm run e2e`               | suite against the TS source                                                |
| `npm run e2e:bin`           | builds the bun binary (`dist/bin/xplain`), then the suite against it       |
| `npm run e2e -- --coverage` | spec coverage check only, never starts the app                             |
| `npm run check`             | typecheck, unit tests, format check, coverage, e2e (tsx), e2e (bun binary) |

CI (`.github/workflows/parity.yml`) runs `npm run check` on Linux and macOS. To gate a port, add a stage that builds
it and runs `XPLAIN_BIN=<its binary> npm run e2e`.

## What the app must provide

Beyond the user-facing behavior, a port must implement the test seams in
[SPEC.md, Test seams](spec/SPEC.md#test-seams). The harness depends on them to run without sleeps or timeouts. The
harness side is described in [e2e/README.md, App contract](e2e/README.md#app-contract).

- **`XPLAIN_SYNC=1` barriers.** The input bytes `ESC [ 9 9 9 9 ~` (idle barrier) and `ESC [ 9 9 9 8 ~` (frame
  barrier) are never keys. For each one the app writes `ESC ] 7770 ; <kind> ; <n> ; <reqs> ; <done> BEL` to stdout.
  An idle reply means all earlier input is handled, no async work is pending and the resulting frame is fully
  written. A frame reply only needs the input handled and the frame written. `<reqs>` and `<done>` are the MCP HTTP
  requests received and fully handled over the whole process; the harness uses them to wait for requests without
  polling. A lone `ESC` directly before a barrier is the Escape key. The exact rules are in the spec; getting "pending
  work" wrong usually shows up as flaky screen assertions.
- **`XPLAIN_MCP_PORT`.** The MCP server listens on `127.0.0.1:<port>`; `0` means any free port, and the effective
  port is shown wherever the spec says so. Invalid values produce the exact error text in the spec.
- **Paths from the environment.** Config at `$XPLAIN_CONFIG`, else `$XDG_CONFIG_HOME/xplain/config.json`; state
  (including `mcp.json` with the bearer token the harness reads) under `$XDG_STATE_HOME/xplain/`; `HOME` as the
  fallback. The harness gives every scenario its own `HOME`, config and state dirs and a `PATH` that contains only
  `git`, `node` and fake CLIs, so do not rely on anything else from the host.
- **Clipboard via OSC 52.** Copies are written to stdout as `ESC ] 52 ; c ; <base64 utf-8> BEL`, never through a
  system clipboard tool.
- **Terminal behavior.** Alternate screen, raw mode set by the app itself, `TERM=xterm-256color` and
  `COLORTERM=truecolor` (24-bit colors). How a frame is drawn does not matter; only the resulting xterm screen does.
- **Runtime-neutral messages.** The app never shows a runtime's error text verbatim. Errors it words itself read
  `<action> <target>: <reason>` (for example `cannot run git: not found`, `cannot read a.txt: permission denied`),
  and only the error code picks the reason: ENOENT `not found`, EACCES/EPERM `permission denied`, EISDIR
  `is a directory`, ENOTDIR `not a directory`, anything else `failed`. Map codes, not message texts; see
  [Messages](spec/SPEC.md#messages) for every site. Check `--cwd` yourself before spawning git: runtimes report a bad
  spawn directory differently.

## UNSPEC

Some behavior is deliberately left open. The [UNSPEC section](spec/SPEC.md#unspec) lists each case as `UNSPEC-<n>`
with the features it touches, for example token colors of syntax highlighting or git's exact error wording in some
cases. A port may do anything there; no scenario asserts it. In the [coverage index](spec/SPEC.md#coverage-index),
features marked `no (UNSPEC)` or `no (REMOVED)` are out of test scope entirely. When a scenario needs to touch an
UNSPEC area to reach something that is specified, it asserts only the specified part.

## Reading the report

Each scenario prints one line:

```
PASS F-MODE-04 e2e/scenarios/u03-mode/f-mode-04-spawn-error-ctrl-c.yaml 266ms
FAIL F-CLI-03 e2e/scenarios/u01-cli/f-cli-03-cwd-repo.yaml 793ms step 1: screen: expected to contain "cannot open directory"
```

A failure is followed by the failing step and a full screen dump (or stdout/stderr for non-TUI scenarios). After all
scenarios, `--- by spec id ---` lists every feature ID with passed/total scenarios, so you can read progress per
spec section (`FAIL F-CLI-03 18/19`). Scenario files are named after the spec ID they cover, and the spec ID leads
to the requirement text in `spec/SPEC.md`. The last line gives totals and the binary that was tested. The exit code
is 1 on any failure.

Useful while porting: filter by area (`npm run e2e -- F-MCPSRV`), repeat to hunt flakes (`--repeat 30`), keep temp
dirs for inspection (`E2E_KEEP=1`), and raise the hang guard for slow debug builds (`E2E_TIMEOUT=60000`).

## Coverage check

An unfiltered run, and `npm run e2e -- --coverage` on its own, compares the scenarios with the coverage index table
at the end of `spec/SPEC.md`. It fails when an in-scope feature has no scenario, when a scenario names an ID that is
out of scope or unknown, or when the index is malformed. This keeps the spec and the suite in step: a green run means
every in-scope feature was exercised, not just that the existing scenarios passed.

## When the spec changes

1. Edit the feature in `spec/SPEC.md` and its row in the coverage index (`yes`, or `no (UNSPEC)` / `no (REMOVED)`).
   Behavior left open goes into the UNSPEC section rather than being asserted.
2. Add or update scenarios under `e2e/scenarios/<area>/`, one YAML file per behavior, with `id:` set to the feature
   ID (and `also:` for extra IDs it covers). The format and step types are in
   [e2e/README.md](e2e/README.md#scenario-format). Loading is strict, so typos fail loudly.
3. Run `npm run e2e -- --coverage`, then the new scenarios against the TypeScript app (`npm run e2e -- F-XYZ-01`).
   The reference app must pass them; if it does not, either the app or the spec is wrong, and that is decided before
   any port is measured against it.
4. Run `npm run check`, then the suite against the port.
