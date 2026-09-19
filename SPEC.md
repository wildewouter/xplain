# xplain

Terminal diff reviewer (Ink 7, TypeScript 7, Node >= 22): browse a git diff, comment on lines or selections, optionally have a coding agent answer them over MCP.

## Run

| Command                       | Purpose                                                                                 |
| ----------------------------- | --------------------------------------------------------------------------------------- |
| `npm install`                 | install deps                                                                            |
| `npm run dev [-- flags]`      | run from source (`tsx src/cli.tsx`)                                                     |
| `npm run build` / `npm start` | compile to `dist/` / run `dist/cli.js`                                                  |
| `npm test`                    | `tests/run.sh`: builds a fixture git repo, runs every `tests/*.test.ts(x)` concurrently |

Usage: `xplain [--cwd dir] [--config file] [--mode all|staged|unstaged | --staged | --unstaged] [--split] [--changes-only] [--theme t] [git diff args...]`

- `--mode`: `all` = `git diff HEAD` (default), `staged` = `--cached`, `unstaged` = plain `git diff`.
- Extra git args replace `HEAD` in `all` mode and are appended in the other modes.
- `--split` starts side by side; `--changes-only` starts with hunks only; `--theme` one of `solarized` (default), `vibrant`, `dull`, `contrast`, `colorblind`, `light`.
- `--config <file>` overrides `$XPLAIN_CONFIG`; `xplain config path` prints the resolved config path; `-h/--help`.

## Views

- Diff source: mode `all` / `staged` / `unstaged` (`m` cycles).
- Layout: unified or split (side-by-side, old | new).
- Scope: full file (whole file with changes marked, `-U1000000`) or changes only (git hunks).
- Browse: any tracked or untracked-unignored file shown read-only, opened from search (`F`). Uses the new-side pane only.
- Themes: six, `t` cycles.
- Header chips: `[mode]` `[full|changes]` `[split|unified]` `[theme]` `[mcp: on|off]` `[n/total]` file index; `[browse]` replaces mode/scope/layout in browse; `[side Lnn:Ccc]` (or `rNN` when no line number) shows the cursor in cursor mode; then path (`old -> new` for renames), `+adds -dels`.

## Keys

`n` = optional count prefix (digits, cursor mode only). Arrows mirror hjkl; PageUp/PageDown/Space page. Ctrl combos are ignored outside the ask box and search.

### Main

| Key                      | Action                                    |
| ------------------------ | ----------------------------------------- |
| `j` `k` / down up        | scroll one line                           |
| `d` `u`                  | half page down / up                       |
| Space, PageDown / PageUp | page down / up                            |
| `g` `G`                  | top / bottom                              |
| `]` `[`                  | next / previous change                    |
| Tab, right / S-Tab, left | next / previous file                      |
| `m`                      | cycle mode all, staged, unstaged          |
| `s`                      | toggle split / unified                    |
| `c`                      | toggle full file / changes only           |
| `t`                      | cycle theme                               |
| `C`                      | config modal                              |
| `M`                      | MCP modal                                 |
| `E`                      | export comments to markdown               |
| `?`                      | help                                      |
| `q`                      | quit (confirm modal if `app.confirmQuit`) |

### Files

| Key          | Action                                                                   |
| ------------ | ------------------------------------------------------------------------ |
| `f`          | file picker of changed files (status A/M/D/R, +/-)                       |
| `F`          | fuzzy file search (tracked + untracked) and open in browse               |
| Esc (browse) | leave browse; `n f p c s m [ ]` and Tab/left/right are ignored in browse |

Picker: `j/k` up/down move, `d/u` half page, Enter open, Esc/`q`/`f` close. Search: type to filter, up/down or Ctrl-n/Ctrl-p move, Enter open in browse, Esc close.

### Cursor mode

`i` toggles cursor mode (works from any state, also leaves it). Esc order: leave comment focus, then end selection, then leave cursor mode.

| Key                      | Action                                                        |
| ------------------------ | ------------------------------------------------------------- |
| `h` `l`                  | column left / right (`n`h)                                    |
| `0` `^` `$`              | line start / first non-blank / end                            |
| `w` `b` `e`              | word forward / back / end                                     |
| `j` `k`                  | line down / up (`n`j)                                         |
| `d` `u`                  | half page down / up (`n` times)                               |
| Space, PageDown / PageUp | page                                                          |
| `g` `G`                  | first row / last row (`nG` goes to row n)                     |
| `]` `[`                  | next / previous change                                        |
| `p`                      | switch split pane old / new (split view only; ends selection) |
| `v`                      | characterwise selection (again to end)                        |
| `V`                      | linewise selection (again to end)                             |
| Enter, `a`               | comment on the line or selection                              |
| `J` `K`                  | focus next / previous comment                                 |

