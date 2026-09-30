# xplain (Rust rewrite)

Workspace of three crates plus a dev-only test harness. Design and rules: [`../ARCHITECTURE.md`](../ARCHITECTURE.md). Behavior contract:
[`../spec/SPEC.md`](../spec/SPEC.md). Parity gate: `XPLAIN_BIN=$PWD/target/release/xplain npm run e2e` from the repo root.

```sh
cd rust
cargo build            # skeleton compiles; bodies are todo!()
cargo test
cargo clippy --all-targets
cargo fmt --check
```

- `xplain-core`: pure state machine (`update`, `view`), diff parser, config, MCP protocol logic.
- `xplain-integrations`: agent CLI descriptions (claude, codex, copilot, opencode). Pure.
- `xplain-app`: runtime and binary `xplain` (terminal, effects, HTTP server, argv).
- `xplain-sim`: dev-only in-process scenario test harness (keys, manual clock, real git/fs in temp dirs); see its README.

Skeleton status: every boundary type and entry function exists; bodies are `todo!()`. Crate leads fill in inner
modules under their crate; workspace-level files (`Cargo.toml`, `lib.rs` of each crate) are shared registries and
change only through the crate lead.
