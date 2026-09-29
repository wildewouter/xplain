# xplain e2e (black box)

Drives the real binary in a PTY, mirrors its output into a headless xterm and checks screen, files, HTTP (MCP) and
fake external CLIs. Language agnostic: any implementation that honors the [app contract](#app-contract) can be tested
with `XPLAIN_BIN`. The TS app is the oracle. Scenarios never run shell code: files, modes, links, git and fake CLIs
are all harness steps.

## Run

```sh
npm run e2e                          # all scenarios, concurrently
npm run e2e -- F-NAV                 # filter: spec id (exact or prefix), path substring or glob (nav/*.yaml)
npm run e2e -- --repeat 30 --jobs 16 # flake hunting
npm run e2e -- --list
XPLAIN_BIN=/path/to/xplain npm run e2e
```

Output: `PASS <id> <file> <ms>` / `FAIL <id> <file> <ms> step <n>: <why>` plus the step and a full screen dump (or
stdout/stderr for `tui: false`), then a per spec id summary. Exit code 1 on any failure.

| env            | default                             | meaning                                                                 |
| -------------- | ----------------------------------- | ----------------------------------------------------------------------- |
| `XPLAIN_BIN`   | `tsx --tsconfig <root> src/cli.tsx` | command line (shell words) that starts the app; args are appended       |
| `E2E_TIMEOUT`  | `30000`                             | per scenario hang guard in ms. Only reports hangs, never used as a wait |
| `E2E_JOBS`     | CPU count                           | concurrent scenarios (`--jobs`)                                         |
| `E2E_REPEAT`   | `1`                                 | runs per scenario (`--repeat`)                                          |
| `E2E_UPDATE=1` |                                     | rewrite `expectGolden` files instead of comparing                       |
| `E2E_KEEP=1`   |                                     | keep scenario temp dirs                                                 |

No sleeps anywhere: every input is followed by a sync barrier and the runner waits for the app's reply. If the
process exits while a step waits, the step fails at once (unless the next step is `expectExit`).

## Isolation

Each scenario gets a fresh temp dir `${TMP}` with:

- `${REPO}`: fixture repo, the app's cwd (unless `cwd`). Built by `e2e/fixture.sh <dir> <kind>` (also used by
  `tests/run.sh`): `standard` = git repo, HEAD = `tests/fixture/base`, working tree = `tests/fixture/work` (`*.fx`
  renamed); `empty` = git repo with one empty commit; `nogit` = plain empty dir.
- `${HOME}`, `${CONFIG}` (`XDG_CONFIG_HOME`), `${STATE}` (`XDG_STATE_HOME`), plus `XDG_CACHE_HOME`, `XDG_DATA_HOME`.
- `${SHIMS}`: fake CLIs, first on `PATH`. The host `PATH` is not inherited: `PATH=${SHIMS}:<node,git>:/usr/bin:/bin`,
  so a real agent CLI is never run. Its files are harness internals; scenarios change shims with `shim` steps.
- `${PORT}`: a free port, passed as `XPLAIN_MCP_PORT`.
- `TERM=xterm-256color`, `COLORTERM=truecolor`, `LANG=en_US.UTF-8`, `XPLAIN_SYNC=1`, git author/committer set.

## Scenario format

`e2e/scenarios/**/*.yaml`. Loading is strict: unknown top-level fields, unknown step types, unknown keys in any step,
matcher or `expect` object, unknown `$` operators, bad regexes, wrong types, empty matchers, rows/cols outside the
screen, `${VAR}`s not defined by an earlier step, `expectShimCall`/`release` of undeclared shims and shell syntax in
`git` steps are load errors (`LOAD: <file>: step <n> (<type>): <path>: <why>`), so a typo never becomes a vacuous
assert.

`${VAR}` expands in every string except `keys` and `paste` (`TMP REPO HOME CONFIG STATE SHIMS PORT ROOT` plus
variables set by `capture`, `captureScreen`, `expectFile.capturePath`, `holdPort.var`). Relative paths are relative
to `${REPO}` (also when `cwd` is set). Rows and cols are 0-based; negative rows count from the bottom (`-1` = last
row).

