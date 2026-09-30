//! Process-level orchestration: argv -> config -> state -> runtime, plus non-UI exits.
//!
//! Spec: F-CLI-01..05 (help/config path/flag errors exit codes and streams), F-CONFIG-01/03/04 (load, warnings
//! to stderr before UI), F-CLI-05 (Ctrl+C exit 0).
//! Owner: component B (startup).
//! Must not: contain UI logic. Reads env/files once, prints, builds `Init`, calls the runtime.

use std::io::Write;

use xplain_core::Effect;
use xplain_core::State;
use xplain_core::config::ConfigFile;
use xplain_core::screen::Size;

use std::path::Path;

use xplain_core::config::{load_config, resolve_config_path};
use xplain_core::options::Init;

use crate::cli::{self, Cli, USAGE};
use crate::config_io;
use crate::env::RawEnv;
use crate::runtime::{self, RuntimeConfig};
use crate::term;

/// Result of startup that needs no UI, or the UI's initial inputs.
pub enum Startup {
    /// Non-UI outcome: help, `config path`, flag error. Text already final (with trailing newlines).
    Exit { code: i32, stdout: String, stderr: String },
    /// Start the UI: warnings go to stderr first (each line + `\n`), then run the loop.
    Ui { state: Box<State>, effects: Vec<Effect>, warnings: Vec<String>, truecolor: bool },
}

/// Pure-ish startup: parse argv (`cli::parse_args`), resolve config path (`xplain_core::config`), read the
/// config via `read_config`, `load_config`, build `EnvInfo`/`Init`, `State::new(init, xplain_integrations::all())`.
/// `abs_cwd` = process cwd (absolute); `--cwd` joined onto it. No printing, no process exit.
pub fn prepare(
    args: &[String],
    env: &RawEnv,
    abs_cwd: &str,
    size: Size,
    read_config: &dyn Fn(&str) -> ConfigFile,
) -> Startup {
    let options = match cli::parse_args(args) {
        Cli::Help => return Startup::Exit { code: 0, stdout: format!("{USAGE}\n"), stderr: String::new() },
        Cli::Error(msg) => {
            return Startup::Exit {
                code: 1,
                stdout: String::new(),
                stderr: format!("xplain: {msg}\n{USAGE}\n"),
            };
        }
        Cli::ConfigPath { config_flag } => {
            let path = resolve_config_path(&env.config_path_env(config_flag.as_deref()));
            return Startup::Exit { code: 0, stdout: format!("{path}\n"), stderr: String::new() };
        }
        Cli::Run(o) => o,
    };
    let config_path = resolve_config_path(&env.config_path_env(options.config_path.as_deref()));
    let file = read_config(&config_path);
    let loaded = load_config(&config_path, &file);
    let cwd = match options.cwd.as_deref() {
        Some(c) => Path::new(abs_cwd).join(c).to_string_lossy().into_owned(),
        None => abs_cwd.to_string(),
    };
    let init = Init { options, config: loaded.config, env: env.env_info(cwd, config_path), size };
    let (state, effects) = State::new(init, xplain_integrations::all());
    Startup::Ui { state: Box::new(state), effects, warnings: loaded.warnings, truecolor: env.truecolor() }
}

/// Full program behavior for `args` (without program name). Returns the exit code. Builds the tokio runtime,
/// calls [`prepare`], prints [`Startup::Exit`] texts or warnings to the given streams, runs
/// `runtime::run_loop`, always restores the terminal.
pub fn main_with_args(args: &[String]) -> i32 {
    let env = RawEnv::from_process();
    let abs_cwd = std::env::current_dir().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
    let startup = prepare(args, &env, &abs_cwd, term::size(), &config_io::read_config_file);
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    match startup {
        Startup::Exit { code, stdout: out, stderr: err } => {
            emit(&out, &err, &mut stdout, &mut stderr);
            code
        }
        Startup::Ui { state, effects, warnings, truecolor } => {
            let text: String = warnings.iter().map(|w| format!("{w}\n")).collect();
            emit("", &text, &mut stdout, &mut stderr);
            let rt = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
                Ok(rt) => rt,
                Err(_) => {
                    emit("", "xplain: cannot start runtime\n", &mut stdout, &mut stderr);
                    return 1;
                }
            };
            term::install_panic_hook();
            rt.block_on(runtime::run_loop(
                *state,
                effects,
                RuntimeConfig {
                    truecolor,
                    keylog: std::env::var("XPLAIN_KEYLOG").ok().filter(|s| !s.is_empty()),
                },
            ))
        }
    }
}

/// Writes non-UI output; split out so tests can capture it.
pub fn emit(startup_stdout: &str, startup_stderr: &str, out: &mut dyn Write, err: &mut dyn Write) {
    if !startup_stdout.is_empty() {
        let _ = out.write_all(startup_stdout.as_bytes());
    }
    let _ = out.flush();
    if !startup_stderr.is_empty() {
        let _ = err.write_all(startup_stderr.as_bytes());
    }
    let _ = err.flush();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn a(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }
    fn env() -> RawEnv {
        RawEnv { home: Some("/h".into()), ..RawEnv::default() }
    }
    fn size() -> Size {
        Size { cols: 80, rows: 24 }
    }

    #[test]
    fn help_and_error() {
        let read = |_: &str| -> ConfigFile { panic!("no config read") };
        match prepare(&a(&["-h"]), &env(), "/w", size(), &read) {
            Startup::Exit { code, stdout, stderr } => {
                assert_eq!(code, 0);
                assert_eq!(stdout, format!("{USAGE}\n"));
                assert!(stderr.is_empty());
            }
            _ => panic!("not exit"),
        }
        match prepare(&a(&["--mode", "x"]), &env(), "/w", size(), &read) {
            Startup::Exit { code, stdout, stderr } => {
                assert_eq!(code, 1);
                assert!(stdout.is_empty());
                assert_eq!(stderr, format!("xplain: invalid mode: x\n{USAGE}\n"));
            }
            _ => panic!("not exit"),
        }
    }

    #[test]
    fn config_path_no_read() {
        let read = |_: &str| -> ConfigFile { panic!("no config read") };
        match prepare(&a(&["--config", "/x/c.json", "config", "path"]), &env(), "/w", size(), &read) {
            Startup::Exit { code, stdout, stderr } => {
                assert_eq!(code, 0);
                assert_eq!(stdout, "/x/c.json\n");
                assert!(stderr.is_empty());
            }
            _ => panic!("not exit"),
        }
        match prepare(&a(&["config", "path"]), &env(), "/w", size(), &read) {
            Startup::Exit { stdout, .. } => assert_eq!(stdout, "/h/.config/xplain/config.json\n"),
            _ => panic!("not exit"),
        }
    }

    #[test]
    fn ui_reads_config_once() {
        let calls = Cell::new(0);
        let read = |p: &str| -> ConfigFile {
            calls.set(calls.get() + 1);
            assert_eq!(p, "/c.json");
            ConfigFile::Text("{oops".into())
        };
        match prepare(&a(&["--config=/c.json", "--cwd", "sub"]), &env(), "/w", size(), &read) {
            Startup::Ui { warnings, truecolor, .. } => {
                assert_eq!(warnings.len(), 1);
                assert!(warnings[0].starts_with("xplain: config: /c.json"));
                assert!(!truecolor);
            }
            _ => panic!("not ui"),
        }
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn emit_writes() {
        let (mut o, mut e) = (Vec::new(), Vec::new());
        emit("out\n", "err\n", &mut o, &mut e);
        assert_eq!(o, b"out\n");
        assert_eq!(e, b"err\n");
    }
}
