//! Plain file IO for effects.
//!
//! Spec: F-BROWSE-01 (`ReadFile`, raw bytes), F-COMMENT-09 (browse read errors), F-EXPORT-01
//! (`WriteExport`), Messages (IoReason mapping).
//! Owner: component B (startup/IO).
//! Must not: interpret contents (NUL detection, line split are core), format messages, or show OS text.

use xplain_core::errors::IoReason;

/// Read whole file. Directory -> `IsDirectory` etc. by error kind (`IoReason::from_io_error`).
pub async fn read_file(_path: &str) -> Result<Vec<u8>, IoReason> {
    todo!("F-BROWSE-01")
}

/// Write `contents` to `path` (create/truncate). Parent dir is not created.
pub async fn write_export(_path: &str, _contents: &str) -> Result<(), IoReason> {
    todo!("F-EXPORT-01")
}
