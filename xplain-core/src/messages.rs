//! Cross-cutting user-facing message builders (the `<action> <target>: <reason>` form and friends).
//!
//! Spec: SPEC "Messages" (sites list), F-MODE-04, F-CLI-06, F-BROWSE-01, F-CONFIG-05, F-EXPORT-01,
//! F-MCPSRV-01, F-MCPUI-03. Owner: component `parse` (A).
//! Must not: hold strings that only one module uses (those stay `const` in that module), or carry OS text.

use crate::errors::{IoReason, fail_msg};

/// Note when an action needs the MCP server but it is stopped (F-COMMENT-02, F-ASK-02).
pub const MCP_OFF: &str = "MCP is off (M to start)";
/// Note when a follow-up is refused (F-ASK-03).
pub const CANT_FOLLOW_UP: &str = "can't follow up yet";
/// Note after a follow-up turn is queued (F-ASK-03).
pub const FOLLOW_UP_QUEUED: &str = "follow-up queued";

/// `cannot run git: <reason>` (F-MODE-04).
pub fn cannot_run_git(reason: IoReason) -> String {
    fail_msg("cannot run git", reason)
}

/// `cannot open directory <dir>: <reason>` (F-CLI-06).
pub fn cannot_open_directory(dir: &str, reason: IoReason) -> String {
    fail_msg(&format!("cannot open directory {dir}"), reason)
}

/// `cannot read <path>: <reason>` (F-BROWSE-01, F-COMMENT-09).
pub fn cannot_read(path: &str, reason: IoReason) -> String {
    fail_msg(&format!("cannot read {path}"), reason)
}

/// `config save failed: <path>: <reason>` (F-CONFIG-05).
pub fn config_save_failed(path: &str, reason: IoReason) -> String {
    fail_msg(&format!("config save failed: {path}"), reason)
}

/// `config unreadable, not saved (<path>)` (F-CONFIG-05).
pub fn config_unreadable(path: &str) -> String {
    format!("config unreadable, not saved ({path})")
}

/// `export failed: <path>: <reason>` (F-EXPORT-01).
pub fn export_failed(path: &str, reason: IoReason) -> String {
    fail_msg(&format!("export failed: {path}"), reason)
}

/// `cannot write <path>: <reason>` (F-MCPSRV-01, token file). Used by the runtime; kept here for one wording.
pub fn cannot_write(path: &str, reason: IoReason) -> String {
    fail_msg(&format!("cannot write {path}"), reason)
}

/// `cannot listen on 127.0.0.1:<port>: <reason>` (F-MCPUI-03).
pub fn cannot_listen(port: u16, reason: IoReason) -> String {
    fail_msg(&format!("cannot listen on 127.0.0.1:{port}"), reason)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_exact_wording() {
        assert_eq!(cannot_run_git(IoReason::NotFound), "cannot run git: not found");
        assert_eq!(
            cannot_open_directory("/x", IoReason::NotDirectory),
            "cannot open directory /x: not a directory"
        );
        assert_eq!(cannot_read("a.txt", IoReason::PermissionDenied), "cannot read a.txt: permission denied");
        assert_eq!(config_save_failed("/c.json", IoReason::Failed), "config save failed: /c.json: failed");
        assert_eq!(config_unreadable("/c.json"), "config unreadable, not saved (/c.json)");
        assert_eq!(export_failed("e.md", IoReason::IsDirectory), "export failed: e.md: is a directory");
        assert_eq!(cannot_write("t", IoReason::NotFound), "cannot write t: not found");
        assert_eq!(
            cannot_listen(80, IoReason::PermissionDenied),
            "cannot listen on 127.0.0.1:80: permission denied"
        );
    }
}
