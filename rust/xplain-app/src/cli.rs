//! Hand-written argv parser and usage/error texts.
//!
//! Spec: F-CLI-01..04, F-CLI-06 (verbatim usage text, error messages, value consumption, `=` forms,
//! `config path`). Owner: app lead (cli component).
//! Must not: read env or files, print, or exit. Pure so it is unit-testable; `run` does the printing.

use xplain_core::options::Options;

/// Outcome of parsing argv (without program name).
#[derive(Debug, Clone, PartialEq)]
pub enum Cli {
    /// Start the UI.
    Run(Options),
    /// `-h`/`--help`: print [`USAGE`] + `\n` to stdout, exit 0.
    Help,
    /// `xplain [--config f] config path`: print resolved config path, exit 0. Carries the `--config` value.
    ConfigPath { config_flag: Option<String> },
    /// Flag error: stderr `xplain: <msg>\n<usage>\n`, exit 1. Holds `<msg>`.
    Error(String),
}

/// Usage text, verbatim from F-CLI-01 (no trailing newline).
pub const USAGE: &str = "usage: xplain [--cwd dir] [--config file] [--mode all|staged|unstaged | --staged | --unstaged] [git diff args...]\n\
  --mode <m>   all (git diff HEAD, default), staged (--cached), unstaged\n\
  --staged     same as --mode staged\n\
  --unstaged   same as --mode unstaged\n\
  --split      start in side-by-side view (s toggles)\n\
  --changes-only  start with git hunks only, not the full file (c toggles)\n\
  --theme <t>  solarized, vibrant, dull, contrast, colorblind, light (first is default) (t cycles)\n\
  --config <f> config file (default $XPLAIN_CONFIG or ~/.config/xplain/config.json)\n\
  -h, --help   show this help\n\
xplain config path  print the resolved config path\n\
extra git args replace HEAD in \"all\" mode, and are appended in the other modes.\n\
keys: ? help, s split/unified, c full/changes, ]/[ next/prev change, m cycles mode, t cycles theme, C config, q quits";

pub fn parse_args(_args: &[String]) -> Cli {
    todo!("F-CLI-01..04, F-CLI-06")
}
