# xplain

Terminal UI to walk a git diff of your working tree. Move a cursor over lines, select text, write comments and
export them to markdown. An optional local MCP server lets a coding agent long-poll your comments as questions,
answer into the UI, add its own notes and signal file changes.

## Install

```sh
cargo install --path rust/xplain-app
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

## Docs

- [`spec/SPEC.md`](spec/SPEC.md): behavior contract
- [`ARCHITECTURE.md`](ARCHITECTURE.md): design, crates, testing
- [`rust/README.md`](rust/README.md): build and test

## License

See [`LICENSE`](LICENSE).
