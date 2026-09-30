# xplain

Terminal UI to walk a git diff of your working tree. Move a cursor over lines, select text, write comments and
export them to markdown. An optional local MCP server lets a coding agent long-poll your comments as questions,
answer into the UI, add its own notes and signal file changes.

## Install

```sh
cargo install --path xplain-app
```

Needs a Rust toolchain (stable) and `git` on PATH.

## Usage

```
xplain [--cwd dir] [--config file] [--mode all|staged|unstaged | --staged | --unstaged] [git diff args...]
  --split          start in side-by-side view
  --changes-only   start with git hunks only, not the full file
  --theme <t>      solarized, vibrant, dull, contrast, colorblind, light
  --config <f>     config file (default $XPLAIN_CONFIG or ~/.config/xplain/config.json)
xplain config path print the resolved config path
```

Extra git args replace `HEAD` in `all` mode and are appended in the other modes. Press `?` in the app for all keys.

## Release builds

```sh
cargo xtask dist [--version <v>]   # tarballs + SHA256SUMS in dist/
cargo xtask install                # cargo install --path xplain-app --locked
```

`dist` builds `aarch64-apple-darwin`, `x86_64-apple-darwin`, `universal-apple-darwin` (lipo of both) and static
`x86_64`/`aarch64-unknown-linux-musl` binaries into `dist/xplain-<version>-<target>.tar.gz`, then checks each
binary's architecture and runs `xplain --help`. Version defaults to the one in `xplain-app/Cargo.toml`.

What gets fetched:

- Missing rustup targets are added with `rustup target add`.
- Linux targets link with zig via the `cargo-zigbuild` library. `dist` uses `zig` from PATH when it is version
  0.15.2, otherwise it downloads that release from ziglang.org once into `target/tools/zig-0.15.2/` (sha256
  verified, reused afterwards). The pinned download is macOS aarch64 only.
- macOS targets use the Xcode command line tools (`xcode-select --install`).

## Docs

- [`spec/SPEC.md`](spec/SPEC.md): behavior contract
- [`ARCHITECTURE.md`](ARCHITECTURE.md): design, crates, testing

## Development

Crates:

- `xplain-core`: pure state machine (`update`, `view`), diff parser, config, MCP protocol logic.
- `xplain-integrations`: agent CLI descriptions (claude, codex, copilot, opencode). Pure.
- `xplain-app`: runtime and binary `xplain` (terminal, effects, HTTP server, argv).
- `xplain-sim`: dev-only in-process scenario test harness; see its README.

```sh
cargo run --bin xplain                 # run in the current git repo (use --cwd <dir> for another)
cargo run --bin xplain -- --help
cargo build --release                  # target/release/xplain

cargo test                             # unit tests + scenario tests + spec coverage gate
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

`git` must be on PATH (tests build real repos in temp dirs).

### Tests

- Unit tests live next to the code (`#[cfg(test)]`) in each crate.
- Scenario tests: `xplain-sim/tests/uNN_<area>.rs`. They drive the real core through `Sim`:

  ```rust
  use xplain_sim::Sim;

  #[test]
  fn f_nav_09_example() {
      let mut s = Sim::builder().build();        // standard fixture repo, 120x40
      s.keys("<Tab>j");                          // keys, settled after each
      s.assert_row_contains(0, "src/big.ts");
  }
  ```

  Name the test `f_<group>_<nn>_<what>` so the spec ID `F-<GROUP>-<NN>` counts as covered. Builder options, key
  notation and assertions: [`xplain-sim/README.md`](xplain-sim/README.md). Fixture repos:
  `xplain-sim/fixtures/base` (HEAD) and `xplain-sim/fixtures/work` (working tree).
- Spec coverage gate: `xplain-sim/tests/spec_coverage.rs` fails when an in-scope ID of `spec/SPEC.md` has no test
  (fn `f_<group>_<nn>_...` or a `// covers: F-X-NN` comment). Known gaps go in `xplain-sim/tests/app_pending.txt`.

## License

See [`LICENSE`](LICENSE).