```yaml
id: F-NAV-01 # spec feature id (required); summary groups by it
title: j scrolls one row
args: [--split] # CLI args
env: {FOO: bar, TERM_PROGRAM: null} # extra env; null removes a var
size: 120x40 # COLSxROWS, default 120x40
fixture: standard # standard | empty | nogit
cwd: src # app working dir, relative to ${REPO}, created if missing; default ${REPO}
tui: true # false: run with pipes, no PTY; only exit/stdout/stderr/file/shim checks
captureStderr: true # TUI only: app stderr goes to a file instead of the PTY; enables expectStderr
startWithoutBarrier: true # TUI only: no startup barrier (see Holding work)
files: # written before start; string = raw, map/list = JSON
  ${CONFIG}/xplain/config.json: {app: {confirmQuit: false}}
shims: [codex] # names only: exit 0, no output
# or rules per name, first `match` (argv prefix) wins:
# shims: {claude: [{match: [mcp, get], exit: 1, stderr: "no"}, {exit: 0, stdout: "ok\n", readStdin: true}]}
# rule fields: match, stdout, stderr, exit, readStdin, block (wait for `release`), passthrough (run the real program)
steps: [...]
```

Text matchers (screen, lines, regions, files, stdout, stderr, OSC 52, results, response text): a plain string means
`contains`; or `{equals, contains, notContains, matches}` (lists allowed; `matches` is a regex, multiline). JSON subset
matchers (`json`, `body`, `headers`, `result`): objects need only the listed keys, arrays match element-wise, plus
operators `{$contains: x}` (substring / some element), `{$matches: re}`, `{$exists: bool}`, `{$len: n}`. An object is
either all operators or none. A `result` given as a text matcher is only used as one when the result is a string.

### Keys

