//! Config file IO (the side-effect half of `xplain_core::config`).
//!
//! Spec: F-CONFIG-01/04 (read: missing / text / unreadable classification), F-CONFIG-05 (save:
//! read-merge-write, create parent dirs, write; write mechanism UNSPEC-36: temp + rename allowed),
//! F-CFGUI-03 (save errors map to `ConfigSaveError`).
//! Owner: component B (startup).
//! Must not: parse or merge JSON itself (core `load_config` / `apply_patch` do), show OS error text.

use xplain_core::config::{ConfigChange, ConfigFile, ConfigSaveError};

/// Read `path`: missing -> `Missing`; readable -> `Text` (lossy utf-8); other errors -> `Unreadable(reason)`.
pub fn read_config_file(_path: &str) -> ConfigFile {
    todo!("F-CONFIG-04")
}

/// Executes `Effect::SaveConfig`: read existing (missing = `None`), `apply_patch(existing, change.to_patch())`,
/// `create_dir_all(parent)`, write. Any IO failure -> `ConfigSaveError::Io(IoReason::from_io_error)`.
/// Existing file that is not a JSON object -> `ConfigSaveError::Unreadable` (from `apply_patch`), file untouched.
pub async fn save_config(_path: &str, _change: &ConfigChange) -> Result<(), ConfigSaveError> {
    todo!("F-CONFIG-05")
}