Global view keys (`s c m t f F C M E ? q`, Tab) still work in cursor mode.

### Comments

Ask box (typing a comment):

| Key                     | Action                                                                |
| ----------------------- | --------------------------------------------------------------------- |
| Enter                   | send (empty is ignored)                                               |
| Tab                     | toggle save / ask (only while MCP runs; otherwise notes "MCP is off") |
| left / right, Backspace | edit text                                                             |
| Esc                     | cancel                                                                |

Focused comment (after `J`/`K`):

| Key          | Action                                                                                    |
| ------------ | ----------------------------------------------------------------------------------------- |
| `e`, Enter   | edit message (Enter sends; no save/ask toggle); not after follow-ups                      |
| `D`          | delete (confirm `y`/Enter, `n`/Esc)                                                       |
| `a`          | saved / errored / cancelled: ask now (MCP on). Answered: open follow-up input. Live: note |
| `j` `k`      | scroll the thread one line (only when it overflows; else unfocus and move)                |
| `d` `u`      | scroll half the visible thread                                                            |
| `g` `G`      | top / bottom of the thread (`G` resumes tail-follow)                                      |
| `A`          | ask all askable comments                                                                  |
| `J` `K`      | next / previous comment                                                                   |
| Esc          | back to cursor                                                                            |
| other motion | unfocus and move                                                                          |

Follow-up input (`a` on an answered comment): header `follow-up`, Enter sends (empty ignored; MCP off or refused keeps the box and notes it), Esc cancels. No save/ask toggle.

### Modals

| Modal  | Keys                                                                                                                                                                                                                                                              |
| ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Help   | `?` / Esc / `q` close                                                                                                                                                                                                                                             |
| Quit   | `y`, Enter quit; `n`, `q`, Esc cancel                                                                                                                                                                                                                             |
| Config | `j/k` row, `h/l` choice, Enter or Space apply and persist, Esc/`q`/`C` close                                                                                                                                                                                      |
| MCP    | `j/k` row (row 0 = server), Enter/Space start or stop server, Enter register in selected agent, `d` unregister, `c` copy register command, `w` copy watch prompt, `R` refresh registration state, Esc/`q`/`M` close; register/unregister ask `y`/Enter or `n`/Esc |

## Comments

- Anchor: file + side (`old` | `new`; unified and browse are always `new`) + line, or a selection (start/end line, 1-based inclusive columns, selected text). Sent with surrounding code lines as context.
- Origin: human, or `agent` (created through the `annotate` MCP tool; cannot be asked).
- States: saved (stored, never asked; shown "saved · not asked"), asked (`pending`, then `streaming` once an agent picked it up), answered (`done`), `error`, `cancelled` (MCP stopped). Errored or cancelled comments can be asked again.
- Threads: a comment is a list of turns (message + answer). An answered human comment takes follow-ups (`a`); each is a new turn, answered by the same agent thread. A thread shows all turns in order (`follow-up: ...`, then its `answer · agent · status`). Unfocused threads are capped (`… +N more`); a focused thread uses the viewport and scrolls (`↕ from-to/total` shown only on overflow). While the latest turn is pending or streaming and the user has not scrolled up, the focused thread shows its tail. Scroll offsets are per comment. Threads cannot be edited after a follow-up; delete removes the whole thread.
- Send mode: `save` (MCP off, or chosen with Tab) or `ask` (default while MCP runs). Comments can be edited, deleted, and asked later with `a` / `A`.
- Comments live in memory only; they are lost on exit.

## Export

`E` (main view, cursor mode and browse; not while typing or in a modal) writes all comments and full threads to `xplain-review-<YYYYMMDD-HHMMSS>.md` in the `--cwd` dir (else the process cwd) and notes `exported N comments -> path` or `export failed: ...`. With no comments it writes nothing and notes `no comments to export`.

Content: title, repo, diff mode and args, date, comment count; then comments grouped by file (sorted by path, then line). Each comment: side and line or selection range (with columns), origin (human/agent), state (saved, answered, pending, streaming, error, cancelled), selected text and context as fenced code (fence longer than any backtick run inside), then every turn in order: comment, its answer (`Answer (agent) - status`), then `Follow-up n` and its answer.

## MCP (optional, off by default)

Purpose: let an external coding agent answer questions asked in the UI. xplain runs a local MCP server the agent long-polls.

- Server: streamable HTTP at `http://127.0.0.1:47615/mcp` (port from `mcp.json` if set). Started from the MCP modal, or at launch with `mcp.autostart`.
- Auth: bearer token (32 random bytes, base64url) in `~/.local/state/xplain/mcp.json` (`$XDG_STATE_HOME/xplain/`), file mode 0600, dir 0700. Created on first start and reused.