`- keys: "ij<Enter>"` sends one key at a time, each followed by a barrier. Literal text is one key per character.
Tokens: `<Esc> <Enter> <Tab> <S-Tab> <Up> <Down> <Left> <Right> <Home> <End> <PageUp> <PageDown> <Space> <BS> <Del>`,
`<C-x>` (ctrl), `<A-x>` / `<M-x>` (alt = ESC prefix), `<lt>` for a literal `<`. Arrow/Home/End follow the app's
cursor key mode (DECCKM). Unknown tokens are a load error. `wait: frame` follows each key with a frame barrier,
`wait: none` sends the keys without barriers (see [Holding work](#holding-work)).

### Steps

Every step may carry a `note:`. Input and sync (TUI):

```yaml
- keys: 'q'
- paste: "a\r\nb" # these bytes in one write (the app reads them as one chunk), then a barrier; no key notation
- barrier: idle # one barrier: idle (what every step uses) or frame
- keys: y
  wait: frame # idle (default) | frame | none; also on paste
```

Screen (TUI):

```yaml
- expectScreen:
    contains: '[all]' # whole screen (rows joined by \n, right-trimmed)
    notContains: [Loading]
    matches: '^\[all\]'
    line: {row: -1, matches: '^\(1-37/62\) '} # or a list of these
    region: {rows: [2, 10], cols: [0, 40], contains: 'const'} # inclusive
    cursor: {row: 39, col: 0, visible: false} # terminal cursor
- expectCell:
    at: {text: '[all]', nth: 0, offset: 1, row: 0} # or {row: 0, col: 3}; row limits the text search
    fg: '#b58900' # '#rrggbb' (truecolor), palette index (number) or 'default'
    bg: default
    bold: false # also dim italic underline inverse strikethrough chars
- expectGolden: nav-start # e2e/golden/nav-start.txt; temp paths written as ${REPO} etc.
- expectOsc52: {contains: 'mcp add', count: 1} # last decoded OSC 52 payload; count = copies so far
- captureScreen: {row: -1, matches: '-> (?<OUT>\S+\.md) '} # named groups become variables; or var: X (group 1)
```

Files (any scenario):

```yaml
- expectFile: {path: '${STATE}/xplain/mcp.json', json: {token: {$exists: true}}} # exists defaults to true
- expectFile: {path: out.md, exists: false}
- expectFile: {path: '${STATE}/xplain', mode: '700'} # permission bits (octal), file or dir
- expectFile: {glob: 'xplain-review-*.md', capturePath: OUT, contains: '> hi'} # glob: exactly one match
- expectFile: {glob: 'xplain-review-*.md', count: 0} # or an exact count; checks apply to every match
- expectFile: {glob: 'a-*.md', name: {matches: '^a-\d{6}\.md$'}} # basename matcher
- writeFile: {path: README.md, content: "# changed\n", mode: '644'} # content map/list = JSON; append: true
- writeFile: {path: n.txt, lines: {from: 1, to: 10001}} # "1\n2\n...10001\n"; text: 'row {n}' is the line template
- writeFile: {path: fit.txt, size: 1048576, fill: a, lineLength: 1024} # exact size; each line = 1023 fill + \n
- writeFile: {path: b.bin, bytes: {hex: '00ff41'}} # or {base64: ...}
- writeFile: {path: t.txt, content: 'a b c', nulAt: [1]} # NUL at byte offsets
- copyFile: {from: fit.txt, to: huge.txt, mode: '600'}
- mkdir: {path: sub/dir, mode: '755'} # or just a path
- chmod: {path: secret.txt, mode: '000'}
- symlink: {target: README.md, path: link.md} # target stored as is (relative to the link's dir)
- removeFile: src/a.ts # file or dir
```

Globs: `*`, `?`, `[...]` within a path segment, `**` for any depth; dot files only when the segment starts with `.`.

Git (any scenario): runs the real `git` with argv, cwd `${REPO}`, never through a shell:

```yaml
- git: add -A # split on whitespace; '...' and "..." quoting, \ escapes; ; & | < > ` ( ) $( are load errors
- git: [commit, -qm, 'x y'] # exact argv
- git: {args: [commit, -qm, x], code: 0} # expected exit code (default 0); args may be a string too
```

Variables expand per argument after splitting, so a value with spaces stays one argument.

HTTP / MCP (TUI):

```yaml
- http: {tool: annotate, args: {file: README.md, line: 1, text: hi}} # MCP tools/call, Bearer token from mcp.json
  expect: {status: 200, result: {ok: true}}
- http: {method: tools/list} # or {body: <json-rpc>} / {raw: 'not json'}; also headers, auth (false|string), path, httpMethod, port
  expect: {body: {result: {tools: {$len: 5}}}}
- http: {method: initialize, params: {clientInfo: {name: bot}}}
  capture: {SID: headers.mcp-session-id} # paths into {status, headers, body, result, text}
- http: {tool: next_question, args: {wait_seconds: 1}, headers: {mcp-session-id: '${SID}'}}
- http: {body: []}
  expect: {status: 202, text: {equals: ''}} # raw response text: exact-empty body
- http: {raw: 'x', bodyRepeat: 1048577} # raw repeated n times (large bodies)
  expect: {status: 413}
- httpStart: {id: poll, tool: next_question, args: {wait_seconds: 60}} # does not wait
- httpAwait: poll # wait for its response, then barrier
  expect: {result: {status: question}} # result = JSON of result.content[0].text (string if not JSON)
  capture: {THREAD: result.thread_id}
- httpAbort: poll # drop the in-flight request's connection; waits until the app has handled the drop
- expectRefused: '${PORT}' # TCP connect to 127.0.0.1:<port> must be refused; or {port: ...}
- holdPort: {port: '${PORT}'} # the harness listens there (app MCP start fails: port busy); released at scenario end
- holdPort: {var: OTHER} # a free port, held, stored in ${OTHER}
```

`http`/`httpAwait` send a barrier after the response. Every barrier also waits until the app has received every
request the runner sent ([request sync](#app-contract)): after `httpStart` the next barrier (`barrier: idle` or any
`keys`) guarantees the request reached the app (a long poll is waiting), and two `httpStart`s separated by a barrier
arrive in that order. Requests to ports held by `holdPort` are not counted.

Fake CLIs (`shim`, `expectShimCall`: any scenario; `release`: TUI):

```yaml
- shim: {name: claude, match: [mcp, get], exit: 0} # replace the rules of a shim (creates it); one rule inline
- shim: {name: claude, rules: [{match: [mcp, get], exit: 1}, {exit: 0}]}
- shim: {name: codex, remove: true} # the CLI is gone from PATH
- shim: {name: claude, match: [mcp, add], block: true} # matching calls wait until `release`
- release: claude # unblock waiting calls, stop blocking, then an idle barrier
- expectShimCall: {name: claude, argsPrefix: [mcp, add], count: 1} # also args (exact), argsContain, stdin, cwd
- expectShimCall: {name: claude, nth: 1, argsPrefix: [mcp, remove], count: 4} # call #1 (0-based) matches; 4 calls
- expectShimCall: {name: claude, sequence: [[mcp, remove], [mcp, add]]} # argv prefixes, in this order
```

`passthrough: true` rules run the real program with the same argv and stdio (e.g. `git: {passthrough: true, block:
true}` holds every git run of the app until `release`). Every call is logged before it blocks. A blocked call waits
on a local TCP connection to the runner (event driven).

Process:

```yaml
- expectExit: 0 # waits for the process to exit
- expectStdout: {matches: '^usage: '} # tui: false only
- expectStderr: {equals: ''} # tui: false, or TUI with captureStderr: true
```

### Holding work

Idle barriers are not answered while app work is pending, so while a blocked shim holds a CLI run (or the initial git
load) they stay unanswered. Use frame barriers meanwhile: `barrier: frame` or `keys: ...` + `wait: frame` (answered
once the input is handled and the frame is written; pending work ignored), or `wait: none`. `release` then sends an
idle barrier, answered once the held work finished. Barriers are answered in order: a frame barrier behind an
unanswered idle barrier waits for it.

```yaml
startWithoutBarrier: true # the startup barrier would wait for the held initial load
shims: {git: {passthrough: true, block: true}}
steps:
  - barrier: frame
  - expectScreen: {contains: Loading...}
  - release: git
  - expectScreen: {line: {row: 0, contains: '[all]'}}
```

## App contract

What an implementation must do so these tests apply. Only active with `XPLAIN_SYNC=1`; without it nothing changes.

**Launch.** The runner starts `sh -c 'stty -echo -icanon min 1 time 0; printf "\033]7770;ready\007"; exec $XPLAIN_BIN
"$@"'` in a PTY (cols x rows from `size`), cwd `${REPO}` (or `cwd`), env as in [Isolation](#isolation). With
`captureStderr` the exec gets `2><file>`: stderr is a regular file then, not the terminal. Input may arrive before the
app puts the terminal in raw mode; the app sets raw mode itself. Exit code 0 on a normal quit.

**Paths.** Config `$XPLAIN_CONFIG`, else `$XDG_CONFIG_HOME/xplain/config.json`. MCP token file
`$XDG_STATE_HOME/xplain/mcp.json` (`{"token": "..."}`, written when MCP starts), read by the runner for `Bearer` auth.

**`XPLAIN_MCP_PORT`.** MCP listens on `127.0.0.1:<port>`. Unset or empty: 47615. Decimal `0-65535` (`0` = any free
port). Anything else: the app still runs, but starting MCP fails with the error `invalid XPLAIN_MCP_PORT "<value>"
(0-65535)` shown where MCP start errors show (MCP dialog, autostart note).

**Sync barrier.**

- Input bytes `ESC [ 9 9 9 9 ~` (`1b 5b 39 39 39 39 7e`, idle barrier) and `ESC [ 9 9 9 8 ~` (`1b 5b 39 39 39 38 7e`,
  frame barrier) are barriers, never keys, and may arrive at any time, including before the first frame.
- For every barrier the app writes to stdout `ESC ] 7770 ; <kind> ; <n> ; <reqs> ; <done> BEL` (e.g. `1b 5d`
  `7770;idle;3;2;1` `07`): `kind` = `idle` or `frame` as received; `n` = decimal 1-based count of barriers (both
  kinds) received, one reply per barrier, in order; `reqs` = MCP HTTP requests received so far (counted when a request
  arrives: any path, method or status); `done` = those fully handled (response written, or connection gone and the
  handler finished). Both count over the whole process, across MCP server restarts.
- An idle barrier is answered only once (1) all input before it has been handled, (2) no work started by that input or
  at startup is pending: diff loads, file reads/writes, file listing, MCP start/stop, integration CLI runs, reading an
  MCP request body, anything async that changes the screen, and (3) the resulting frame is completely written to
  stdout before the reply (mind render throttling). A startup barrier is answered after the first full frame of the
  loaded diff.
- A frame barrier needs (1) and (3) only; pending work is ignored.
- Timer driven animation (spinners) is not pending work and may keep drawing after the reply; the runner freezes its
  screen at the reply's position in the output stream until it sends more input.
- A lone `ESC` directly before a barrier is the Escape key (the barrier ends escape-sequence ambiguity; no timeout).
- Input after a barrier is handled after that barrier's reply. Barriers still pending when the app exits may go
  unanswered.
- State changes caused by an MCP request must be applied (or be pending work) before its HTTP response is written:
  the runner sends a barrier right after each response. A request still in flight (`httpStart`) is not pending work
  once its body is read, but must have reached its waiting state (e.g. a long poll registered) by the first reply
  that counts it in `reqs`.
- Request sync (runner side): the runner counts requests it sent to the app (`sent`) and those that got a response or
  that it aborted (`finished`). After a reply with `reqs < sent` or `done < finished` it sends another barrier of the
  same kind, until both hold: one barrier round trip per round, no sleeps. The older reply `ESC ] 7770 ; idle ; <n>
BEL` (no counts) is still accepted; then there is no request sync.

**Clipboard.** Copies are written to stdout as OSC 52: `ESC ] 52 ; c ; <base64 utf-8> BEL`.

**Screen.** Checked through xterm emulation (`TERM=xterm-256color`, `COLORTERM=truecolor`): text, cursor, colors
as reported by xterm (`#rrggbb`, palette index or default), attributes. How the frame is drawn does not matter.

## Layout

`run.ts` runner CLI; `lib/scenario.ts` load + strict validation (schemas); `lib/runner.ts` one scenario (isolation,
steps); `lib/session.ts` PTY + xterm + barriers; `lib/keys.ts` key notation; `lib/match.ts` matchers; `lib/shim.mjs`
fake CLI; `fixture.sh` fixture repos; `scenarios/` by area; `golden/` screen snapshots.
