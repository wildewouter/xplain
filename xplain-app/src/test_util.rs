//! Shared test helper: a fresh temp directory per test.

use std::path::PathBuf;

/// Fresh empty dir `<tmp>/xplain-<tag>-<pid>-<name>` (removed first if left over).
pub fn tmp(tag: &str, name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("xplain-{tag}-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}
