# xplain-integrations modules

Pure crate: no IO. Returns `CommandSpec`s, interprets `CommandResult`s, returns texts. Core runs commands,
composes/masks/wraps notes (F-INTEG-03 error notes are core's job).

## Tree

    src/lib.rs       registry `all()` (fixed order: claude, codex, opencode, copilot)
    src/common.rs    cli_command, bearer_header, watch_prompt, parse_url, parse_check, compose_register_note
    src/claude.rs    Claude (trait impl)
    src/codex.rs     Codex
    src/opencode.rs  OpenCode (copy-paste only)
    src/copilot.rs   Copilot

## Ownership

| module | spec IDs | component |
| --- | --- | --- |
| common.rs | F-INTEG-02, F-INTEG-06, UNSPEC-37 | I1 |
| claude.rs, codex.rs, copilot.rs | F-INTEG-01..06, UNSPEC-6 | I1 |
| opencode.rs | F-INTEG-01, 05, 06 | I1 |
| lib.rs | F-MCPUI-01 order | fixed |

## Interfaces

- agent modules call `common::*`; `common` depends only on core types.
- OpenCode: `check_command` None, `register_commands` empty, `unregister_command` None, `parse_check` NotRegistered,
  `register_hint` empty string, `watch_prompt` = `common::watch_prompt(45)` with tool names prefixed `xplain_`.
- `register_hint` returns the FULL note (`<hint>; restart the agent session, then paste the watch prompt`) via
  `common::compose_register_note`.
- `register_commands` = [remove, add]; token appears only in add argv/texts.
