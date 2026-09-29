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

use crate::env::RawEnv;

/// Result of startup that needs no UI, or the UI's initial inputs.
pub enum Startup {
    /// Non-UI outcome: help, `config path`, flag error. Text already final (with trailing newlines).
    Exit { code: i32, stdout: String, stderr: String },
    /// Start the UI: warnings go to stderr first (each line + `\n`), then run the loop.
    Ui { state: Box<State>, effects: Vec<Effect>, warnings: Vec<String>, sync: bool, truecolor: bool },
}

/// Pure-ish startup: parse argv (`cli::parse_args`), resolve config path (`xplain_core::config`), read the
/// config via `read_config`, `load_config`, build `EnvInfo`/`Init`, `State::new(init, xplain_integrations::all())`.
/// `abs_cwd` = process cwd (absolute); `--cwd` joined onto it. No printing, no process exit.
pub fn prepare(
    _args: &[String],
    _env: &RawEnv,
    _abs_cwd: &str,
    _size: Size,
    _read_config: &dyn Fn(&str) -> ConfigFile,
) -> Startup {
    todo!("F-CLI-01..05, F-CONFIG-01/03/04")
}

/// Full program behavior for `args` (without program name). Returns the exit code. Builds the tokio runtime,
/// calls [`prepare`], prints [`Startup::Exit`] texts or warnings to the given streams, runs
/// `runtime::run_loop`, always restores the terminal.
pub fn main_with_args(_args: &[String]) -> i32 {
    todo!("parse argv, load config, run")
}

/// Writes non-UI output; split out so tests can capture it.
pub fn emit(_startup_stdout: &str, _startup_stderr: &str, _out: &mut dyn Write, _err: &mut dyn Write) {
    todo!("write + flush")
}
