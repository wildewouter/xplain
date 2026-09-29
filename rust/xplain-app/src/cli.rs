//! Hand-written argv parser and usage/error texts.
//!
//! Spec: F-CLI-01..04, F-CLI-06 (verbatim usage text, error messages, value consumption, `=` forms,
//! `config path`). Owner: component B (startup).
//! Rules to implement: left to right; `-h`/`--help` wins only if no earlier flag error; space-form values are
//! taken verbatim (F-CLI-06); `config path` only when the remaining non-flag args are exactly `config path`.
//! Must not: read env or files, print, or exit. Pure so it is unit-testable; `run` does the printing.

use xplain_core::options::{DiffMode, Options};
use xplain_core::theme::ThemeId;

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

const THEME_LIST: &str = "solarized|vibrant|dull|contrast|colorblind|light";

fn set_mode(v: Option<&str>) -> Result<DiffMode, Cli> {
    v.and_then(DiffMode::parse)
        .ok_or_else(|| Cli::Error(format!("invalid mode: {}", v.unwrap_or("(missing)"))))
}

fn set_theme(v: Option<&str>) -> Result<ThemeId, Cli> {
    v.and_then(ThemeId::parse)
        .ok_or_else(|| Cli::Error(format!("invalid theme: {} ({THEME_LIST})", v.unwrap_or("(missing)"))))
}

fn non_empty(v: &str) -> Option<String> {
    (!v.is_empty()).then(|| v.to_string())
}

/// Parse argv (without program name), left to right (F-CLI-01..04, F-CLI-06).
pub fn parse_args(args: &[String]) -> Cli {
    match parse_inner(args) {
        Ok(c) | Err(c) => c,
    }
}