| Tool            | Args                                                      | Behavior                                                                                                                                                               |
| --------------- | --------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `next_question` | `wait_seconds` 1..120, default 45                         | long-poll; returns `question` (with `thread_id`, `turn`, `follow_up`, `question`; follow-ups add `previous` turns), `no_question_yet` (call again), or `closed` (stop) |
| `answer`        | `thread_id`, `text`                                       | delivers the answer; error on unknown thread                                                                                                                           |
| `get_questions` | none                                                      | lists undelivered questions (200-char preview), does not consume                                                                                                       |
| `annotate`      | `file`, `line`, `text`, `side` (`old`/`new`, default new) | adds an agent comment at that line                                                                                                                                     |

Workflow: start MCP (`M`) -> register the agent (Enter, or `c` to copy the command) -> restart or resume the agent session -> paste the watch prompt (`w`) -> agent loops `next_question` / `answer` until `closed`.

| id         | Register                                                                             | Poll s | Restart                     |
| ---------- | ------------------------------------------------------------------------------------ | ------ | --------------------------- |
| `claude`   | auto: `claude mcp add ... --scope local`                                             | 100    | yes (`claude --resume`)     |
| `codex`    | auto: `codex mcp add --bearer-token-env-var XPLAIN_MCP_TOKEN`; token exported in env | 45     | yes (`codex resume --last`) |
| `opencode` | manual: `c` copies an `opencode.json` snippet (tools prefixed `xplain_`)             | 45     | yes                         |
| `copilot`  | auto: `copilot mcp add ... --timeout 200000`                                         | 120    | yes (or `/mcp`)             |

Security:

- Binds loopback only; requests need matching loopback `Host`, loopback or absent `Origin` (else 403), and the bearer token (else 401).
- Question, answer and annotation text is stripped of ANSI/control characters and capped at 20000 chars.
- Tokens are masked in error output. Tools never touch the repo; they only read questions and add answers or annotations in the UI.

## Config

Path: `--config`, else `$XPLAIN_CONFIG`, else `$XDG_CONFIG_HOME/xplain/config.json` (default `~/.config/xplain/config.json`). JSON; missing file means defaults; invalid values warn on stderr and fall back per key; an unparseable file is left untouched and never written. Changes made in the config modal apply live and are merged into the file atomically.

| Key               | Values                                                | Default     |
| ----------------- | ----------------------------------------------------- | ----------- |
| `theme`           | solarized, vibrant, dull, contrast, colorblind, light | `solarized` |
| `view.mode`       | all, staged, unstaged                                 | `all`       |
| `view.split`      | boolean                                               | `false`     |
| `view.full`       | boolean                                               | `true`      |
| `app.confirmQuit` | boolean                                               | `true`      |
| `mcp.autostart`   | boolean                                               | `false`     |

`version: 1`; `agent` and `keys` are reserved. Precedence: defaults < config file < CLI flags.

## Architecture

- `src/cli.tsx` flags and render; `src/app.tsx` state and all key handling; `src/keys.ts` help table and footer; `src/settings.ts` config-modal rows; `src/config.ts`, `src/defaults.ts`, `src/theme.ts`.
- `src/diff/` git diff loading and parsing, file listing; `src/components/` Ink views and modals.
- `src/ask/` headless comment/answer store (`createAskController`); no react/ink imports (enforced by test). `src/ask/export.ts` renders the review markdown (`renderReviewMarkdown`); `app.tsx` writes the file.
- `src/mcp/` hub (queue, long-poll), server (HTTP/JSON-RPC), token, tools, bridge (connects hub to the ask controller); `src/useAsk.ts`, `src/useMcp.ts` are the UI hooks.
- `src/integrations/` all agent-specific code, behind the `AgentIntegration` interface (`id`, `label`, `pollSeconds`, `needsRestart`, `canRegister`, `registerCommand`, `watchPrompt`, `register`/`unregister`/`isRegistered`). Agent names must not appear outside it (enforced by `tests/architecture.test.ts`).
- `tests/`: `*.test.ts(x)` (config, ask, controller, export, bridge, mcp, integrations, key handling via ink-testing-library, architecture); `fixture/base` and `fixture/work` seed the temp git repo.

## Non-goals and known gaps

- Comments are not persisted across runs.
- Follow-ups only after a `done` answer (no retry of a failed follow-up); agent notes take none.
- Untracked files do not appear in the diff (only in search/browse).
- Agents must be restarted or resumed after registering; only OpenCode has been tested against a real agent.
- No editing of repository files; `keys` config is reserved, not implemented.
