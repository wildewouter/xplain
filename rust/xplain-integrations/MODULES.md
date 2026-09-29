# xplain-integrations modules

Pure crate: no IO. Returns `CommandSpec`s, interprets `CommandResult`s, returns texts. Core runs commands,
composes/masks/wraps notes (F-INTEG-03 error notes are core's job).

## Tree

    src/lib.rs       registry `all()` (fixed order: claude, codex, opencode, copilot)
    src/common.rs    cli_command, bearer_header, watch_prompt(poll, prefix), parse_url, parse_check,
                     compose_register_note, cfg(test) testutil (ep, ok, registration)
    src/cli.rs       CliAgent: one table-driven type implementing AgentIntegration + CliRegistration
    src/claude.rs    claude::agent() (table row)
    src/codex.rs     codex::agent()
    src/opencode.rs  OpenCode (copy-paste only: implements AgentIntegration, no registration)
    src/copilot.rs   copilot::agent()

## Ownership

| module                          | spec IDs                          | component |
| ------------------------------- | --------------------------------- | --------- |
| common.rs                       | F-INTEG-02, F-INTEG-06, UNSPEC-37 | I1        |
| cli.rs, claude.rs, codex.rs, copilot.rs | F-INTEG-01..06, UNSPEC-6  | I1        |
| opencode.rs                     | F-INTEG-01, 05, 06                | I1        |
| lib.rs                          | F-MCPUI-01 order                  | fixed     |

## Interfaces

- agent modules call `common::*`; `common` depends only on core types.
- OpenCode: `registration()` is None (default), so no check/register/unregister; `watch_prompt` =
  `common::watch_prompt(45, "xplain_")`.
- registrable agents: `registration()` returns `Some(&dyn CliRegistration)` (core trait); `lib.rs` has one
  registry-level invariant test over `all()`.
- `register_hint` returns the FULL note (`<hint>; restart the agent session, then paste the watch prompt`) via
  `common::compose_register_note`.
- `register_commands` = [remove, add]; token appears only in add argv/texts.
