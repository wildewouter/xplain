# xplain spec

Black-box spec. Source of truth for rewrite. Only observable behavior. Strings in backticks verbatim.

## Purpose

Terminal UI. Walk git diff of working tree. Cursor on lines, select text, write comments. Export comments to markdown. Optional local MCP HTTP server: coding agent long-polls comments as questions, answers into UI, adds own notes, signals file changes (UI reloads). Integrations register server with agent CLIs.

## Run

`xplain [flags] [git diff args...]`. Fullscreen on alternate screen (`ESC[?1049h` on start, `ESC[?1049l` on exit). Reads keys from stdin in raw mode; stdin not TTY: keys ignored, UI still renders.

## Contract surface

Everything test may touch:

- argv: flags, subcommand `config path`, passthrough git args.
- env:
  - `XPLAIN_CONFIG`: config file path (below `--config`).
  - `XDG_CONFIG_HOME`: config dir base, default `$HOME/.config`.
  - `XDG_STATE_HOME`: state dir base, default `$HOME/.local/state` (empty value = unset).
  - `HOME`: fallback for both.
  - `PATH`: finds `git`, `claude`, `codex`, `copilot`.
  - `XPLAIN_MCP_PORT`: MCP listen port (Environment below).
  - `XPLAIN_KEYLOG`: key debug log file (Environment below).
  - color depth: terminal color detection (`COLORTERM=truecolor` gives 24-bit SGR). Tests pin `COLORTERM=truecolor`, `TERM=xterm-256color`.
- terminal size: columns (default 80 when unknown), rows (default 24).
- stdin keys. Escape key = an `ESC` that is the last byte of a read (no escape timeout). `ESC` followed by more bytes in the same read is a key sequence or Alt+key. A sequence split across reads (ESC at the end of one read) decodes as Escape then text: known rare edge case. Also accepted: CSI-u and modifyOtherKeys key reports.
- screen: header row, rule row, diff viewport, footer row, overlays.
- stdout escape OSC 52 for clipboard.
- stderr: flag errors, config warnings.
- exit code: 0 on quit, help, `config path`; 1 on flag error.
- files: config file (read, write), export markdown in cwd, `mcp.json` in state dir, repo files (read only).
- git: `git diff`, `git ls-files` run in `--cwd` (or process cwd).
- HTTP: `127.0.0.1:<port>` path `/mcp` (port 47615 or `XPLAIN_MCP_PORT`).
- external CLIs: `claude`, `codex`, `copilot` run by integrations (argv exact below).

## Environment

- `XPLAIN_MCP_PORT`: MCP listen port. Unset or empty: 47615. Decimal `0`-`65535` (`0` = any free port). Other value: app runs; MCP start fails with error `invalid XPLAIN_MCP_PORT "<value>" (0-65535)`, shown where start errors show (F-MCPUI-01 error row, F-MCPUI-04 note). Elsewhere `<port>` (URLs, modal power row, port-busy message, integration `<url>`) = effective port; `0` gives actual bound port.
- `XPLAIN_KEYLOG`: debug aid. Non-empty: file path; each stdin read appends one line `<unix ms> bytes=<hex> items=<decoded keys>` (file created if missing; open failure ignored). Unset or empty: off.

## Messages

Error notes / screens the app words itself: `<action> <target>: <reason>`. Runtime (language, OS library) error texts never shown verbatim; only the error code picks `<reason>`:

| code            | `<reason>`          |
| --------------- | ------------------- |
| ENOENT          | `not found`         |
| EACCES, EPERM   | `permission denied` |
| EISDIR          | `is a directory`    |
| ENOTDIR         | `not a directory`   |
| any other error | `failed`            |

- `failed`: text after it UNSPEC (UNSPEC-42); tests assert prefix up to `failed` only.
- Sites: `cannot run git: <reason>` (F-MODE-04), `cannot open directory <dir>: <reason>` (F-CLI-06), `cannot read <path>: <reason>` (F-BROWSE-01, F-COMMENT-09), `config save failed: <path>: <reason>` (F-CONFIG-05), `export failed: <path>: <reason>` (F-EXPORT-01), `cannot write <path>: <reason>` (F-MCPSRV-01 token file), `cannot listen on 127.0.0.1:<port>: <reason>` (F-MCPUI-03).
- Not this form (quoted elsewhere): git's own stderr (F-MODE-04), port busy (F-MCPUI-03), `invalid XPLAIN_MCP_PORT ...` (Environment), integration messages (F-INTEG-*), config load warnings (F-CONFIG-04).

## Colors

Colors named ANSI (`gray`, `cyan`, `yellow`, `magenta`, `green`, `red`, `greenBright`, `redBright`, `black`, `white`) or 24-bit hex. Syntax highlighting: code highlighted by file extension (ts, tsx, mts, cts, js, jsx, mjs, cjs, json, md, css, html, xml, yml, yaml, sh, bash, zsh, py, go, rs, java, c, h, cpp, rb, sql, toml); token colors not specified. Other extension: plain. Token colors and language detection details: UNSPEC (UNSPEC-31).

Theme chrome per theme (key: use):

- `addBg`/`delBg`: add/del row gutter background.
- `addMark`/`delMark`: `+`/`-` mark color.
- `gutter`: line numbers, context mark.
- `hunk`: hunk header row fg.
- `mode`: header `[mode]` `[full|changes]` `[browse]` chips; help key column; picker `R` status.
- `view`: header `[split|unified]` `[theme]` `[mcp: ..]` chips.
- `file`: header path.
- `adds`/`dels`: header `+N`/`-N`; picker `A`/`D`; error lines.
- `dim`: rule, footer, note rows, split separator, hints.
- `accent`: cursor header tag, help group titles, comment heads, answer dividers.
- `modalBorder`/`modalBg`/`modalFg`: modal and box border, bg, default text.
- `selBg`/`selFg`: selected row in picker, search, config, MCP modal.
- `curBg`: cursor row background.
- `visBg`/`visFg`: visual selection chars.

| key         | solarized | vibrant     | dull    | contrast | colorblind | light   |
| ----------- | --------- | ----------- | ------- | -------- | ---------- | ------- |
| addBg       | #0b3b1f   | #1f4d2b     | #26332a | #005f00  | #12345a    | #d4f0d4 |
| delBg       | #4a1a1f   | #5a1f26     | #382528 | #870000  | #5a3410    | #f8d4d4 |
| addMark     | #859900   | greenBright | #7f9c7f | #ffffff  | #5fafff    | #0a6b1f |
| delMark     | #dc322f   | redBright   | #a87f7f | #ffffff  | #ffaf3f    | #a01010 |
| gutter      | #586e75   | gray        | #5f6368 | #808080  | #808080    | #6a6a6a |
| hunk        | #2aa198   | cyan        | #7f9fa8 | #ffffff  | #5fafff    | #00609c |
| mode        | #b58900   | yellow      | #b39f80 | #ffff00  | #f0c674    | #8a5a00 |
| view        | #6c71c4   | magenta     | #a8899c | #ff00ff  | #b48ead    | #8f1f8f |
| file        | #268bd2   | cyan        | #9aa5b1 | #00ffff  | #56b6f7    | #00609c |
| adds        | #859900   | green       | #7f9c7f | #00ff00  | #5fafff    | #0a6b1f |
| dels        | #dc322f   | red         | #a87f7f | #ff0000  | #ffaf3f    | #a01010 |
| dim         | #586e75   | gray        | #5f6368 | #808080  | #8a8a8a    | #6e6e6e |
| accent      | #cb4b16   | cyan        | #7f9fa8 | #ffff00  | #56b6f7    | #00609c |
| modalBorder | #268bd2   | cyan        | #5f6368 | #ffffff  | #56b6f7    | #303030 |
| modalBg     | #002b36   | black       | #1c1c1c | #000000  | #000000    | #f4f4f4 |
| modalFg     | #93a1a1   | #e4e4e4     | #c0c0c0 | #ffffff  | #e0e0e0    | #202020 |
| selBg       | #073642   | cyan        | #3a3f47 | #ffff00  | #56b6f7    | #bcd8ff |
| selFg       | #93a1a1   | black       | #d0d0d0 | #000000  | #000000    | #101010 |
| curBg       | #22586b   | #33336b     | #3f3f5f | #3a3aa8  | #5a5a5a    | #ffe9a0 |
| visBg       | #6b4f00   | #875f00     | #6b5a2e | #af5f00  | #b8a000    | #7fb2ff |
| visFg       | #fdf6e3   | #ffffff     | #f0f0f0 | #ffffff  | #000000    | #000000 |

Theme-independent: find hits bg `yellow`, fg `black`. Code on add/del rows: no background (only gutter + mark cells colored). Contrast theme: syntax tokens bold.

---

## CLI

### F-CLI-01 help

- Input: `-h` or `--help` anywhere in argv (flags parsed left to right; earlier bad flag fails first).
- Result: stdout = usage text + `\n`, exit 0, no UI.
- Usage text verbatim:

```
usage: xplain [--cwd dir] [--config file] [--mode all|staged|unstaged | --staged | --unstaged] [git diff args...]
  --mode <m>   all (git diff HEAD, default), staged (--cached), unstaged
  --staged     same as --mode staged
  --unstaged   same as --mode unstaged
  --split      start in side-by-side view (s toggles)
  --changes-only  start with git hunks only, not the full file (c toggles)
  --theme <t>  solarized, vibrant, dull, contrast, colorblind, light (first is default) (t cycles)
  --config <f> config file (default $XPLAIN_CONFIG or ~/.config/xplain/config.json)
  -h, --help   show this help
xplain config path  print the resolved config path
extra git args replace HEAD in "all" mode, and are appended in the other modes.
keys: ? help, s split/unified, c full/changes, ]/[ next/prev change, m cycles mode, t cycles theme, C config, q quits
```

### F-CLI-02 flag errors

- Error output: stderr `xplain: <msg>\n<usage>\n`, exit 1, no UI.
- Messages:
  - `--cwd` last arg: `--cwd needs a value`
  - `--config` last arg: `--config needs a value`
  - `--mode` missing value: `invalid mode: (missing)`; bad value `v`: `invalid mode: v`. `--mode=` empty: `invalid mode: ` (trailing space).
  - `--theme` missing: `invalid theme: (missing) (solarized|vibrant|dull|contrast|colorblind|light)`; bad `v`: `invalid theme: v (solarized|vibrant|dull|contrast|colorblind|light)`. `--theme=` empty: `invalid theme:  (solarized|...)` (two spaces).

### F-CLI-03 flags

- `--cwd <dir>`: git commands, file reads, export dir, integration CLIs run there. `<dir>` repo subdir: diff paths repo-root-relative vs browse/search paths cwd-relative: UNSPEC (UNSPEC-9); tests use repo root.
- `--config <f>` / `--config=<f>`: config path. Empty value falls back to env/default.
- `--mode <m>` / `--mode=<m>`: `all|staged|unstaged`. `--staged`, `--unstaged` shortcuts. Last wins.
- `--split`: start split. `--changes-only`: start changes scope.
- `--theme <t>` / `--theme=<t>`.
- Flags override config values (defaults < config < flags). No flag to force unified/full; config decides.
- Any other arg (also unknown `--x`, `--`) collected in order as git diff args. `--no-color` arg: UNSPEC (UNSPEC-10).

### F-CLI-04 config path

- Input: remaining args exactly `config path` (after flags removed), e.g. `xplain --config /x config path`.
- Result: stdout resolved config path + `\n`, exit 0. No config read, no warnings, no UI. File need not exist.
- `config path extra`: not subcommand; args go to git.

### F-CLI-05 start and exit

- Config warnings on stderr before UI.
- UI enters alternate screen. First frame before diff loads: `Loading...` (dim) only. Keys already processed while loading (invisible; e.g. `q` with confirm off quits).
- Quit (F-QUIT-*) or Ctrl+C: leave alternate screen, exit 0. Ctrl+C: immediate, no confirm, any mode (also in modals, text inputs, error screen).

### F-CLI-06 flag value parsing

- `--cwd`, `--config`, `--mode`, `--theme` (space form) take next argv item verbatim, even if it starts with `-`. E.g. `--cwd -h` sets cwd `-h` (no help); `--mode --staged` fails `invalid mode: --staged`.
- `=` form only for `--config=`, `--mode=`, `--theme=`. `--cwd=<d>`, `--split=x`, `--staged=x` etc. not flags: passed to git as args (git then errors, F-MODE-04).
- `--cwd` dir checked before each diff load (git not spawned when bad): error screen (F-MODE-04) `cannot open directory <dir>: <reason>`, `<dir>` as given. Missing: `not found`; regular file: `not a directory`. Dir created later: `r` loads it.

## CONFIG

### F-CONFIG-01 path resolution

- Order: `--config` value, else `$XPLAIN_CONFIG`, else `$XDG_CONFIG_HOME/xplain/config.json`, else `$HOME/.config/xplain/config.json`.
- Empty value (`--config=`, `--config ""`, `XPLAIN_CONFIG=`, `XDG_CONFIG_HOME=`) = unset, next source used.

### F-CONFIG-02 schema and defaults

```
{
	"version": 1,
	"theme": "solarized",
	"view": {"mode": "all", "split": false, "full": true},
	"app": {"confirmQuit": true},
	"mcp": {"autostart": false}
}
```

- `theme`: one of `solarized vibrant dull contrast colorblind light`.
- `view.mode`: `all|staged|unstaged`. `view.split`: bool. `view.full`: bool (true = full file scope).
- `app.confirmQuit`: bool, quit asks confirm.
- `mcp.autostart`: bool, start MCP server on launch.
- `version`, `agent`, `keys`, unknown keys: ignored on read, kept on write.
- Missing file: defaults, no warning.

### F-CONFIG-03 validation warnings

