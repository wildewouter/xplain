//! xplain-app: the runtime around `xplain-core`. Owns every side effect: terminal, git and fs, process
//! spawning, clock/timers, OSC 52, the MCP HTTP server, argv.
//!
//! Spec: CLI (F-CLI-01..06), Test seams (barriers, request counters, MCP port), F-MODE-01/02/04 (execution
//! side), F-CONFIG-05 (write side), F-EXPORT-01 (write side), F-MCPSRV-01/02 (socket side), F-INTEG-*
//! (process side), Messages (error reasons from OS errors), F-LAYOUT-01 (terminal size).
//! Owner: app lead. Depends on `xplain-core` and `xplain-integrations`.
//! Must not: contain UI logic or state (that is core), agent names (that is integrations), or
//! decisions core can make. Never shows OS error text: map to `IoReason`.
//!
//! Module registry: workers add inner modules inside their own top-level module, not here.

pub mod cli;
pub mod exec;
pub mod http;
pub mod input;
pub mod present;
pub mod run;
pub mod runtime;
