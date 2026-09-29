//! Cross-cutting user-facing message builders (the `<action> <target>: <reason>` form and friends).
//!
//! Spec: SPEC "Messages" (sites list), F-MODE-04, F-CLI-06, F-BROWSE-01, F-CONFIG-05, F-EXPORT-01,
//! F-MCPSRV-01, F-MCPUI-03. Owner: component `parse` (A).
//! Must not: hold strings that only one module uses (those stay `const` in that module), or carry OS text.

use crate::errors::{IoReason, fail_msg};

/// `cannot run git: <reason>` (F-MODE-04).
pub fn cannot_run_git(_reason: IoReason) -> String {
    todo!("F-MODE-04")
}

/// `cannot open directory <dir>: <reason>` (F-CLI-06).
pub fn cannot_open_directory(_dir: &str, _reason: IoReason) -> String {
    todo!("F-CLI-06")
}

/// `cannot read <path>: <reason>` (F-BROWSE-01, F-COMMENT-09).
pub fn cannot_read(path: &str, reason: IoReason) -> String {
    fail_msg(&format!("cannot read {path}"), reason)
}

/// `config save failed: <path>: <reason>` (F-CONFIG-05).
pub fn config_save_failed(_path: &str, _reason: IoReason) -> String {
    todo!("F-CONFIG-05")
}

/// `config unreadable, not saved (<path>)` (F-CONFIG-05).
pub fn config_unreadable(_path: &str) -> String {
    todo!("F-CONFIG-05")
}

/// `export failed: <path>: <reason>` (F-EXPORT-01).
pub fn export_failed(_path: &str, _reason: IoReason) -> String {
    todo!("F-EXPORT-01")
}

/// `cannot write <path>: <reason>` (F-MCPSRV-01, token file). Used by the runtime; kept here for one wording.
pub fn cannot_write(_path: &str, _reason: IoReason) -> String {
    todo!("F-MCPSRV-01")
}

/// `cannot listen on 127.0.0.1:<port>: <reason>` (F-MCPUI-03).
pub fn cannot_listen(_port: u16, _reason: IoReason) -> String {
    todo!("F-MCPUI-03")
}
