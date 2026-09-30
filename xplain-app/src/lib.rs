//! xplain-app: the runtime around `xplain-core`. Owns every side effect: terminal, git and fs, process
//! spawning, clock/timers, OSC 52, the MCP HTTP server, argv.
//!
//! Spec: CLI (F-CLI-01..06), Environment (MCP port), F-MODE-01/02/04 (execution
//! side), F-CONFIG-05 (write side), F-EXPORT-01 (write side), F-MCPSRV-01/02 (socket side), F-INTEG-*
//! (process side), Messages (error reasons from OS errors), F-LAYOUT-01 (terminal size).
//! Owner: app lead. Depends on `xplain-core` and `xplain-integrations`.
//! Must not: contain UI logic or state (that is core), agent names (that is integrations), or
//! decisions core can make. Never shows OS error text: map to `IoReason`.
//!
//! Module registry: complete up front (see `MODULES.md`); workers add inner modules inside their own
//! top-level module directory, never here.

// component B: startup, config and file/git IO
pub mod cli;
pub mod config_io;
pub mod env;
pub mod fsio;
pub mod git;
pub mod run;
// component A: terminal, input, loop
pub mod clipboard;
pub mod input;
pub mod present;
pub mod runtime;
pub mod term;
pub mod timers;
// component C: MCP sockets, processes, effect dispatch
pub mod exec;
pub mod http;
pub mod proc;
pub mod token;

#[cfg(test)]
mod test_util;
