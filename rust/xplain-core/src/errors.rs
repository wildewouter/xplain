//! Runtime-neutral error reasons and the `<action> <target>: <reason>` message form.
//!
//! Spec: SPEC "Messages" (reason table). Owner: core lead.
//! Must not: carry runtime error text, or depend on OS error message strings. The runtime maps an OS
//! error to [`IoReason`] via [`IoReason::from_io_error`] and only the reason crosses into core.

use std::io::ErrorKind;

/// Reason word table: ENOENT `not found`, EACCES/EPERM `permission denied`, EISDIR `is a directory`,
/// ENOTDIR `not a directory`, anything else `failed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IoReason {
    NotFound,
    PermissionDenied,
    IsDirectory,
    NotDirectory,
    Failed,
}

impl IoReason {
    pub fn as_str(self) -> &'static str {
        match self {
            IoReason::NotFound => "not found",
            IoReason::PermissionDenied => "permission denied",
            IoReason::IsDirectory => "is a directory",
            IoReason::NotDirectory => "not a directory",
            IoReason::Failed => "failed",
        }
    }

    /// Maps by error kind only; never uses the error's message text.
    pub fn from_io_error(e: &std::io::Error) -> Self {
        match e.kind() {
            ErrorKind::NotFound => IoReason::NotFound,
            ErrorKind::PermissionDenied => IoReason::PermissionDenied,
            ErrorKind::IsADirectory => IoReason::IsDirectory,
            ErrorKind::NotADirectory => IoReason::NotDirectory,
            _ => IoReason::Failed,
        }
    }
}

/// `<what>: <reason>`, e.g. `fail_msg("cannot read a.txt", IoReason::NotFound)`.
pub fn fail_msg(what: &str, reason: IoReason) -> String {
    format!("{what}: {}", reason.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_form() {
        assert_eq!(fail_msg("cannot run git", IoReason::NotFound), "cannot run git: not found");
    }
}
