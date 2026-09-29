//! Process-level orchestration: argv -> config -> state -> runtime, plus non-UI exits.
//!
//! Spec: F-CLI-01..05 (help/config path/flag errors exit codes and streams), F-CONFIG-01/03/04 (load, warnings
//! to stderr before UI), F-CLI-05 (Ctrl+C exit 0).
//! Owner: app lead (main component).
//! Must not: contain UI logic. Reads env/files once, prints, builds `Init`, calls the runtime.

/// Full program behavior for `args` (without program name). Returns the exit code.
pub fn main_with_args(_args: &[String]) -> i32 {
    todo!("parse argv, load config, run")
}