- Each bad value: stderr line `xplain: config: <msg>\n`, that key uses default, rest still applied.
- `<msg>` verbatim (`<v>` = JSON encoding of value, e.g. `"x"`, `1`, `null`):
  - `invalid theme <v> (solarized|vibrant|dull|contrast|colorblind|light); using solarized`
  - `view must be an object; using defaults`
  - `invalid view.mode <v> (all|staged|unstaged); using all`
  - `invalid view.split <v> (boolean); using false`
  - `invalid view.full <v> (boolean); using true`
  - `app must be an object; using defaults`
  - `invalid app.confirmQuit <v> (boolean); using true`
  - `mcp must be an object; using defaults`
  - `invalid mcp.autostart <v> (boolean); using false`
- Arrays count as not object.
- Warning order fixed: theme, view (object, mode, split, full), app, mcp. Key absent: no warning. `view` not object: only that one line, no per-key lines.

### F-CONFIG-04 broken file

- Invalid JSON (also empty file): stderr `xplain: config: <path>: <parser message>; using defaults, file will not be modified`. Contract: prefix `xplain: config: <path>: ` + suffix `; using defaults, file will not be modified`. Parser message UNSPEC (UNSPEC-29).
- Path exists but unreadable (e.g. is a directory): same prefix + suffix, read error text between UNSPEC (UNSPEC-29).
- Top level not object (array, number, null): `xplain: config: <path>: top level must be an object; using defaults, file will not be modified`.
- All defaults used. File never rewritten (save fails, F-CFGUI-03).

### F-CONFIG-05 save

- Trigger: config modal select (F-CFGUI-02).
- Existing file must parse to JSON object, else note `config unreadable, not saved (<path>)`, file untouched.
- Missing file: base `{"version": 1}`. Parent dirs created.
- Patch deep-merged into existing object (other keys kept, unknown keys kept).
- Written as JSON, tab indent, trailing `\n`. Failure: note `config save failed: <path>: <reason>` (Messages; e.g. parent path a regular file: `not a directory`; dir not writable: `permission denied`). Write mechanism (current: temp `<path>.<pid>.tmp` then rename, temp removed on failure): UNSPEC (UNSPEC-36).
- Success: note cleared.
- Patch per setting: `theme` → `{"theme": v}`; mode → `{"view": {"mode": v}}`; split → `{"view": {"split": bool}}`; view → `{"view": {"full": bool}}`; confirm quit → `{"app": {"confirmQuit": bool}}`; mcp on startup → `{"mcp": {"autostart": bool}}`.
- Example fresh file after selecting theme dull:

```
{
	"version": 1,
	"theme": "dull"
}
```

## MODE

Diff source.

### F-MODE-01 git command

- Always `git diff --no-color --no-ext-diff`, plus `-U1000000` when scope full, then base, then extra args.
- Base: `all` no extra args: `HEAD`; `all` with extra args: none (args replace HEAD); `staged`: `--cached`; `unstaged`: none.
- Run in cwd. Timeout: UNSPEC (UNSPEC-37).

### F-MODE-02 untracked files in all mode

- Only mode `all` with no extra args.
- `git ls-files --others --exclude-standard -z`: each regular file (not symlink, not dir) size <= 1048576 bytes shown as fully added file: `git diff --no-index --no-color --no-ext-diff [-U1000000] -- /dev/null <name>`.
- Larger files, symlinks, unreadable: skipped silently.
- Order: tracked diff files first (git order), then untracked in ls-files order.
- Never modifies index or worktree.

### F-MODE-03 cycle mode `m`

- Pre: diff view, not browse.
- `m`: `all` then `staged` then `unstaged` then `all`. File index to 1 at once, diff reloaded. Header `[mode]` chip updates.
- When new diff arrives: shown file path (first file of new diff) + scope + effective layout same as before `m`: cursor kept per F-RELOAD-03; viewport per F-NAV-10. Else position per F-NAV-08.
- Path compared with file shown before `m` press, regardless of its file index (e.g. `[3/4] x.txt`, new diff first file `x.txt`: cursor kept).
- Browse: `m` ignored.

### F-MODE-04 git error screen

- Diff load fails (not git repo, bad args, no HEAD, bad `--cwd`): whole screen replaced by error text (red): git stderr verbatim; bad `--cwd`: `cannot open directory <dir>: <reason>` (F-CLI-06); git cannot be started (e.g. not on `PATH`): `cannot run git: <reason>` (not on `PATH`: `cannot run git: not found`). Text always from the `git diff` command (F-MODE-01), never from the untracked listing (F-MODE-02), deterministic. Outside any repo: `git diff` stderr = git's `git diff --no-index` usage (long text). Header/footer gone. Stderr taller than screen: UNSPEC (UNSPEC-2).
- Keys on error screen: Ctrl+C exits (F-CLI-05); `r` retries load. Other keys: UNSPEC (UNSPEC-1).
- Later successful load restores UI: `r` / agent reload (F-RELOAD-02) succeeding. `r` failing while on error screen: error screen stays.

### F-MODE-05 no changes

- Pre: diff empty, not browsing.
- Header: `[<mode>] [mcp: off] No changes (m cycles mode, F search, q quits)` (mcp chip reflects state). No scope/layout/theme chips.
- Viewport empty; no cursor tag; footer `(0-0/0) hjkl move  enter ask  J/K comments  ? help`, regardless of split setting and cols (never `p pane`; `too narrow for split | ` part per F-LAYOUT-02).
- `m`, `F`, `q`, `M`, `C`, `?`, `E`, `r` work. Tab/S-Tab, arrows, cursor moves, `p`: no-op. `t`, `s`, `c` change state silently (not visible until changes exist). `f` opens empty picker ` Files (1/0)`.
- Enter / `a` (comment editor): UNSPEC (UNSPEC-13).

## EDGE

Diff edge cases. Each file: path, adds, dels, rows.

### F-EDGE-01 paths

- Only git's own `a/`, `b/` prefix stripped (once). Path whose own first dir is `a` or `b` shown in full: repo file `a/x.txt` shows `a/x.txt`, `b/a/y` shows `b/a/y` (header, picker, rename). Deleted file path = old path. Path relative to repo root (as git prints).
- Path with spaces shown exactly: `my file.txt` shows `my file.txt` followed by one space and the counts (header, picker). The TAB git appends after such names in `---`/`+++` lines is not part of the path.

### F-EDGE-02 rename

- Rename (git rename detection): header path `<old> -> <new>`, picker status `R`, picker path `<old> -> <new>`.
- Rename without content change: no hunks, one dim note row `Renamed, no content changes`, `+0 -0`.

### F-EDGE-03 binary

- Binary diff (`Binary files ... differ` or `GIT binary patch`): one dim note row `Binary file`, `+0 -0`, status `M`. Any binary file entry: modified, added, deleted tracked binary, untracked binary (F-MODE-02). Detection per file entry (other entries in same diff never change it).

### F-EDGE-04 no textual changes

- File entry without hunks, not binary, not rename (mode change, empty new file, deleted empty file): note row `No textual changes`. Mode change with content change has hunks: no note.

### F-EDGE-05 status letters (picker)

- `R`: renamed. `A`: has hunks, all hunk headers start `@@ -0,0 `. `D`: has hunks, all hunk headers contain ` +0,0 @@`. Else `M`.
- Untracked text files show `A`. Untracked binary or empty file: no hunks, so `M` (note row `Binary file` / `No textual changes`). Added/deleted binary tracked file: also `M`, note row `Binary file` (never `No textual changes`).

### F-EDGE-06 no-newline marker