fn parse_inner(args: &[String]) -> Result<Cli, Cli> {
    let mut o = Options::default();
    let mut rest: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        let mut next = || {
            i += 1;
            args.get(i).map(String::as_str)
        };
        match a {
            "-h" | "--help" => return Ok(Cli::Help),
            "--cwd" => match next() {
                Some(v) => o.cwd = Some(v.to_string()),
                None => return Err(Cli::Error("--cwd needs a value".into())),
            },
            "--config" => match next() {
                Some(v) => o.config_path = non_empty(v),
                None => return Err(Cli::Error("--config needs a value".into())),
            },
            "--mode" => o.mode = Some(set_mode(next())?),
            "--theme" => o.theme = Some(set_theme(next())?),
            "--split" => o.split = Some(true),
            "--changes-only" => o.full = Some(false),
            "--staged" => o.mode = Some(DiffMode::Staged),
            "--unstaged" => o.mode = Some(DiffMode::Unstaged),
            _ => {
                if let Some(v) = a.strip_prefix("--config=") {
                    o.config_path = non_empty(v);
                } else if let Some(v) = a.strip_prefix("--mode=") {
                    o.mode = Some(set_mode(Some(v))?);
                } else if let Some(v) = a.strip_prefix("--theme=") {
                    o.theme = Some(set_theme(Some(v))?);
                } else {
                    rest.push(a.to_string());
                }
            }
        }
        i += 1;
    }
    if rest.len() == 2 && rest[0] == "config" && rest[1] == "path" {
        return Ok(Cli::ConfigPath { config_flag: o.config_path });
    }
    o.git_args = rest;
    Ok(Cli::Run(o))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(a: &[&str]) -> Cli {
        parse_args(&a.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }
    fn err(a: &[&str]) -> String {
        match p(a) {
            Cli::Error(m) => m,
            other => panic!("not error: {other:?}"),
        }
    }
    fn run(a: &[&str]) -> Options {
        match p(a) {
            Cli::Run(o) => o,
            other => panic!("not run: {other:?}"),
        }
    }

    #[test]
    fn help() {
        assert_eq!(p(&["-h"]), Cli::Help);
        assert_eq!(p(&["x", "--help"]), Cli::Help);
        assert_eq!(err(&["--cwd"]), "--cwd needs a value");
        assert_eq!(err(&["--mode", "x", "-h"]), "invalid mode: x");
        assert_eq!(p(&["--split", "-h", "--mode", "x"]), Cli::Help);
    }

    #[test]
    fn usage_theme_list_matches() {
        assert!(USAGE.contains("solarized, vibrant, dull, contrast, colorblind, light (first"));
        assert!(!USAGE.ends_with('\n'));
    }

    #[test]
    fn errors() {
        assert_eq!(err(&["--config"]), "--config needs a value");
        assert_eq!(err(&["--mode"]), "invalid mode: (missing)");
        assert_eq!(err(&["--mode=bad"]), "invalid mode: bad");
        assert_eq!(err(&["--mode="]), "invalid mode: ");
        assert_eq!(err(&["--theme"]), format!("invalid theme: (missing) ({THEME_LIST})"));
        assert_eq!(err(&["--theme", "x"]), format!("invalid theme: x ({THEME_LIST})"));
        assert_eq!(err(&["--theme="]), format!("invalid theme:  ({THEME_LIST})"));
        assert_eq!(err(&["--mode", "--staged"]), "invalid mode: --staged");
    }

    #[test]
    fn flags() {
        let o = run(&[
            "--cwd",
            "d",
            "--config",
            "c",
            "--split",
            "--changes-only",
            "--theme=light",
            "--mode",
            "staged",
        ]);
        assert_eq!(o.cwd.as_deref(), Some("d"));
        assert_eq!(o.config_path.as_deref(), Some("c"));
        assert_eq!(o.split, Some(true));
        assert_eq!(o.full, Some(false));
        assert_eq!(o.theme, Some(ThemeId::Light));
        assert_eq!(o.mode, Some(DiffMode::Staged));
        assert!(o.git_args.is_empty());
        assert_eq!(run(&[]), Options::default());
    }

    #[test]
    fn last_mode_wins() {
        assert_eq!(run(&["--staged", "--unstaged"]).mode, Some(DiffMode::Unstaged));
        assert_eq!(run(&["--unstaged", "--mode=all"]).mode, Some(DiffMode::All));
        assert_eq!(run(&["--mode", "staged", "--unstaged", "--staged"]).mode, Some(DiffMode::Staged));
    }

    #[test]
    fn verbatim_values() {
        let o = run(&["--cwd", "-h"]);
        assert_eq!(o.cwd.as_deref(), Some("-h"));
        assert_eq!(run(&["--config", "--split"]).config_path.as_deref(), Some("--split"));
    }

    #[test]
    fn empty_config_is_none() {
        assert_eq!(run(&["--config="]).config_path, None);
        assert_eq!(run(&["--config", ""]).config_path, None);
        assert_eq!(run(&["--config", "a", "--config="]).config_path, None);
        assert_eq!(run(&["--config=a", "--config=b"]).config_path.as_deref(), Some("b"));
    }

    #[test]
    fn passthrough() {
        let o = run(&["--cwd=x", "--split=x", "--staged=x", "--", "--foo", "HEAD~1", "a.txt"]);
        assert_eq!(o.cwd, None);
        assert_eq!(o.split, None);
        assert_eq!(o.mode, None);
        assert_eq!(o.git_args, ["--cwd=x", "--split=x", "--staged=x", "--", "--foo", "HEAD~1", "a.txt"]);
    }

    #[test]
    fn config_path() {
        assert_eq!(p(&["config", "path"]), Cli::ConfigPath { config_flag: None });
        assert_eq!(
            p(&["--config", "/x", "config", "path"]),
            Cli::ConfigPath { config_flag: Some("/x".into()) }
        );
        assert_eq!(p(&["config", "--split", "path"]), Cli::ConfigPath { config_flag: None });
        assert_eq!(run(&["config", "path", "extra"]).git_args, ["config", "path", "extra"]);
        assert_eq!(run(&["config"]).git_args, ["config"]);
        assert_eq!(run(&["path", "config"]).git_args, ["path", "config"]);
    }
}
