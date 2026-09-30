# xplain-sim modules

Dev-only harness (see `README.md`). Depends on `xplain-core`, `xplain-integrations`, and `xplain-app` for its pure
startup helpers only (`cli`, `env`, `run::prepare`, `config_io::read_config_file`). Nothing depends on it.

## Tree

    src/lib.rs      registry and re-exports
    src/sim.rs      Sim + SimBuilder: state, effect dispatch, settle, manual clock, fake CLIs, in-process HTTP, assertions
    src/exec.rs     synchronous real IO: git (hermetic env), fs read/write, config save, token file
    src/fixture.rs  TempRoot, fixture repos (standard cached + copied), hermetic git command
    src/keys.rs     key notation parser
    src/http.rs     Http request builder, HttpReply, Pending
    src/shim.rs     Rule, Call: scripted fake integration CLIs
    src/view.rs     rows, cells, colors, text search, CellExpect
    tests/          harness.rs (self tests), uNN_<area>.rs ported scenarios

## Rules

- No agent names in `src/`; tests use names only where a scenario is inherently about one CLI, otherwise take
  them from `xplain_integrations::all()`.
- Panics are the assertion mechanism (`#[track_caller]`), so the crate allows `clippy::panic/unwrap/expect`.
- No sleeps, no threads, no network. Real work is git and fs in temp dirs only.