- Git line `\ No newline at end of file` becomes extra row of same kind as line before it (del or add), same line number, text ` No newline at end of file` (leading `\` dropped, space kept). Not counted in `+N`/`-N`.
- Example (`a\nb` to `a\nc`, no final newline): rows `   2      - b`, `   2      -  No newline at end of file`, `        2 + c`, `        2 +  No newline at end of file`.

### F-EDGE-07 non-ASCII paths

- Paths git C-quotes (non-ASCII, e.g. `é.txt`): display in header/picker/search and opening in browse UNSPEC (UNSPEC-17). Tests use ASCII paths.

### F-EDGE-08 content chars

- Line text shown raw except tabs (2 spaces). Wide chars (CJK) take 2 cells.
- Column unit for non-ASCII text (cursor `:C<n>`, selection cols, find offsets, horizontal shift): UNSPEC (UNSPEC-28). ASCII: one char = one column.
- ANSI escapes / control chars inside file content: UNSPEC (UNSPEC-26). CR at line end (CRLF files): UNSPEC (UNSPEC-27).

## SCOPE

### F-SCOPE-01 full vs changes

- Full (default): `-U1000000`, whole file as one hunk. Changes: git default context hunks.
- Header chip `[full]` or `[changes]`.

### F-SCOPE-02 toggle `c`

- Pre: diff view, not browse.
- `c`: toggle scope, reload diff, stay on same file path (index of that path, else first file). Cursor + viewport reset per F-NAV-08.
- Browse: ignored.

## LAYOUT

### F-LAYOUT-01 frame

- Rows total = terminal rows R (default 24). Viewport height H = max(3, R-3).
- Row 1: header (truncated to width, F-LAYOUT-07). Row 2: rule `─` x (cols-1), dim. Rows 3..H+2: viewport. Last row: footer (dim, truncated).
- R < 6: layout UNSPEC (UNSPEC-33). Tests use R >= 6.

### F-LAYOUT-02 footer

- Composition, in order:
  1. find open: `/<text>█`; goto open: `:<text>█`.
  2. else: `/<term> | ` when find term active; then `<note> | ` when note set.
  3. `too narrow for split | ` when split setting on and cols < 100. Shown also in browse, no-changes screen, and while find/goto open.
  4. `(<a>-<b>/<n>) ` where n = rows of current view, a = min(n, off+1), b = min(n, off+H), off = top row index.
  5. key hints per state (F-HELP-04).
- No separator between `█` and rest (e.g. `/foo█(1-20/40) hjkl move  enter ask  J/K comments  ? help`; hints F-HELP-04).
- Note: last status message. Shown right after action that set it. Lifetime after later keys/events UNSPEC (UNSPEC-11), except: replaced by next note; cleared by successful config save (F-CONFIG-05).

### F-LAYOUT-03 unified rows

- File without hunks (F-EDGE-02..04): single note row. Else per hunk: hunk header row (full `@@ ... @@ ...` line, fg `hunk`), then line rows. Never note row and hunks together.
- Line row: `OOOO NNNN ` (old no, new no, each right-aligned width 4, blank if none; numbers > 9999 not cut, widen that row), number cells fg `gutter`, mark cell (`+`, `-`, ` `, bold, color addMark/delMark/gutter), cursor cell (`▶` on cursor row, else space), code.
- Example rows: `   1    1   a` (context), `   2      - b` (del), `        2 + B` (add).
- Tabs shown as 2 spaces. Long lines truncated at screen edge (no wrap), last cell `…` (F-LAYOUT-07).
- Add row: number cells + mark cells bg `addBg`; code no bg. Del: `delBg`. Context: no bg.

### F-LAYOUT-04 split `s`

- Pre: diff view, not browse. `s` toggles split setting. Header chip `[split]`/`[unified]` shows setting (cols < 100 with split on: chip UNSPEC, UNSPEC-3).
- Effective only when cols >= 100; else unified rendering + footer `too narrow for split | `.
- Pane width W = floor((cols-1)/2). Left pane: old side; `│` (dim) separator; right pane: new side.
- Pane cell: `NNNN ` (width 4 number + space), mark, cursor cell, code. Each pane truncated at its own width (last cell `…` when cut).
- Pairing: within run of changes, i-th deleted line pairs with i-th added line; surplus on one side leaves other cell empty. Context line appears both sides with own numbers (left old no, right new no). Hunk and note rows full width.
- Browse: `s` ignored.
- cols >= 100: effective layout changes, cursor + viewport per F-NAV-08. cols < 100: rows unchanged, cursor kept; viewport per F-NAV-10.

### F-LAYOUT-05 browse rows

- Single pane: `NNNN ` (line number), mark ` `, cursor cell, code. No hunk row.

### F-LAYOUT-06 modal placement

- Modals (picker, search, config, MCP, delete, quit): centered horizontally, vertically centered. Box w x h: cells left of box = ceil((cols-w)/2), rows above box = ceil((R-h)/2) (odd leftover: extra cell/row before box). E.g. delete modal 25x3 at 80x24: left col 28 (0-based), top row 11 (0-based). With help panel open: top at screen row 3 (horizontal same).
- Help panel drawn over modals.

### F-LAYOUT-07 truncation

- Every single-line text (header, footer, diff rows, modal rows, comment box rows) wider than its box: cut to width-1 cells + `…` (last cell). E.g. header at 60 cols ends `... [1/4] f.txt +…`.
- Cursor row last cell when row text fits: UNSPEC (UNSPEC-16).

### F-LAYOUT-08 narrow modals

- Box narrower than its hint text + 2 (picker at cols < 73, search at cols < 52): layout UNSPEC (UNSPEC-18). Tests use cols >= 80 for modals.

## THEME

### F-THEME-01 cycle `t`

- Pre: diff or browse, no modal. `t`: next theme in order `solarized vibrant dull contrast colorblind light`, wraps. Header `[theme]` chip updates. Not saved.

### F-THEME-02 colors

- Chrome colors per table (Colors section). Test pins add/del gutter bg, cursor row bg, selection bg/fg, header chip fg per theme.

## HEADER

### F-HEADER-01 diff header

- Format: `[<mode>] [<full|changes>] [<split|unified>] [<theme>] [mcp: <on|off>] [<i>/<n>] <cursortag><path> +<adds> -<dels>`.
- Rename: path `<old> -> <new>`.
- Colors: mode + scope chips `mode`; layout, theme, mcp chips `view`; `[i/n]` bold; path `file`; `+N` `adds`; `-N` `dels`.
- `[mcp: on]` only when server running (off while starting).

### F-HEADER-02 browse header

- `[browse] [<theme>] [mcp: <on|off>] <cursortag><path>`.

### F-HEADER-03 cursor tag

- Always shown in diff and browse headers (also at start, no toggle; not on no-changes screen): `[cursor <pos>] ` or `[visual <pos>] ` while selection active. Color `accent`, bold.
- `<pos>`: optional `old `/`new ` pane prefix when split effective (diff), then `L<line>` (row's line number) or `r<row index+1>` (row without number: hunk/note/empty), then `:C<col+1>`.
- Line number: unified: new no, else old no (del row shows old no). Split pane old: left old no (empty left: `r<n>`). Split pane new: right new no, else left old no. Browse: line no.
- Pane prefix = chosen pane (`p`). Del-only split row with pane new (char cursor in left pane): prefix UNSPEC (UNSPEC-20).
- Col = shown (clamped) column, empty line `C1`.
- Example: `[cursor new L12:C3] `, `[visual L4:C1] `, `[cursor r1:C1] `.

## NAV

Diff view. Cursor always on (F-CURSOR-09); viewport follows cursor (F-NAV-09). No plain scroll mode.

Terms: top = index of first shown row (footer `a` = top+1). Row block = row line + its comment boxes. Bottom offset = smallest top where blocks from top to last row fit in H lines (no boxes: max(0, n-H)). Change start = changed (add/del) row whose previous row not changed.

### F-NAV-01 line scroll `j` `k` Down Up

REMOVED: cursor-only app, no viewport-only scroll. See F-CURSOR-03, F-NAV-09.

### F-NAV-02 half page `d` `u`

REMOVED: cursor-only app. See F-CURSOR-03 (`d`/`u` move cursor), F-NAV-09.

### F-NAV-03 page Space PageDown PageUp

REMOVED: cursor-only app. See F-CURSOR-03 (page keys move cursor H-1 rows), F-NAV-09.

### F-NAV-04 top/bottom `g` `G`

REMOVED: cursor-only app. See F-CURSOR-03 (`g`/`G` move cursor), F-NAV-09.

### F-NAV-05 change jump `]` `[`

- Rewritten for cursor (was viewport jump).
- `]`: cursor to first change start row > cursor row. `[`: last change start row < cursor row. None: no move. Count ignored (cleared). Column: desired column kept (F-CURSOR-03).
- Viewport per F-NAV-09.
- Browse: no change starts, no move.

### F-NAV-06 file switch Tab S-Tab

- Tab: next file, wraps. Shift-Tab: previous, wraps. Position per F-NAV-08. Header `[i/n]` updates.
- Left/Right: char cursor moves (F-CURSOR-04), never file switch.
- One file only: same file stays, cursor unchanged; viewport per F-NAV-10.
- Browse: ignored. No files: no-op.

### F-NAV-07 ctrl keys ignored

- Any Ctrl combo (except Ctrl+C exit, Ctrl+N/P in search modal) ignored in diff/browse view and picker (picker: Ctrl+F, Ctrl+Q do not close it, Ctrl+K/D/U do not move). Text inputs: ignored (not typed).
- Config, MCP, confirm modals: Ctrl combos UNSPEC (UNSPEC-25).
- Digits: count prefix in diff and browse (F-CURSOR-05).

### F-NAV-08 initial position per file

- Trigger: shown rows change and (shown file differs, diff<->browse, scope differs, or effective split differs). Covers file switch, picker open, browse open, browse Esc back to diff, `c`, `s` (cols >= 100), numbered jump to other file, `m` landing on other path.
- Then: cursor to first change row when scope full + changes exist, else first row; col 1; pane new; selection cleared; count cleared. firstChange = first change start row (NAV terms).
- Find term (F-FIND-01) kept across all of these (file switch, browse, scope, layout, mode, reload): highlights apply to new rows, `n`/`N` keep working.
- Viewport: top = max(0, firstChange-3) (clamped to bottom offset) when scope full + changes exist, else top 0; then follow (F-NAV-09).
- Same file + scope + layout, rows changed (reload, `m` back to same path): F-RELOAD-03 instead.
- Rows unchanged (Tab with one file, picker Enter on current file): cursor untouched; viewport per F-NAV-10.
- Example: 40-line file, only line 30 changed, R=8 (H=5, margin 2): after browse Esc cursor on del row of line 30, footer `(29-33/42)`. R=24: footer `(22-42/42)`.

### F-NAV-09 viewport follow

- Only cursor moves / placement change top (plus F-RELOAD-03 and F-NAV-10 cases). No key scrolls viewport without moving cursor.
- Margin m = min(2, floor((H-1)/2)) rows. Editor open: m = 0, editor box counted under cursor row.
- After cursor change: cursor row < top+m: top = max(0, cursor-m). Then while row blocks from top through min(last, cursor+m) (comment boxes incl.) need > H lines and top < cursor: top+1. Then top clamped to bottom offset.
- Row blocks never cut at top (whole comment boxes, no partial block at viewport top).
- Cursor in view with margin: top unchanged.
- Example: 62-row file, R=24 (H=21), cursor row 1, `20j`: cursor row 21, footer `(3-23/62)`; then `19k`: cursor row 2, footer `(1-21/62)`.

### F-NAV-10 top reset keeps cursor

- Cursor row always visible. Actions that reset top without moving cursor (rows unchanged or cursor kept): `m` / config mode select landing on same path (F-MODE-03, any file index before), Tab/S-Tab with one file (F-NAV-06), picker Enter on current file (F-FILES-02), `s` at cols < 100 (F-LAYOUT-04), config select of current view/split value (F-CFGUI-02).
- Cursor unchanged (row, col, pane). Top set to 0, then follow (F-NAV-09). So (no comment boxes): cursor row (1-based) <= H-m: top 0; else cursor lands m rows above viewport bottom.
- `m` / config mode: top reset + follow on key press (old rows); when new diff arrives, F-RELOAD-03 (cursor mapped, top kept clamped, follow).
- Example: one file (62 rows), R=12 (H=9, m 2), cursor row 52 (`L50`), footer `(50-58/62)`; Tab or picker Enter on it: cursor stays `L50`, footer `(46-54/62)`.

## FILES

### F-FILES-01 picker open `f`

- Pre: diff view (not browse), no modal. `f` opens picker, selection = current file.
- Box: round border, width min(cols, max(20, floor(cols*0.7))), height min(R, max(5, min(files+4, floor(R*0.6)))).
- Title ` Files (<sel+1>/<n>)` bold.
- Row: `>` if selected else space, status letter (colors: A adds, D dels, R mode, M accent), space, `<old> -> ` if rename, path, ` +<adds>` (adds color), ` -<dels>` (dels color), ` *` if current file. Selected row bg selBg fg selFg for all its cells (status letter, `+N`, `-N` too: own colors not used on selected row).
- List window: height-4 rows, centered on selection, clamped to ends.
- Hint row: ` j/k/↑↓ move d/u half page enter open esc/q close` (dim).

### F-FILES-02 picker keys

- `j`/Down, `k`/Up: move 1, clamped. `d`/`u`: move floor(H/2) (viewport half), clamped.
- Enter: show selected file, position per F-NAV-08 (current file: F-NAV-10), close.
- Esc, `q`, `f`: close, no change.
- Ctrl keys (incl. Ctrl+F, Ctrl+Q: picker stays open), other keys: ignored. `?`: help toggle.
- Empty list (no-changes screen): title ` Files (1/0)`, no rows. Esc closes. Other keys (`j`/`d`/Enter): UNSPEC (UNSPEC-19).

## SEARCH

File search + open in browse.

### F-SEARCH-01 open `F`

- Pre: no modal/text input. Works in diff, browse, no-changes screen.
- Lists `git ls-files --cached --others --exclude-standard` (cwd), unique, sorted. Loaded async; empty until done; list error: stays empty.
- Box width like picker; height min(R, max(8, floor(R*0.7))).
- Title ` Search (<sel+1>/<hits>)`, `(0/0)` when no hits. Row 2: ` > <query>` (`> ` accent) then inverse space caret.
- Paths as `git ls-files` prints them (non-ASCII: UNSPEC-17).
- Hits: empty query = all files sorted. Else fzf-style: hit set = exactly the paths containing all query chars in order (subsequence, gaps allowed).
- Smart case: query all lowercase: case-insensitive; query with any uppercase char: case-sensitive (e.g. `rm` hits `README.md` and `readme.md`; `RM` hits `README.md` only; `rM` hits neither).
- No extended syntax: every query char literal, incl. space, `'`, `^`, `$`, `!`, `|` (e.g. query `a b` hits only paths with `a`, then space, then `b`).
- Path equal to query (same case rule) = first hit. Order of other hits: UNSPEC (UNSPEC-30; rewrite may use fzf/nucleo-like scoring).
- Matched chars (one placement per hit; which placement: UNSPEC-30) bold; on non-selected rows fg `accent`, other chars `modalFg`. Row prefix `> ` selected / two spaces. Selected row bg selBg, unmatched chars fg selFg; matched chars fg on selected row: UNSPEC (UNSPEC-30).
- Window height-5 rows, centered on selection.
- Hint ` ↑↓/^n^p move enter open esc close`.

### F-SEARCH-02 keys

- Printable chars (incl. `j`, `k`, `q`, `?`, space): append to query, selection first. Pasted newlines: UNSPEC (UNSPEC-8).
- Backspace/Delete: delete last char, selection first. Left/Right: nothing.
- Down / Ctrl+N: next hit (clamped). Up / Ctrl+P: previous.
- Enter: open selected hit in browse (F-BROWSE-01), close. No hits: nothing.
- Esc: close.

## BROWSE

Read-only file viewer.

### F-BROWSE-01 open

- File read `<cwd>/<path>`. First 8000 bytes contain NUL: content `binary file, not shown`.
- Trailing single newline dropped; lines split on `\n`; empty file = one empty row. Line numbers 1..n.
- Header F-HEADER-02 (cursor tag always). Cursor row 1, col 1, top 0.
- Read error: note `cannot read <path>: <reason>` (Messages; e.g. `cannot read a.txt: not found`; `<path>` = `<cwd>/<p>` with `--cwd`, else `<p>` as given), browse not opened.
- Only one trailing `\n` dropped: file `a\n\n` = rows `a`, empty.

### F-BROWSE-02 keys

- Esc (no copy button picked, no focused comment, no selection; F-CURSOR-10): back to diff view, same diff file, position per F-NAV-08. No cursor-mode exit step first.
- Ignored: `n` (without find term), `f`, `p`, `c`, `s`, `m`, Tab, S-Tab.
- Work: cursor keys as in diff (F-CURSOR-02..05, single pane, no `p`; `[`/`]` no move), visual, comments, `t`, `C`, `M`, `E`, `F`, `/` `n` `N`, `:`, `r`, `(` `)`, `q`, `?`.
- Comments on browse lines: file = browse path.

## FIND

In-view text search.

### F-FIND-01 input `/`

- Pre: no modal/text input. Opens find line; footer `/<text>█`; count prefix dropped.
- Printable chars appended (each run of CR/LF in paste becomes one space). Backspace/Delete deletes last. Ctrl/Meta/Tab/arrows ignored. `?` typed.
- While open: footer `/<text>█` replaces term + note parts; rest of footer stays.
- Esc: close, term unchanged.
- Enter: close, term = text. Empty text: term cleared (highlights gone), no move. Non-empty text with no match: term still set (footer `/<text> | `), note `pattern not found: <text>`.

### F-FIND-02 matching

- Smartcase: term all lowercase: case-insensitive; else case-sensitive. Plain substring (no regex). Occurrences non-overlapping, scanned left to right (`aa` in `aaa`: one hit at col 1).
- Row matches if any code text matches (split pair: both sides). Hunk/note rows never match. Tabs as 2 spaces.
- All occurrences in visible rows highlighted bg `yellow` fg `black` while term active. Footer shows `/<term> | `.
- Cursor row: hit cells keep bg `yellow` fg `black` (over curBg). Precedence per cell: char cursor (reverse, F-CURSOR-02; no hit colors) > selection (visBg/visFg) > hit > normal.

### F-FIND-03 jump

- Enter: first match row >= cursor row; none after: wrap to first match. No match anywhere: note `pattern not found: <text>`.
- `n`: first match row > cursor row, wrap. `N`: last match row < cursor row, wrap. No match: note `pattern not found: <term>`. Only when term set (else `n`/`N` do nothing).
- Jump: cursor to row, column = first hit in cursor pane text (else first hit in row texts, else 1), selection cleared. Viewport F-NAV-09.
- Successful jump sets no note (earlier note: UNSPEC-11).
- `n`/`N` also work in browse, focused comment.

## GOTO

### F-GOTO-01 input `:`

- Opens goto line; footer `:<text>█`. Chars appended (newlines removed, any char accepted). Backspace/Delete. Esc cancels. Enter closes and, if text non-empty, jumps with trimmed text (spaces only: note `not a line number: ` with empty text).

### F-GOTO-02 jump

- Text not `^\d+$` or value < 1: note `not a line number: <text>`.
- Target by new-side line numbers of rows (unified: new no; split: right cell new no; browse: line no). Deleted-only rows ignored.
- No numbered row: note `no lines in view`.
- Scope full or browse and N > max number: note `line <N> out of range (1-<max>)`, no jump.
- Nearest row (smallest |no-N|, first on tie). Not exact: note `line <N> not in view, nearest L<no>`, still jumps. Changes scope with N > max: nearest note, no range error.
- Exact hit: sets no note (earlier note: UNSPEC-11).
- Jump: comment unfocused, pane new, cursor to row, col 1, selection cleared. Viewport F-NAV-09.

## CURSOR

### F-CURSOR-01 toggle `i`

REMOVED: no cursor mode toggle; cursor always on (F-CURSOR-09), `i` unbound (F-CURSOR-08), Esc chain F-CURSOR-10.

### F-CURSOR-02 render

- Cursor row bg `curBg` full width (number cells, mark cells, code, padding); last cell UNSPEC-16. Cursor cell `▶`. Add/del mark still shown. Example: `   2      -▶b` (cursor on del row).
- Char cursor: char at column in reverse video (empty line: reverse space). Hunk/note rows: row bg only, no char cursor (column still tracked, shown in header tag).
- Split: both panes of row get bg; `▶` in both non-empty panes; char cursor only in active pane. Empty active cell: `      ▶` + reverse space.
- Footer hints F-HELP-04.

### F-CURSOR-03 vertical moves

- `j`/Down, `k`/Up: n rows (count). `d`/`u`: floor(H/2)*n. Space/PageDown, PageUp: (H-1)*n rows (count).
- `g`: row 1 (count ignored). `G`: last row. `nG`: row n (1-based index over all shown rows incl hunk/note rows, not line number), clamped to last row. E.g. file with hunk row first: `5G` = `L4`.
- `]`/`[`: F-NAV-05.
- Clamped to rows. Column remembered (desired column kept across rows, shown clamped to row length-1).
- Viewport F-NAV-09.

### F-CURSOR-04 horizontal moves

- `h`/Left, `l`/Right: n chars, clamped to 0..len-1. `0`: col 1 (when no count pending). `^`: first non-blank (all blank: col 1). `$`: last char, sticky on vertical moves (until next horizontal move).
- Column unit: char of tab-expanded text (ASCII); non-ASCII UNSPEC (UNSPEC-28).
- `w` `b` `e` (count): vim word motions. Char classes: whitespace, word (`[A-Za-z0-9_]`), other. `w` next word start, crosses rows, stops at empty row, stays at last char of last row. `b` previous word start, crosses rows, stops at row 1 col 1 or empty row. `e` end of word, crosses rows.
- Text used: cursor pane text, tabs as 2 spaces. Hunk row text = header text.

### F-CURSOR-05 count prefix

- Digits `1`-`9` start count; `0` extends count when pending. Max 99999 (more digits keep 99999). Count applies to next key only, then cleared (also by keys that ignore it). Used by `h l j k d u w b e G`, Space, arrows, PageDown, PageUp. Not shown anywhere.
- Examples: `5j` down 5 rows; `3l` right 3; `10j` down 10.
- Count accepted in diff and browse (no mode needed), also while comment focused.
- Any key other than a count digit clears count, whether it uses it, ignores it or is a no-op: e.g. `i`, `]`, `[`, Esc, `t`, `J`/`K`, `/`, `:`, `(`, `)`, `?`, `F`, `f`, `C`, `M`, `E`, `r`, `n`/`N` (term set or not), `q`, Ctrl combos, keys ignored in browse. Also cleared by editor open, file/rows change.
- So count never reaches a later key: `3?j`, `3rj`, `3M<Esc>j`, `3<C-x>j` each move 1 row. Keys inside modals/inputs never use count.

### F-CURSOR-06 pane `p`

- Pre: split effective, diff. `p`: toggle pane old/new; ends selection. Header tag shows pane.
- Row with only deleted line: char cursor in left pane (header tag: UNSPEC-20).
- Unified/browse: pane always new; `p` no-op. Split setting on but cols < 100: same as unified.

### F-CURSOR-07 horizontal follow

- Vertical follow: F-NAV-09.
- Horizontal: code shifted so char cursor visible with margin min(4, floor((cw-1)/2)); cw = code width (unified cols-12, browse cols-7, split floor((cols-1)/2)-7). Shift applies to code of all line rows (both panes); number cells fixed; hunk/note rows never shifted. Start: shift 0.
- Shift rule: shift s kept while s+m <= col <= s+cw-1-m; col < s+m: s = max(0, col-m); col > s+cw-1-m: s = col-cw+1+m. E.g. cols 80 (cw 68, m 4), `$` on 100-char line: col 99, s = 36, cursor at screen column 12+99-36+1 = 76.

### F-CURSOR-08 global keys

- Rewritten (was keys falling through in cursor mode). Diff view keys beside cursor keys: Tab/S-Tab (F-NAV-06), `s` `c` `m` `f` `t` `C` `q` `M` `E` `F` `/` `n` `N` `:` `r` `(` `)` `?`, each per own feature.
- `i`: unbound, no-op (also in visual, focused comment, browse).

### F-CURSOR-09 always on, start position

- No plain scroll mode. Cursor on from first frame in diff and browse; cannot be turned off.
- Start (first load): cursor per F-NAV-08: first change row of first file when scope full + changes exist, else first row; col 1; pane new. Viewport per F-NAV-08 + F-NAV-09.
- Example: first file `# Title\nhello` to `# Title\nhello world\nmore`, full scope, unified, 80x24: rows `@@ -1,2 +1,3 @@`, `   1    1   # Title`, `   2      -▶hello`, ...; header `... [1/<n>] [cursor L2:C1] <path> ...`; footer `(1-5/5) hjkl move  enter ask  J/K comments  ? help`.

### F-CURSOR-10 Esc chain

- Esc, one step per press, first that applies: unpick copy button (F-ASK-08); unfocus comment; end selection; browse: back to diff (F-BROWSE-02); else no-op.
- Esc never exits cursor mode; with nothing to undo in diff view: no-op (no state change, no note).
- Modals, find, goto, editor: own Esc handling (F-FIND-01, F-GOTO-01, F-COMMENT-02, dialogs), before this chain. Help panel open: Esc not consumed by panel (F-HELP-01), chain applies.

## VISUAL

### F-VISUAL-01 start/end

- `v`: char selection anchored at cursor. `V`: line selection. Same key again: end. Other key while active: switch kind, anchor kept.
- Esc ends (F-CURSOR-10). `p` ends. `i` no-op, selection kept. Moves extend selection (all F-CURSOR motions).
- Also ended by: `J`/`K` focus, `(`/`)` jump, find jump, goto jump, comment submit, file/rows change (F-NAV-08).
- Header tag `[visual ...]`. Footer visual hints.

### F-VISUAL-02 render

- Char: chars from anchor to cursor inclusive (multi-row: first row from anchor col, middle rows full, last row to cursor col). Line: whole text of each row. Empty row in range: one highlighted cell.
- Selected cells bg `visBg` fg `visFg`; cursor char inside selection reverse with same colors.
- Split: selection only in active pane.

### F-VISUAL-03 comment on selection

- Enter or `a`: editor with selection header (F-COMMENT-01).
- Selection tag: labels `L<no>` or `r<row+1>` for start/end row (label rule F-HEADER-03). Line mode: `L5` (same label) or `L5-9` (end label minus first char, so `r1` to `L3` = `r1-3`). Char mode: `L5:C3-C7` (same label) or `L5:C3-L9:C2`.
- Same label also when both rows share number (e.g. del + add row both `L2` in unified): `L2` / `L2:C1-C3`.

## COMMENT

In-memory comments. Lost on exit.

### F-COMMENT-01 editor open

- No focused comment: Enter or `a` opens editor box under cursor row (below its comment boxes).
- Box: round border `modalBorder`, bg `modalBg`, width max(10, cols-1).
- Optional head (accent): selection `selection <tag>`, edit `edit <comment head>`, follow-up `follow-up`; then up to 5 quoted lines ` > <text>` (dim), then ` … +<k> more` if more. New comment on line: no head.
- Input row: ` <text>` with reverse-video caret cell (char under caret, or space at end); long text: window of width-4 chars, start = max(0, caret-(width-4)+1).
- Box height: 4 (borders, input, hint) + head/quote rows. Text default color `modalFg`.
- Hint row: new comment: first fitting of `[<save|ask>] enter send  tab save/ask  esc cancel`, `enter send  tab save/ask  esc cancel`, `enter send  esc cancel`, `enter send` (fit width-3). Edit/follow-up: `enter send  esc cancel` or `enter send`.

### F-COMMENT-02 editor keys

- Printable: insert at caret (each CR/LF run in paste becomes one space). Left/Right: caret. Backspace/Delete: delete before caret. Ctrl/Meta ignored. Up/Down/Home/End: nothing. `?` typed.
- Editor open: cursor row kept visible with editor box under it (follow margin 0).
- Esc: close, discard (edit/follow-up cancelled).
- Tab (new comment only): mode ask to save; save to ask when MCP running; MCP off: note `MCP is off (M to start)`. Mode default: `ask` when MCP running, else `save`; chosen mode reset when MCP stops.
- Enter: trimmed text empty: nothing. Else submit (F-COMMENT-03 / F-COMMENT-07 / F-ASK-03).

### F-COMMENT-03 submit new comment

- Records: file, cursor row, pane, line number, line text or selected text, message, selection lines/cols, context ±3 rows, wide ±15 rows.
- Line text = cursor pane text raw (tabs not expanded); hunk/note row: its text. Selected text = rows' pane texts (tabs expanded), char mode cut at cols, joined `\n`.
- Context rows: line rows only (no hunk/note) from first selected row-3 to last+3, pane text raw. Line mode endCol = max(1, len of last row); startCol 1.
- Box anchored under last selected row (or cursor row). Selection cleared, editor closed.
- Head: `selection <tag>`, or `line L<no>`, or `line r<row+1>`.
- Note: save mode: `question saved (<k>)`, k = total comments incl. agent notes. Ask mode (only possible while MCP running, F-COMMENT-02): sent to MCP (F-ASK-01), note `question sent to agent`. Ask mode with MCP not running: unreachable, UNSPEC (UNSPEC-38).
- Pane side recorded = cursor pane (unified, browse: always `new`). Comment on unified del row (or split del-only row, pane new): side `new`, line = old line number, deleted flag set (anchoring F-COMMENT-05); agent context says `Side: new (after the change)` with no deleted marker (F-MCPSRV-06), export `new side` (F-EXPORT-02).

### F-COMMENT-04 comment box render

- Under anchor row, full width max(10, cols-1). Border round (`╭─╮│╰╯`) dim; focused: heavy border `┏━┓┃┗┛`, bold, accent.
- Head row: ` sent  <head>` (focused ` ▸ sent  <head>`, bold), accent. Suffix dim `  saved · not asked` when human comment, one turn, no answer. Focused + scrollable suffix `  ↕ <from>-<to>/<total>`.
- Quoted selection lines: up to 3 ` > <line>` + ` … +<k> more`.
- Body: thread (F-ASK-05). Unfocused: max 14 body lines then ` … +<k> more`. Focused: body window = max(1, H - 1 - (4 + quoted rows) - heights of other boxes on same row - follow-up editor rows) lines.
- Box height: 2 borders + head + quoted rows + body rows (+1 more-row) (+1 hint when focused).
- Focused hint row: first fitting (width-3) of `e edit  D delete<a><sc><cb>  esc back`, `e edit  D delete<a>  esc back`, `e edit  D delete<a>`, `e edit  D delete`, `e edit`; `<a>` = `  a follow up` (can follow) or `  a ask` (can ask) or empty; `<sc>` = `  j/k scroll` when overflow; `<cb>` = `  ↑/↓ code` when code blocks.

### F-COMMENT-05 anchoring

- Box shown on row matching file + line number + deleted flag + pane side. Survives split/unified, scope toggle, reload if line still present. Line absent: box hidden (comment kept).
- Match rules: pane old: split row whose left old no = n; unified del/context row with old no = n. Pane new, deleted flag: unified del row old no = n; split row with empty right side and left old no = n. Pane new, not deleted: row with new no = n (split: right side).
- Comment on unified del row whose deleted line pairs with added line in split: visibility in split UNSPEC (UNSPEC-24); back in unified.
- Row without line number (hunk/note): anchored by row index.
- Only boxes of shown file visible (browse: browse path). Several boxes on one row: creation order.

### F-COMMENT-06 focus `J` `K`

- File comments ordered by row then creation. `J`: next (unfocused: first); `K`: previous (unfocused: last). Clamped, no wrap. Cursor moves to comment row, selection ends.
- No comments in file: note `no comments`.
- Motion keys (`h j k l w b v V d u g G 0 $ ^ [ ] p` Space, arrows, PageUp/Down) unfocus then act. Exceptions: thread scroll (F-ASK-07), code pick (F-ASK-08). Not motion while focused: `e`/Enter edit, `a` ask, `A`, `D`.
- Digits while focused: count starts, focus kept.

### F-COMMENT-07 edit `e` / Enter

- Focused comment: editor prefilled, caret end, head `edit <head>` + its quoted lines.
- Thread with follow-ups: note `can't edit after follow-ups`.
- Enter: message replaced, note `comment updated`. Agent note with single turn editable too (changes note text). Comment with answer (single turn): editable, answer kept.

### F-COMMENT-08 delete `D`

- Focused: modal `Delete comment? (y/n)` (round border, paddingX 1).
- `y`/Enter: remove, focus next comment in file (none: unfocus, cursor stays), note `comment deleted`. `n`/Esc: close. Other keys ignored; `?` toggles help.

### F-COMMENT-09 numbered jump `)` `(`

- Numbered comments = agent notes with `number` (F-MCPSRV-08). Sorted by number. Any file.
- `)` next, `(` previous, wrapping. Start from focused numbered comment, else last jumped, else first (`)`) / last (`(`).
- Target file in diff list: switch to it (leave browse). Else open file in browse (file unchanged or not in diff).
- Focus target, cursor on its row. Anchor row missing (line not in shown rows): UNSPEC (UNSPEC-14).
- None: note `no numbered comments`. Browse open error: note `cannot read <path>: <reason>` (F-BROWSE-01).

### F-COMMENT-10 agent note render

- Head `#<number> agent note L<line>` or `agent note L<line>`. Never `saved · not asked`. Body = note text (markdown code blocks as F-ASK-06).

## ASK

Comment to agent via MCP.

### F-ASK-01 enqueue

- Comment sent as question: thread id = comment id (`q1`, `q2`, ... in creation order, agent notes also take ids), turn 1, message, context (F-MCPSRV-06). Status `pending`.

### F-ASK-02 ask focused `a`

- Focused comment:
  - pending/streaming answer: note `still waiting for the agent`.
  - answer done, or agent note that can take reply: open follow-up editor.
  - agent note otherwise: note `can't reply to this note yet` (no black-box trigger: UNSPEC-39).
  - human comment with follow-ups, or answer not error/cancelled: note `can't retry a follow-up yet`.
  - askable (human, one turn, no answer or error/cancelled): enqueue; note `question queued`, MCP off: `MCP is off (M to start)`.
- Answer status `error`: no black-box trigger (UNSPEC-40); tests use no answer / `cancelled`.

### F-ASK-03 follow-up

- Follow-up editor Enter: MCP running and thread can take follow-up: new turn queued, note `follow-up queued`, editor closes, thread follows bottom. MCP not running: editor stays, note `MCP is off (M to start)`. Running but thread refuses: note `can't follow up yet`, editor stays (no black-box trigger: UNSPEC-39).
- Can take follow-up: last turn answered and not live; agent note with no reply yet.

### F-ASK-04 ask all `A`

- Focused comment present. Askable comments of current file.
- None: note `nothing to ask`. MCP off: `MCP is off (M to start)`. Else enqueue all, note `queued <k> question` / `queued <k> questions`.

### F-ASK-05 thread body

- Turn 1 message (wrapped to box width-3), then answers of turn 1 (earlier kept answers first), then per follow-up: `follow-up: <message>` lines (accent), its answers.
- Answer divider: `─ answer · <agent> · <status> ` padded with `─` to width-2; accent. `<agent>` = MCP client name from initialize (not `unknown`), else `agent`. `<status>`: `waiting…`, `streaming…`, `done`, `cancelled` (`error` status, its dels color and text: UNSPEC-40).
- Pending, no text: line ` ⠿ waiting for agent…`. Streaming (delivered to agent), no text: ` <spinner> agent working…`, spinner frames `⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏` every 80 ms.
- Done: answer text wrapped. Cancelled by MCP stop: text `MCP stopped`.
- Wrap: break at last space within width, else hard cut; blank lines kept; trailing blank lines dropped; tabs as 2 spaces; CR removed. Wrapped pieces: trailing spaces of line and leading spaces of continuation trimmed.
- Follow-up line wraps as one text incl. `follow-up: ` prefix.

### F-ASK-06 code blocks

- Fenced block (line starting with ``` or ~~~, 3+, optional lang) in message or answer: fences hidden; button line ` [ copy ]` (accent) + dim ` <lang>`; code lines `│` (`│ ` dim) + code; code highlighted by fence lang (`ts`, `python`, `shell`, `console`, `golang`, `c++`, `yml`, extension names, any highlight.js language name; unknown: plain).
- Open fence: optional leading whitespace, lang = first word after fence (e.g. ` ```ts title ` gives `ts`). Close: same char, length >= open, only whitespace around. No close: block runs to end of text.
- Long code line (box width-5 chars max per line): split into consecutive `│` lines of that width (not wrapped at spaces, not cut off). Empty code line: one empty `│` line. Tabs 2 spaces.
- Not in answers while pending/streaming/error (plain wrapped text). Blocks numbered across whole thread (turn 1 message, answers, later answers).

### F-ASK-07 thread scroll

- Focused box whose body exceeds window: `j`/`k` ±1, `d`/`u` ±floor(window/2), `g` top, `G` bottom, clamped.
- Window start kept per thread (initially top). Follow flag per thread: on initially, after `G`, after follow-up submit (F-ASK-03); off when user moves window up (`k`, `u`, `g`, code pick scrolling up). Live answer (pending/streaming) with follow on: window at bottom.
- Answer arrival (F-ASK-09, incl. live answer turning `done`) on focused box: window showed last body line just before (at bottom, e.g. follow on while live, or scrolled back down to end) → after arrival window ends at new last body line (bottom kept, never jumps to top). Else window start kept (clamped).

### F-ASK-08 code copy

- Focused box with code blocks: Down/Up selects next/prev button (wrap; first press Down: first, Up: last), scrolls into view. Selected button reverse bold + dim `  enter copy  esc cancel`; its gutter `│` accent.
- Enter: copy block raw text (lines joined `\n`, tabs kept, no trailing newline) via OSC 52 (`ESC ] 52 ; c ; <base64 utf8> BEL`), note `copied <k> line` / `copied <k> lines` (k = line count; empty block: 1).
- Follow-up lines are never code blocks (plain wrap).
- Esc: unpick.

### F-ASK-09 answer arrival

- Agent `answer` on thread: latest delivered turn gets status `done` + text. Second answer on same done turn: previous kept, new appended (both shown).
- Comment deleted before answer: UNSPEC (UNSPEC-7).

## EXPORT

### F-EXPORT-01 `E`

- No comments: note `no comments to export`.
- Writes `<abs cwd>/xplain-review-YYYYMMDD-HHMMSS.md` (local time). Note `exported <k> comment -> <path>` / `exported <k> comments -> <path>`. Write error: note `export failed: <path>: <reason>` (Messages; e.g. cwd not writable: `permission denied`).
- Includes agent notes.

### F-EXPORT-02 markdown format

````
# xplain review

- repo: `<abs cwd>`
- diff: `<mode>[ <git args...>]`
- date: <ISO 8601 UTC, ms, Z>
- comments: <k>

## `<file>`

### <n>. <where>

- origin: <human|agent>
- state: <saved|answered|pending|streaming|error|cancelled>

Line:            (or "Selected text:" for selection; block omitted when text empty)

```
<text>
```

Context:         (omitted when none)

```
<context lines>
```

**Comment:**     (agent note: **Note:**)

> <message lines>

**Answer (<agent>) - <status>**     (" (<agent>)" omitted when unknown)

> <answer lines>

**Follow-up 1:**

> <message>
...
````

- Files sorted by path (code-unit order). Comments in file sorted numerically by start line, else line, else row index (mixed keys compared as numbers); ties keep creation order. n numbers across whole doc from 1.
- Agent note: `- origin: agent`, no Line block (text empty), no Context, `**Note:**` + note text; `<where>` = `<side> side, line <l>`.
- Line block text: human comment line text raw (tabs kept) or selected text.
- Cancelled answer (agent unknown): `**Answer - cancelled**` + `> MCP stopped`. Error answers (and any `Error:` line): UNSPEC-40.
- `<where>`: `<side> side, selection line <a>` or `<side> side, selection lines <a>-<b>`, always + `, cols <c1>-<c2>`; else `<side> side, line <l>`; else `<side> side, row <row+1>`. side `new`/`old`.
- cols: char mode `<start col>-<end col>` (1-based, inclusive, as tag F-VISUAL-03). Line mode: `1-<m>`, m = max(1, length of last selected row's pane text, tabs as 2 spaces). E.g. `V` on `  foo` (5 chars) row L3: `new side, selection line 3, cols 1-5`.
- state: no answer `saved`; latest answer done `answered`; else its status (`pending`, `streaming`, `cancelled`; `error` UNSPEC-40).
- Quote: each line `> x`, empty line `>`.
- Code fence: backticks, length max(3, longest backtick run in content+1). Inline code: backticks count = fence-2; content padded with spaces when it starts/ends with backtick.
- Runs of 3+ newlines collapsed to 2. Ends with single `\n`.

## HELP

### F-HELP-01 panel `?`

- `?` cycles: closed, level 1, level 2 (only when context has motion keys), closed. Works in all states except text inputs (editor, find, goto, search: `?` typed). Other keys pass through to app; panel updates to new context live. Esc does not close. Opening a text input keeps panel open (switches to that context).
- Contexts with level 2: Diff view, Focused comment, File picker, MCP, Config, File viewer. Without: Visual selection, Editor, Find in file, Go to line, File search, Confirm.
- Group order = order of first item; L2 puts [Move] first where listed.
- Level 2 open and context switches to one without level 2: panel shows that context's L1 entries, hint ` ? close`; next `?` closes. Context with level 2 again while still level 2: L2 shown.
- Placement: bottom-right; bottom edge on row above footer (row R-1); never above screen row 3. Round border: border cells fg `modalBorder`, bg terminal default; inner cells bg `modalBg`, text `modalFg` unless stated.
- Width: box width = min(W, N+2), W = min(96, max(36, floor(cols*0.6))). N (content width) = max(2 + K + longest description, 1 + longest group name, title length, credit row width). K = longest key + 1. Credit row width = hint length + 1 + 30 (30 = credit incl. trailing space; hint length 0 in text inputs); counted even when credit not shown. Description wrap width = inner width - 2 - K.
- Height: M = max(3, R-3). Box height never > M, top never above row 3.
- Rows: top border, title ` Help · <context label>` bold; per group ` <group>` (accent), items `  <key>` (key column padded to K, `mode` color) + description (word-wrapped); ` … <k> more` row (dim) when items cut; bottom row (hint/credit) when shown; bottom border.
- Overflow: body rows available A = M - 3 - (1 when hint shown). All items fit in A: all shown. Else items shown in order while whole item (all wrapped lines) fits in A-1 rows, stop at first item that does not fit; group title only with >= 1 of its items; then ` … <k> more`, k = items not shown.
- Tiny (M < 5, R 6-7; items never fit): R=7 (M=4): rows = border, title, ` … <k> more` (k = all items), border; no bottom row. R=6 (M=3): border, title, border only. R < 6: UNSPEC-33.
- Bottom row: hint left (dim): ` ? move keys` (level 1 and context has motion keys), ` ? close` (otherwise), none in text inputs; right: `Made by Wouter de Wild - 2026 ` (dim, trailing space) when inner width >= credit row width; in text inputs credit only when items not cut and body + credit row fit A. Text input without credit: no bottom row.
- Example 80x24 diff view L1: last row ` ? move keys` + spaces + `Made by Wouter de Wild - 2026 `, right border at column 80, bottom border on row 23.

### F-HELP-02 contexts

- Priority: editor > find > goto > dialog (delete/quit modal) > search > mcp > config > picker > focused comment > visual > browse > diff.
- No `Cursor mode` context: diff view (cursor always on) = `Diff view`. Browse = `File viewer`.
- Labels: `Diff view`, `Visual selection`, `Focused comment`, `Editor`, `Find in file`, `Go to line`, `File picker`, `File search`, `MCP`, `Config`, `Confirm`, `File viewer`.

### F-HELP-03 entries

Format `key :: description`, grouped `[group]`. L1 = level 1, L2 = level 2 (only if differs).

- No `i` entry anywhere. Keys the context's footer always shows are left out (e.g. `J/K` and `?` in Diff view / File viewer; `J/K` listed in Visual selection and Focused comment).
- Diff view L1: [Find] `]/[ :: next/prev change`, `/ n/N :: find, next/prev`, `: :: go to line`, `tab/S-tab :: next / prev file`, `f/F :: file picker / search`; [Comments] `v/V :: select chars/lines`, `a :: comment on line`, `)/( :: numbered, any file`, `E :: export comments`; [General] `s/c/m :: split, full, staged`, `t/r :: theme / reload`, `p :: old/new pane`, `M/C :: MCP / config`, `q :: quit`. (`p` listed also when split not effective.)
- Diff view L2: [Move] `w/b/e :: word fwd/back/end`, `0/^/$ :: start/nonblank/end`, `d/u :: half page down/up`, `PgDn/PgUp :: page down/up (space: down)`, `g/G :: first / last line`, `1-9 :: count (5j, 12G)` first, then L1 groups.
- Visual selection L1 (no L2): [Selection] `V :: lines, end`, `a :: comment on it`; [Comments] `J/K :: next/prev in file`, `)/( :: numbered, any file`.
- Focused comment L1: [Comments] `J/K :: next/prev in file`, `)/( :: numbered, any file`, `Enter :: edit (no replies)`, `A :: ask: all`, `up/down :: pick block to copy`, `hjkl… :: motion unfocuses`.
- Focused comment L2: [Comments] same 6 items; [Move] `d/u :: scroll thread`, `g/G :: thread top/bottom`.
- Editor: [Editor] `type :: comment text`, `tab :: save / ask agent`, `left/right :: move cursor`, `backspace :: delete char`.
- Find in file: [Find] `type :: search text`, `backspace :: delete char`, `Enter :: jump to match`, `esc :: cancel`.
- Go to line: [Go to line] `type :: line number`, `backspace :: delete char`, `Enter :: go to line`, `esc :: cancel`.
- File picker L1: [File picker] `Enter :: open file`, `esc/f/q :: close`. L2: [Move] `j/k :: move`, `d/u :: half page down/up`, then L1.
- File search: [Search] `type :: filter files`, `backspace :: delete char`, `down/up :: next / prev hit`, `Enter :: open hit`, `esc :: close`.
- MCP L1: [MCP (M)] `⏎/space :: server on / off`, `Enter :: register agent`, `d :: unregister agent`, `y/n :: confirm / cancel`, `c/w :: copy cmd / prompt`, `R :: refresh status`, `esc/q/M :: close`. L2: [Move] `j/k :: move`, then L1.
- Config L1: [Config (C)] `h/l :: change value`, `⏎/space :: toggle / apply`, `esc/q/C :: close`. L2: [Move] `j/k :: select setting`, then L1.
- Confirm: [Dialogs] `y/Enter :: confirm`, `n/esc :: cancel (q: quit)`.
- File viewer L1: [File viewer] `esc :: back to diff`, `s/c/m/f :: diff-only, no-op`; [Comments] `v/V :: select chars/lines`, `a :: comment on line`, `)/( :: numbered, any file`. L2: [Move] same 6 items as Diff view L2 [Move], then L1.

### F-HELP-04 footer hints

- Diff (unified or split not effective), browse, no-changes screen (any split setting / cols, F-MODE-05): `hjkl move  enter ask  J/K comments  ? help`. No `esc exit`, no scroll-mode footer.
- Diff, split effective: `hjkl move  enter ask  J/K comments  p pane  ? help`.
- Visual: `v/esc end  enter ask  hjkl move  ? help`.
- Focused comment: `e edit  D delete  a ask/follow up  j/k scroll  esc back  ? help`.
- Editor new comment: `enter send  tab save/ask  esc cancel`. Edit / follow-up: `enter send  esc cancel`.
- Modals and find/goto keep underlying hint.

## QUIT

### F-QUIT-01 `q`

- Pre: diff/browse, no modal/text input.
- confirmQuit on: modal `Quit xplain? (y/n)`. `y`/Enter: quit. `n`, `q`, Esc: close. Other keys ignored; `?` toggles help.
- confirmQuit off: quit now.
- Quit: MCP server stopped (pending long polls answered `closed` best-effort), exit 0.

## RELOAD

### F-RELOAD-01 `r`

- Re-runs diff load with current mode/scope/args; re-reads browsed file (read error ignored, old text kept).
- Note `reloaded` immediately (even when nothing changed); diff load error: note = F-MODE-04 error text made single line: each `\n` replaced by one space, then leading/trailing whitespace trimmed (footer stays one row; error screen not shown; old files kept).
- Works in diff, browse, focused comment, no-changes screen, error screen.
- Result identical: no visible change. Else files replaced, same file path kept shown (missing: first file).

### F-RELOAD-02 agent reload

- MCP `files_changed` call: same reload, silent (no note, errors ignored).

### F-RELOAD-03 cursor kept

- Reload of same file (same scope, same effective layout): cursor stays on same source line (line number, deleted flag, pane). Line gone: nearest new-side line number. No numbers: same row index clamped.
- Viewport: top kept (clamped to bottom offset), then follow (F-NAV-09).
- Comment boxes re-anchor (F-COMMENT-05).

## CFGUI

### F-CFGUI-01 config modal `C`

- Pre: no modal/text input.
- Box width min(cols, 56), height 10. Title ` Config` bold.
- Rows (label padded to 14): `theme`, `mode`, `split`, `view`, `confirm quit`, `mcp on startup`.
- Choices: theme `solarized vibrant dull contrast colorblind light`; mode `all staged unstaged`; split `off on`; view `full changes`; confirm quit `off on`; mcp on startup `off on`.
- Row text: `> ` (selected) or two spaces, label, `‹ ` when choices hidden left else two spaces, choice cells joined by one space: committed value `[v]`, others `v` (space-padded, same width as `[v]`), then ` ›` when choices hidden right. E.g. unselected mode row, committed all: ` mode            [all]  staged   unstaged`. Selected row bg selBg fg selFg; choice cursor on selected row reverse: covers its cell (`[v]` / `v`) plus the joining space before it (none for first shown choice).
- Choice window: fits width-22 cols, each choice len+2 plus 1 gap; grows from cursor right first then left alternately.
- Hint ` j/k row h/l browse enter select esc close`.
- On open: choice cursors on current values; committed values = current state.

### F-CFGUI-02 keys

- `j`/Down, `k`/Up: row (clamped). `h`/Left, `l`/Right: choice cursor (clamped). Theme row: live preview of theme under cursor.
- Enter/Space: select choice under cursor: apply now + save (F-CONFIG-05). Theme becomes committed. mode: file index 1, reload when value differs (same as `m`, F-MODE-03). split: like `s` (F-LAYOUT-04). view: scope change keeping file (like `c`). confirm quit: affects next `q`. mcp on startup: preference only, no start/stop. Modal stays open.
- Selecting current value: still saved; mode: file index 1 without reload. Mode/view/split select without row change: cursor kept, viewport per F-NAV-10.
- Esc, `q`, `C`: close; theme preview reverted to committed theme.

### F-CFGUI-03 save errors

- Broken file: note `config unreadable, not saved (<path>)`; setting still applied live.

## MCPUI

### F-MCPUI-01 modal `M`

- Pre: no modal/text input. Opens with power row selected, no note/preview/confirm.
- Box width min(cols, 64), round border. Rows:
  - ` MCP` bold.
  - Power row: `> ` / two spaces + `○ off` | `… starting` | `● on ` + ` 127.0.0.1:<port>` when on.
  - Error (dels color) when last start failed: ` <error>`.
  - ` clients <c> pending <p> delivered <d>` (dim).
  - Up to 2 clients `   <name>[ <version>][ ⟳]` (⟳ = long poll waiting), then `   … +<k> more`.
  - ` INTEGRATIONS` (accent).
  - Per integration (order Claude Code, Codex, OpenCode, Copilot): `> `/two spaces + `<label> ` + `… ` when busy + status. Suffix `  restart needed` on claude/codex/copilot rows: UNSPEC (UNSPEC-5). Status: `copy-paste only` (OpenCode), `stale`, `registered`, `not registered` (also before any check).
  - Example (running, nothing registered, port 47615; `  restart needed` suffixes UNSPEC-5, tests match row prefix):

```
 MCP
> ● on  127.0.0.1:47615
 clients 0 pending 0 delivered 0
 INTEGRATIONS
  Claude Code not registered  restart needed
  Codex not registered  restart needed
  OpenCode copy-paste only
  Copilot not registered  restart needed
 j/k  enter register  d remove  c/w copy  R refresh  esc
```

- Confirm (accent): ` Register <label> MCP server? (y/n)` + dim ` adds the xplain MCP server to <label> config`; or ` Remove <label> registration? (y/n)` + dim ` removes the xplain MCP server from <label> config`.
- Note (accent, rows ` <text>`, max 2 wrapped rows, rest dropped): modal note, else selected integration's last result message. Hidden during confirm. Note <= width-4 chars: one row. Longer: wrap width / break points UNSPEC (UNSPEC-41); tests assert first-row prefix only.
- Preview: dim ` <what>:` + up to 3 lines of text wrapped at width-4 (F-ASK-05 wrap); more lines: 3rd line cut to width-5 chars + `…` (blank 3rd line gives ` …`). Hidden during confirm.
- Hint (dim): ` j/k  enter register  d remove  c/w copy  R refresh  esc`; during confirm ` y confirm  n/esc cancel`.
- Selected row bg selBg fg selFg.

### F-MCPUI-02 keys

- Confirm pending: `y`/Enter run register/unregister; `n`/Esc cancel. Others ignored.
- Esc, `q`, `M`: close.
- `j`/Down, `k`/Up: row (power + 4), clamped; clears preview and note.
- Power row Enter/Space: start server if off, stop if on. Clears note.
- Integration row Enter: OpenCode: note `copy-paste only: c copies the snippet`; MCP off: `start MCP first`; registered and not stale: `already registered (d to remove)`; else confirm register. Clears preview.
- Integration row `d`: OpenCode: `copy-paste only: c copies the snippet`; not registered: `not registered`; else confirm unregister (MCP state irrelevant). Clears preview.
- Space on integration row, `d`/`c`/`w` on power row: nothing.
- Integration row `c`: copy register command; `w`: copy watch prompt. MCP off: note `start MCP first`. Else OSC 52 copy of full text (with token); preview `register command (copied)` / `watch prompt (copied)` showing text with token replaced by `***`.
- `R`: MCP off: note `start MCP first`; else re-check registrations.

### F-MCPUI-03 start/stop effects

- Start: header `[mcp: on]`; new comments default ask mode; registration check runs (F-INTEG-02).
- Start fail: stays off, error row shows message. Port busy: `MCP port <port> is already in use on 127.0.0.1. Stop the other process or choose another port.` Other listen failure: `cannot listen on 127.0.0.1:<port>: <reason>` (not black-box tested).
- Stop: live answers become `cancelled` with text `MCP stopped`; every waiting `next_question` long poll answered at once, HTTP 200, result `{"status":"closed","note":"xplain closed the session. Stop."}` (F-MCPSRV-06), response fully written before server closes; header `[mcp: off]`; clients list cleared, pending 0; editor mode back to save. `delivered <d>` after stop: UNSPEC (UNSPEC-21); 0 after next start.
- Integration statuses kept after stop (last known). Start error row stays until next start attempt.

### F-MCPUI-04 autostart

- `mcp.autostart` true: server starts after first frame, non-blocking. Failure: note `mcp autostart failed: <error>`. Token never shown.

## MCPSRV

HTTP JSON-RPC 2.0 MCP server (streamable HTTP, JSON responses only, no SSE).

### F-MCPSRV-01 listen and token

- Listens `127.0.0.1:<port>` (Environment), URL `http://127.0.0.1:<port>/mcp`. Only while started.
- Token file `<state dir>/mcp.json`, state dir `$XDG_STATE_HOME/xplain` or `$HOME/.local/state/xplain`. Dir created mode 0700; file mode 0600; content `{"token": "<t>"}` JSON 2-space indent + `\n`.
- Existing file with string `token` length >= 16: reused, file untouched. Other fields (e.g. `port`): UNSPEC (UNSPEC-15). Missing/corrupt/short token: new token = 32 random bytes base64url (43 chars), file rewritten as `{"token": ...}` only.
- Created on first start, not on launch. Token file write failure (e.g. state dir not writable): start fails, error `cannot write <state dir>/mcp.json: <reason>` (F-MCPUI-01 error row, F-MCPUI-04 note).
- Dir mode 0700 only applied when dir created; file chmod 0600 on every write.

### F-MCPSRV-02 request checks

In order:

1. `Host` header hostname not `localhost`, `127.0.0.1`, `[::1]`, `::1` (or missing: not black-box tested), or `Origin` present with non-loopback hostname (unparseable = bad): 403 `{"error":"forbidden"}`.
2. Path (query ignored) not `/mcp`: 404 `{"error":"not found"}`.
3. `Authorization` not exactly `Bearer <token>`: 401 `{"error":"unauthorized"}`, header `www-authenticate: Bearer`.
4. Method not POST: 405 `{"error":"method not allowed"}`, header `allow: POST`.
5. Body > 1048576 bytes: 413 `{"error":"body too large"}`, `connection: close`.
6. Body not JSON: 400 `{"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Parse error"}}`.

- Responses `content-type: application/json`. Unexpected failure: 500 `{"jsonrpc":"2.0","id":null,"error":{"code":-32603,"message":"Internal error"}}` (not black-box tested).

### F-MCPSRV-03 JSON-RPC envelope

- Body single object or array (batch). Batch response array (even one item), order kept. Items handled sequentially (long poll in batch blocks later items).
- Item not object or `method` not string: `{"jsonrpc":"2.0","id":<id or null>,"error":{"code":-32600,"message":"Invalid Request"}}` (also for items without `id`).
- `jsonrpc` field not checked. Response `id` echoes request `id` as sent.
- Item without `id` key: notification, no response item (any method, even unknown). `"id": null` counts as request (response with `id: null`). All notifications, or empty batch `[]`: 202 empty body (no content-type).
- Unknown method: error -32601 `Method not found: <method, max 100 chars>`.
- Session: request header `mcp-session-id` (id from `initialize`) identifies client. Missing or unknown id: UNSPEC (UNSPEC-12).

### F-MCPSRV-04 initialize, ping, tools/list

- `initialize`: new session id (UUID) in response header `mcp-session-id` (each initialize = new client entry; later items of same batch use it). Client name/version from `params.clientInfo` (non-string: `unknown` / empty), sanitized, max 200/100 chars (caps not black-box tested). Result:

```
{"protocolVersion": <requested if in [2025-06-18, 2025-03-26, 2024-11-05] else "2025-06-18">,
 "capabilities": {"tools": {"listChanged": false}},
 "serverInfo": {"name": "xplain", "version": "0.1.0"},
 "instructions": "<SERVER_INSTRUCTIONS>"}
```

- SERVER_INSTRUCTIONS verbatim:

```
xplain shows the user a live diff of the working tree. REQUIRED: after EVERY edit, create, rename or delete of a file on disk, call the `files_changed` tool (pass the changed paths). Without this call the xplain view stays stale and the user does not see your changes. Batch several edits into one call, but always call it before you answer or go idle. Questions from the user arrive via `next_question`; answer with `answer`.
```

- `ping`: result `{}`.
- `tools/list`: result `{"tools": [...]}`, 5 tools in order below, each `{name, description, inputSchema}`.

### F-MCPSRV-05 tools/call

- `params.name` unknown: error -32602 `Unknown tool: <name, max 100>` (non-string name: `Unknown tool: `).
- `params.arguments` non-object treated as `{}`.
- Result `{"content":[{"type":"text","text":<string>}]}`, plus `"isError": true` on tool errors. JSON results serialized compact into `text`.
- Sanitize (all agent text): strip ANSI escapes and control chars except `\n` `\t`, max 20000 chars (cap not black-box tested).

Tool `next_question`:

- Description verbatim:

```
Long-poll for the next question a human asked in the xplain code-review UI. Returns {status:"question", thread_id, turn, follow_up, question}: answer it with the answer tool. If follow_up is true it continues an earlier thread: use `previous` (earlier questions and your answers) for reference and answer with the SAME thread_id. A follow-up can also be the user replying to a note you added with annotate: the question then names the note (file, line, text). If status is "no_question_yet", call next_question again immediately. Keep looping: after every answer, call next_question again immediately, until status is "closed".
```

- Schema: `{"type":"object","properties":{"wait_seconds":{"type":"integer","minimum":1,"maximum":120,"default":45,"description":"Max seconds to wait for a question (1-120)."}},"additionalProperties":false}`.

Tool `answer`:

- Description verbatim:

````
Send your answer for a question received from next_question, using its thread_id (for a follow-up, the same thread_id as before). Plain text or markdown. Put code samples in markdown fenced blocks (```lang ... ```): xplain highlights them and gives the user a copy button. Then call next_question again immediately.
````

- Schema: properties `thread_id` `{"type":"string","description":"thread_id from next_question."}`, `text` `{"type":"string","description":"The answer text. Put code samples in markdown fenced blocks (```lang ... ```): xplain highlights them and gives the user a copy button."}`; required `["thread_id","text"]`; additionalProperties false.

Tool `get_questions`:

- Description: `List questions (and follow-ups, marked follow_up) still waiting to be delivered, without consuming them. Does not replace the next_question loop.`
- Schema: `{"type":"object","properties":{},"additionalProperties":false}`.

Tool `annotate`:

- Description: `Attach a note to a line of a file in the xplain diff view. The user may reply to it; replies arrive via next_question as follow-ups. Then continue the next_question loop.`
- Schema properties: `file` `{"type":"string","description":"File path as shown in the diff."}`; `line` `{"type":"number","description":"1-based line number."}`; `text` `{"type":"string","description":"Annotation text. Put code samples in markdown fenced blocks (```lang ... ```): xplain highlights them and gives the user a copy button."}`; `side` `{"type":"string","enum":["old","new"],"description":"Diff side; default new."}`; `number` `{"type":"integer","description":"Optional order label shown on the note (e.g. step 1, 2, 3); the user jumps between numbered notes in order."}`; required `["file","line","text"]`; additionalProperties false.

Tool `files_changed`:

- Description: `REQUIRED after every file change: you MUST call this after each edit, creation, rename or deletion of any file on disk (also right after a batch of edits, before replying). It makes the xplain diff view reload; without it the user sees stale content. Pass the changed paths in `paths`. Then continue the next_question loop.`
- Schema: `{"type":"object","properties":{"paths":{"type":"array","items":{"type":"string"},"description":"Paths of the files you edited, created or deleted (relative to the repo root)."}},"additionalProperties":false}`.

### F-MCPSRV-06 next_question

- `wait_seconds`: non-number: 45; floored; clamped 1..120 (clamp not black-box tested; out-of-range value is no error).
- Queue non-empty (eligible item): returns immediately. Else waits until question enqueued or timeout.
- Result texts (JSON):
  - question: `{"status":"question","thread_id":"q1","turn":1,"follow_up":false,"question":"<q>"}`; follow-up adds `"previous":[{"turn":1,"question":"...","answer":"..."}]` before `question` and `follow_up:true`.
  - timeout: `{"status":"no_question_yet","call_again":true,"note":"No question yet. Call next_question again immediately."}`.
  - server closing/stopped: `{"status":"closed","note":"xplain closed the session. Stop."}`. MCP stop from modal (F-MCPUI-03): every waiting poll gets it at once (HTTP 200, written before server closes). Quit: best-effort (F-QUIT-01).
- `<q>` turn 1: `<message>\n\n<context>`. Context:

````
File: <file>
Side: new (after the change)          (or: old (before the change))
Lines: <a>-<b>                        (a when single; line omitted when none)

Code at cursor line:                  (selection: Selected code:)
```
<text>
```

Surrounding context:                  (block omitted when no context rows)
```
<context lines>
```
````

- `Side:` = comment pane side (F-COMMENT-03). Unified del row comment: `Side: new (after the change)`, `Lines: <old line no>`, no deleted marker anywhere in context.
- `<q>` follow-up: `Follow-up to your earlier answer (thread <id>, turn <n>): <message>`; agent note reply adds `\n\n` + note context:

```
Reply to your annotate note #<number>:      ("#<number>" omitted when none)
File: <file>
Side: <old|new>
Line: <line>

Your note:
<note text>
```

- `previous`: earlier turns, last 5, each text max 4000 chars. `answer` = all answers of that turn joined `\n\n` (empty string if none). `question` of a human turn = message only (no context). Agent note turn 1: question `(note you added with annotate)`, answer = note text.
- Example follow-up result: `{"status":"question","thread_id":"q1","turn":2,"follow_up":true,"previous":[{"turn":1,"question":"why?","answer":"because"}],"question":"Follow-up to your earlier answer (thread q1, turn 2): and?"}`.
- Same client new poll while one pending: old poll answers `no_question_yet`.
- HTTP connection dropped while waiting: poll cancelled. Question delivered but response not written (client gone): requeued at front, delivered count decremented (not black-box tested). UI status meanwhile: UNSPEC (UNSPEC-23).
- Sticky: follow-up turn of thread goes to client that got earlier turn while that client is polling; else any client.
- Multiple pollers: queue dispatched in poller arrival order.
- Delivery: UI answer status `streaming`, agent name = client name.

### F-MCPSRV-07 answer

- Missing/non-string `thread_id` or `text`: isError text `thread_id and text (strings) are required`.
- Thread never delivered: isError `Unknown thread_id: <id, max 100>`.
- Ok: `{"ok":true,"note":"Answer delivered. Call next_question again immediately."}`. UI: F-ASK-09. Text sanitized. Thread of deleted comment: UNSPEC (UNSPEC-7).

### F-MCPSRV-08 annotate

- `file` string, `line` number, `text` string required, else isError `file (string), line (number) and text (string) are required`.
- `side` other than `old`/`new`: ignored (new). `number` non-integer: ignored. `line` floored, min 1.
- Ok: `{"ok":true}`. UI: agent note added on file/line/side (F-COMMENT-10); visible when that file shown (diff or browse) and line row present. Line row absent: UNSPEC (UNSPEC-14).

### F-MCPSRV-09 get_questions

- `{"questions":[{"thread_id":"q1","turn":1,"follow_up":false,"preview":"<message first 200 chars>"}]}` for queued, undelivered questions. Does not consume.

### F-MCPSRV-10 files_changed

- `paths` optional array (missing or not an array: no paths); non-strings dropped; max 100, each max 500 chars. Result `{"ok":true}`. UI silent reload (F-RELOAD-02).

### F-MCPSRV-11 clients and counters

- Client list: every session from `initialize` (name/version). `polling` while long poll waits. Polls without valid session: UNSPEC (UNSPEC-12).
- pending = queued undelivered; delivered = deliveries count. Shown in MCP modal.

## INTEG

Integrations. Commands run in cwd, env inherited; timeout UNSPEC (UNSPEC-37). `<url>` = `http://127.0.0.1:<port>/mcp`, `<token>` = token. Token masked as `***` in every shown message.

### F-INTEG-01 catalog

| id       | label       | CLI       | poll seconds | auto register        |
| -------- | ----------- | --------- | ------------ | -------------------- |
| claude   | Claude Code | `claude`  | 100          | yes                  |
| codex    | Codex       | `codex`   | 45           | yes                  |
| opencode | OpenCode    | none      | 45           | no (copy-paste only) |
| copilot  | Copilot     | `copilot` | 120          | yes                  |

- argv (one element per space, quoted part single element):
  - claude check: `claude mcp get xplain`; remove: `claude mcp remove xplain -s local`; add: `claude mcp add xplain <url> --transport http --scope local --header "Authorization: Bearer <token>"`.
  - codex check: `codex mcp get xplain --json`; remove: `codex mcp remove xplain`; add: argv starts `codex mcp add xplain --url <url>`, rest (token passing, currently `--bearer-token-env-var XPLAIN_MCP_TOKEN`) UNSPEC (UNSPEC-6).
  - copilot check: `copilot mcp get xplain --json`; remove: `copilot mcp remove xplain`; add: `copilot mcp add xplain <url> --transport http --header "Authorization: Bearer <token>" --timeout 200000`.
- xplain writes no agent config files itself.

### F-INTEG-02 registration check

- Runs after server start and on `R`, for claude, codex, copilot.
- Exit 0: registered. Stdout URL: JSON `url`, or `xplain.url`, or `mcpServers.xplain.url`, else first `http(s)://` URL in text (up to whitespace, `"`, `'`, `,`). URL found and != `<url>`: `stale`. No URL: `registered`. Non-zero exit: `not registered`.
- Any spawn failure (CLI not found, timeout, other): `not registered`, no note.
- Check that follows register/unregister (F-INTEG-03, F-INTEG-04): that result note kept. Any other check (`R`, next server start): clears that row's note (incl. earlier register/unregister result). Codex absent from PATH: row `not registered`.

### F-INTEG-03 register

- Confirmed in modal. Row shows `… ` busy.
- Runs remove (result ignored), then add.
- Add ok: note `<hint>; restart the agent session, then paste the watch prompt`. Hints:
  - claude: `Registered. Restart or resume the session (claude --resume), then paste the watch prompt.`
  - codex: hint text UNSPEC (UNSPEC-6). Current: `Registered. Codex reads the token from env: export XPLAIN_MCP_TOKEN=<token> before starting codex, then resume (codex resume --last).` (`<token>` literal).
  - copilot: `Registered. Restart copilot (or use /mcp), then paste the watch prompt.`
- CLI missing: `<label> CLI not found`. Non-zero exit: `<label> register failed (exit <code>)` + `: <out>` when out non-empty. out: s = stderr if non-empty (even whitespace-only), else stdout; s trimmed as whole (both ends); split at `\n`; first 3 pieces joined by one space (pieces not trimmed, inner blank lines kept as empty pieces). E.g. stderr `\n  bad\n\nx\ny\n` gives `: bad  x` (pieces `bad`, ``, `x`). Other spawn error: `<label> register failed: <message>` (e.g. timeout, UNSPEC-37). Token masked `***` in all.
- Note shown max 2 wrapped lines (F-MCPUI-01), on that row when selected.
- Then registration check re-runs; note kept.

### F-INTEG-04 unregister

- Confirmed. Runs remove. Ok: `Removed xplain from <label>`. Errors as register with `unregister`. Works with MCP off if row known registered; row status afterwards UNSPEC (UNSPEC-22).
- Remove argv same as F-INTEG-01 remove.

### F-INTEG-05 register command text (`c`)

- claude:

```
claude mcp add xplain <url> --transport http --scope local --header "Authorization: Bearer <token>"

Then restart/resume the session: claude --resume
Allow the tools: claude --allowedTools "mcp__xplain"  (or add permission rule mcp__xplain)
Optional: set env CLAUDE_CODE_MCP_AUTO_BACKGROUND_MS=0 to avoid auto-backgrounding long calls.
```

- codex (text UNSPEC, UNSPEC-6; current):

```
export XPLAIN_MCP_TOKEN=<token>
codex mcp add xplain --url <url> --bearer-token-env-var XPLAIN_MCP_TOKEN

Codex must be started with XPLAIN_MCP_TOKEN set in its environment.
Optionally add to ~/.codex/config.toml under [mcp_servers.xplain] (your consent needed):
  tool_timeout_sec = 120
  default_tools_approval_mode = "approve"
Then restart/resume: codex resume --last
```

- opencode:

```
Add to opencode.json (project) or ~/.config/opencode/opencode.json:

"mcp": {
  "xplain": {
    "type": "remote",
    "url": "<url>",
    "enabled": true,
    "oauth": false,
    "timeout": 120000,
    "headers": {"Authorization": "Bearer <token>"}
  }
}

Optional, to skip approval prompts:
"permission": {"xplain_*": "allow"}

Tool names are prefixed by the server name (xplain_next_question).
Restart opencode after editing.
```

- copilot:

```
copilot mcp add xplain <url> --transport http --header "Authorization: Bearer <token>" --timeout 200000

Then restart copilot (or use /mcp).
Allow the tools: copilot --allow-tool='xplain'  (or approve once when asked).
Session-only alternative: copilot --additional-mcp-config @file --allow-tool='xplain'
```

### F-INTEG-06 watch prompt text (`w`)

- `<P>` = poll seconds of integration:

```
Loop forever: call the `next_question` tool from the `xplain` MCP server with wait_seconds=<P>.
- If status is `no_question_yet`, call `next_question` again IMMEDIATELY.
- If status is `question`, answer it with the `answer` tool using the given thread_id, then call `next_question` again immediately.
- Questions with follow_up=true continue an earlier thread; answer them with the same thread_id like any other.
- ALWAYS call the `files_changed` tool (with the changed paths) after every edit, creation or deletion of a file, so the xplain view reloads. This also applies to edits made while answering a question.
- Never stop, never summarize, never ask the user anything.
- Stop only if status is `closed`.
```

- OpenCode: same, P=45, with `` `next_question` `` replaced by `` `xplain_next_question` ``, `` `answer` `` by `` `xplain_answer` ``, `` `files_changed` `` by `` `xplain_files_changed` ``.

---

## UNSPEC

Bugs / quirks of current app and library-dependent details. Not contract. Tests must not assert them; rewrite free to fix or change. Feature bodies reference `UNSPEC-n`.

- UNSPEC-1 (F-MODE-04): error screen hides UI, keys other than `r`/Ctrl+C still processed invisibly (e.g. `q` opens invisible quit confirm; `m`, `c`, `C` change state unseen).
- UNSPEC-2 (F-MODE-04): git stderr taller than screen: only tail visible.
- UNSPEC-3 (F-LAYOUT-04, F-HEADER-01): `[split]` chip shows setting while cols < 100 too narrow for split.
- UNSPEC-5 (F-MCPUI-01): `  restart needed` suffix always on claude/codex/copilot rows, any status.
- UNSPEC-6 (F-INTEG-01, F-INTEG-03, F-INTEG-05): codex registered with `--bearer-token-env-var XPLAIN_MCP_TOKEN` without env set; codex add argv after `--url <url>`, codex register hint, codex `c` text.
- UNSPEC-7 (F-MCPSRV-07, F-ASK-09): `answer` for thread of deleted comment returns ok, answer dropped.
- UNSPEC-8 (F-SEARCH-02): pasted newlines kept in search query.
- UNSPEC-9 (F-CLI-03): `--cwd` repo subdir: diff paths repo-root-relative, browse/search paths cwd-relative (mismatch).
- UNSPEC-10 (F-CLI-03): `--no-color` arg both disables colors and is passed to git.
- UNSPEC-11 (F-LAYOUT-02, F-FIND-03, F-GOTO-02): note lifetime; current note never times out, stays through later actions until replaced or config save.
- UNSPEC-12 (F-MCPSRV-03, F-MCPSRV-11): `mcp-session-id` not validated; missing header gives anonymous client per TCP connection (`anon:<remote port>`); poll-only clients named `unknown`.
- UNSPEC-13 (F-MODE-05): comment editor on no-changes screen creates comment on empty path.
- UNSPEC-14 (F-COMMENT-09, F-MCPSRV-08): agent note on line not in shown rows invisible; `)`/`(` to it notes `comment not in view`.
- UNSPEC-15 (F-MCPSRV-01): `mcp.json` fields besides `token`: current app never writes `port` (file holds only `token`), existing `port` field ignored (port rewrite never runs). Rewrite may add fields.
- UNSPEC-16 (F-LAYOUT-07): cursor row padded past screen edge, last cell always `…`.
- UNSPEC-17 (F-EDGE-07, F-SEARCH-01): non-ASCII paths git-escaped in header/picker (`\303\251.txt`), search shows quoted form, opening fails `cannot read <p>: not found`.
- UNSPEC-18 (F-LAYOUT-08): modal narrower than its hint + 2: rows overlap, title / first list row hidden.
- UNSPEC-19 (F-FILES-02): empty picker `j`/`d` + Enter leaves file index -1; `No changes` persists after reload finds files, until `m`/`c`.
- UNSPEC-20 (F-HEADER-03, F-CURSOR-06): header pane tag says `new` on del-only split row while char cursor in left pane.
- UNSPEC-21 (F-MCPUI-03): `delivered` count kept after MCP stop.
- UNSPEC-22 (F-INTEG-04): unregister with MCP off does not re-check status (row keeps `registered`).
- UNSPEC-23 (F-MCPSRV-06): UI status stays `streaming` after question requeued.
- UNSPEC-24 (F-COMMENT-05): comment on unified del row hidden in split when deleted line paired with added line.
- UNSPEC-25 (F-NAV-07): Ctrl combos in config, MCP, confirm modals not filtered (act as plain key).
- UNSPEC-26 (F-EDGE-08): ANSI escapes / control chars in file content not sanitized (reach terminal).
- UNSPEC-27 (F-EDGE-08): CR at line end (CRLF files) handling.
- UNSPEC-28 (F-EDGE-08, F-CURSOR-04, F-VISUAL-02, F-FIND-02): column unit for non-ASCII text; current UTF-16 code units. Rewrite may count code points / graphemes. Wide char = 2 cells stays contract.
- UNSPEC-29 (F-CONFIG-04): parser / read error text between prefix and suffix.
- UNSPEC-30 (F-SEARCH-01): hit order except exact path first (current: fuzzysort ranking; rewrite may use fzf/nucleo-like scoring); which placement highlighted when several match; matched-char fg on selected row (current: selFg). Hit set, smart case, literal chars (no extended syntax) are contract. Current app deviates (fuzzysort: always case-insensitive, space splits terms): app to be fixed.
- UNSPEC-31 (Colors, F-ASK-06): syntax token colors, language detection details.
- UNSPEC-33 (F-LAYOUT-01): R < 6: frame still 6 rows (H=3), top rows scroll off terminal (header not visible).
- UNSPEC-34: (now contract, see F-CURSOR-05: any non-count key clears count).
- UNSPEC-35: (now contract, see F-MODE-05, F-HELP-04: no-changes footer never `p pane`, `p` no-op).
- UNSPEC-36 (F-CONFIG-05): config write mechanism: current temp `<path>.<pid>.tmp` + rename (atomic), temp removed on failure. Not observable black-box.
- UNSPEC-37 (F-MODE-01, F-INTEG-01..04): command timeouts: current git 60 s, integration CLIs 20 s (timeout note `<label> register failed: <message>`, check `not registered`).
- UNSPEC-38 (F-COMMENT-03): ask-mode submit while MCP not running (current note `MCP is off (M to start)`): unreachable, ask mode exists only while MCP running.
- UNSPEC-39 (F-ASK-02, F-ASK-03): notes `can't reply to this note yet` (agent note refusing reply) and `can't follow up yet` (follow-up editor Enter, MCP running, thread refuses): no black-box trigger.
- UNSPEC-40 (F-ASK-02, F-ASK-05, F-EXPORT-02): answer status `error` (divider `error` in dels color, error text in dels color, export state `error` and `Error:` line): no black-box trigger (nothing sets it). Rewrite may drop it.
- UNSPEC-41 (F-MCPUI-01, F-INTEG-03): MCP modal note wrap width and break points for notes longer than width-4 (current: F-ASK-05 wrap at max(10, width-4)).
- UNSPEC-42 (Messages, F-MODE-04): text after reason `failed` (current: none); which of several failing codes wins. git failing with empty stderr (timeout, killed, non-zero exit without output): current `git failed` / `git failed (exit <n>)`.

## Coverage index

Test: `yes` = has Rust tests (in-process scenario tests live in `xplain-sim`, unit tests in each crate; `xplain-sim/tests/spec_coverage.rs` gates this index); `no (REMOVED)` / `no (UNSPEC)` = out of test scope. UNSPEC parts of in-scope features never asserted (UNSPEC section). Parts marked `(not black-box tested)` in a feature body are contract but have no scenario-test trigger (F-MCPSRV-02 500 / missing Host, F-MCPSRV-04 clientInfo caps, F-MCPSRV-05 20000-char cap, F-MCPSRV-06 requeue of a delivered question whose response was not written, 1..120 `wait_seconds` clamp, F-MCPUI-03 other listen failure).

| ID           | Summary                                                               | Test         |
| ------------ | --------------------------------------------------------------------- | ------------ |
| F-CLI-01     | `-h`/`--help` usage to stdout, exit 0                                 | yes          |
| F-CLI-02     | flag errors to stderr with usage, exit 1                              | yes          |
| F-CLI-03     | flags cwd/config/mode/split/changes-only/theme, git args passthrough  | yes          |
| F-CLI-04     | `config path` prints resolved path                                    | yes          |
| F-CLI-05     | alt screen, Loading..., exit 0, Ctrl+C                                | yes          |
| F-CLI-06     | flag value consumption, unsupported `=` forms, bad --cwd              | yes          |
| F-CONFIG-01  | config path resolution order                                          | yes          |
| F-CONFIG-02  | schema and defaults                                                   | yes          |
| F-CONFIG-03  | per-key validation warnings                                           | yes          |
| F-CONFIG-04  | broken file warning, defaults                                         | yes          |
| F-CONFIG-05  | save: merge, tab JSON, errors (write mechanism UNSPEC-36)             | yes          |
| F-MODE-01    | git diff argv per mode/scope/args                                     | yes          |
| F-MODE-02    | untracked files in all mode                                           | yes          |
| F-MODE-03    | `m` cycles mode                                                       | yes          |
| F-MODE-04    | git error screen, `git diff` stderr deterministic                     | yes          |
| F-MODE-05    | No changes screen, footer never `p pane`, `p` no-op                   | yes          |
| F-EDGE-01    | git prefix strip once (own `a/`/`b/` dir kept), deleted path          | yes          |
| F-EDGE-02    | rename display, rename-only note                                      | yes          |
| F-EDGE-03    | binary note (also added/deleted tracked binary)                       | yes          |
| F-EDGE-04    | No textual changes note                                               | yes          |
| F-EDGE-05    | picker status letters                                                 | yes          |
| F-EDGE-06    | no-newline-at-EOF marker rows                                         | yes          |
| F-EDGE-07    | non-ASCII (git-quoted) paths: all UNSPEC-17                           | no (UNSPEC)  |
| F-EDGE-08    | content chars: tabs, wide chars (rest UNSPEC)                         | yes          |
| F-SCOPE-01   | full vs changes scope                                                 | yes          |
| F-SCOPE-02   | `c` toggles scope keeping file                                        | yes          |
| F-LAYOUT-01  | screen frame rows                                                     | yes          |
| F-LAYOUT-02  | footer composition, note                                              | yes          |
| F-LAYOUT-03  | unified row format and colors                                         | yes          |
| F-LAYOUT-04  | split view, pairing, too narrow                                       | yes          |
| F-LAYOUT-05  | browse row format                                                     | yes          |
| F-LAYOUT-06  | modal placement                                                       | yes          |
| F-LAYOUT-07  | truncation with `…`                                                   | yes          |
| F-LAYOUT-08  | narrow modal layout: all UNSPEC-18                                    | no (UNSPEC)  |
| F-THEME-01   | `t` cycles theme                                                      | yes          |
| F-THEME-02   | theme chrome colors                                                   | yes          |
| F-HEADER-01  | diff header chips                                                     | yes          |
| F-HEADER-02  | browse header                                                         | yes          |
| F-HEADER-03  | cursor/visual header tag, always shown                                | yes          |
| F-NAV-01     | REMOVED: j/k viewport scroll                                          | no (REMOVED) |
| F-NAV-02     | REMOVED: d/u viewport half page                                       | no (REMOVED) |
| F-NAV-03     | REMOVED: Space/PageDown/PageUp viewport page                          | no (REMOVED) |
| F-NAV-04     | REMOVED: g/G viewport top/bottom                                      | no (REMOVED) |
| F-NAV-05     | ]/[ cursor change jump (rewritten)                                    | yes          |
| F-NAV-06     | Tab/S-Tab file switch, Left/Right never switch                        | yes          |
| F-NAV-07     | ctrl keys ignored, digits count                                       | yes          |
| F-NAV-08     | initial cursor + viewport per file                                    | yes          |
| F-NAV-09     | viewport follows cursor, margin, row blocks                           | yes          |
| F-NAV-10     | top reset without cursor move: cursor kept, follow                    | yes          |
| F-FILES-01   | picker render                                                         | yes          |
| F-FILES-02   | picker keys                                                           | yes          |
| F-SEARCH-01  | `F` file search: fzf-style subsequence, smart case, render            | yes          |
| F-SEARCH-02  | search keys                                                           | yes          |
| F-BROWSE-01  | open file read-only, binary, errors                                   | yes          |
| F-BROWSE-02  | browse keys, Esc back to diff, ignored keys                           | yes          |
| F-FIND-01    | `/` input                                                             | yes          |
| F-FIND-02    | smartcase match, highlight                                            | yes          |
| F-FIND-03    | Enter/n/N jumps, not found                                            | yes          |
| F-GOTO-01    | `:` input                                                             | yes          |
| F-GOTO-02    | goto line rules and notes                                             | yes          |
| F-CURSOR-01  | REMOVED: `i` toggle                                                   | no (REMOVED) |
| F-CURSOR-02  | cursor render                                                         | yes          |
| F-CURSOR-03  | vertical cursor moves                                                 | yes          |
| F-CURSOR-04  | horizontal and word moves                                             | yes          |
| F-CURSOR-05  | count prefix, cleared by any non-count key                            | yes          |
| F-CURSOR-06  | `p` pane toggle                                                       | yes          |
| F-CURSOR-07  | horizontal follow shift                                               | yes          |
| F-CURSOR-08  | global keys, `i` unbound                                              | yes          |
| F-CURSOR-09  | cursor always on, start position                                      | yes          |
| F-CURSOR-10  | Esc chain, no cursor exit                                             | yes          |
| F-VISUAL-01  | v/V start/end/switch                                                  | yes          |
| F-VISUAL-02  | selection render                                                      | yes          |
| F-VISUAL-03  | comment on selection, tag format                                      | yes          |
| F-COMMENT-01 | editor open and render                                                | yes          |
| F-COMMENT-02 | editor keys, tab save/ask                                             | yes          |
| F-COMMENT-03 | submit new comment, notes                                             | yes          |
| F-COMMENT-04 | comment box render                                                    | yes          |
| F-COMMENT-05 | anchoring                                                             | yes          |
| F-COMMENT-06 | J/K focus                                                             | yes          |
| F-COMMENT-07 | edit                                                                  | yes          |
| F-COMMENT-08 | delete modal                                                          | yes          |
| F-COMMENT-09 | )/( numbered jump across files                                        | yes          |
| F-COMMENT-10 | agent note render                                                     | yes          |
| F-ASK-01     | enqueue question                                                      | yes          |
| F-ASK-02     | `a` on focused comment                                                | yes          |
| F-ASK-03     | follow-up                                                             | yes          |
| F-ASK-04     | `A` ask all                                                           | yes          |
| F-ASK-05     | thread body, answer divider, status                                   | yes          |
| F-ASK-06     | code blocks in thread                                                 | yes          |
| F-ASK-07     | thread scroll, bottom kept on answer arrival                          | yes          |
| F-ASK-08     | code copy OSC 52                                                      | yes          |
| F-ASK-09     | answer arrival                                                        | yes          |
| F-EXPORT-01  | `E` writes review file                                                | yes          |
| F-EXPORT-02  | markdown format                                                       | yes          |
| F-HELP-01    | `?` panel levels, layout, height cap (tiny R)                         | yes          |
| F-HELP-02    | help contexts and labels (no `Cursor mode`)                           | yes          |
| F-HELP-03    | help entries per context (no `i` entries)                             | yes          |
| F-HELP-04    | footer hints per state (no `esc exit`)                                | yes          |
| F-QUIT-01    | `q`, confirm modal                                                    | yes          |
| F-RELOAD-01  | `r` reload                                                            | yes          |
| F-RELOAD-02  | agent-triggered reload                                                | yes          |
| F-RELOAD-03  | cursor kept on reload                                                 | yes          |
| F-CFGUI-01   | config modal render                                                   | yes          |
| F-CFGUI-02   | config modal keys, live apply                                         | yes          |
| F-CFGUI-03   | config save errors                                                    | yes          |
| F-MCPUI-01   | MCP modal render                                                      | yes          |
| F-MCPUI-02   | MCP modal keys                                                        | yes          |
| F-MCPUI-03   | start/stop effects, stop answers polls `closed`                       | yes          |
| F-MCPUI-04   | autostart                                                             | yes          |
| F-MCPSRV-01  | listen, token file                                                    | yes          |
| F-MCPSRV-02  | request checks, HTTP errors (500, missing Host: not black-box tested) | yes          |
| F-MCPSRV-03  | JSON-RPC envelope, batch, notifications                               | yes          |
| F-MCPSRV-04  | initialize, ping, tools/list (clientInfo caps not black-box tested)   | yes          |
| F-MCPSRV-05  | tools/call, tool definitions (20000-char cap not black-box tested)    | yes          |
| F-MCPSRV-06  | next_question long poll (requeue, wait clamp: not black-box tested)   | yes          |
| F-MCPSRV-07  | answer tool                                                           | yes          |
| F-MCPSRV-08  | annotate tool                                                         | yes          |
| F-MCPSRV-09  | get_questions tool                                                    | yes          |
| F-MCPSRV-10  | files_changed tool                                                    | yes          |
| F-MCPSRV-11  | clients and counters                                                  | yes          |
| F-INTEG-01   | integration catalog, argv                                             | yes          |
| F-INTEG-02   | registration check                                                    | yes          |
| F-INTEG-03   | register                                                              | yes          |
| F-INTEG-04   | unregister                                                            | yes          |
| F-INTEG-05   | register command texts                                                | yes          |
| F-INTEG-06   | watch prompt texts                                                    | yes          |
