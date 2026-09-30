//! Config file IO (the side-effect half of `xplain_core::config`).
//!
//! Spec: F-CONFIG-01/04 (read: missing / text / unreadable classification), F-CONFIG-05 (save:
//! read-merge-write, create parent dirs, write; write mechanism UNSPEC-36: temp + rename allowed),
//! F-CFGUI-03 (save errors map to `ConfigSaveError`).
//! Owner: component B (startup).
//! Must not: parse or merge JSON itself (core `load_config` / `apply_patch` do), show OS error text.

use std::io::ErrorKind;
use std::path::Path;

use xplain_core::config::{ConfigChange, ConfigFile, ConfigSaveError, apply_patch};
use xplain_core::errors::IoReason;

/// Read `path`: missing -> `Missing`; readable -> `Text` (lossy utf-8); other errors -> `Unreadable(reason)`.
pub fn read_config_file(path: &str) -> ConfigFile {
    match std::fs::read(path) {
        Ok(b) => ConfigFile::Text(String::from_utf8_lossy(&b).into_owned()),
        Err(e) if e.kind() == ErrorKind::NotFound => ConfigFile::Missing,
        Err(e) => ConfigFile::Unreadable(crate::fsio::io_reason(e)),
    }
}

fn io(e: std::io::Error) -> ConfigSaveError {
    ConfigSaveError::Io(crate::fsio::io_reason(e))
}

/// Executes `Effect::SaveConfig`: read existing (missing = `None`), `apply_patch(existing, change.to_patch())`,
/// `create_dir_all(parent)`, write. Any IO failure -> `ConfigSaveError::Io(IoReason::from_io_error)`.
/// Existing file that is not a JSON object -> `ConfigSaveError::Unreadable` (from `apply_patch`), file untouched.
pub async fn save_config(path: &str, change: &ConfigChange) -> Result<(), ConfigSaveError> {
    let existing = match tokio::fs::read(path).await {
        Ok(b) => Some(String::from_utf8_lossy(&b).into_owned()),
        Err(e) if matches!(e.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory) => None,
        // Exists but cannot be read: same as unreadable content, nothing written.
        Err(_) => return Err(ConfigSaveError::Unreadable),
    };
    let out = apply_patch(existing.as_deref(), &change.to_patch())?;
    let p = Path::new(path);
    if let Some(parent) = p.parent().filter(|d| !d.as_os_str().is_empty()) {
        // Existing non-directory on the path reports AlreadyExists, which is reported as ENOTDIR.
        tokio::fs::create_dir_all(parent).await.map_err(|e| {
            if e.kind() == ErrorKind::AlreadyExists {
                ConfigSaveError::Io(IoReason::NotDirectory)
            } else {
                io(e)
            }
        })?;
    }
    crate::fsio::atomic_write(p, out.as_bytes(), None).await.map_err(io)
}

#[cfg(test)]
mod tests {
    use super::*;
    use xplain_core::options::DiffMode;

    fn tmp(name: &str) -> std::path::PathBuf {
        crate::test_util::tmp("cfgio", name)
    }

    #[test]
    fn read_classify() {
        let d = tmp("read");
        let f = d.join("c.json");
        assert_eq!(read_config_file(f.to_str().unwrap()), ConfigFile::Missing);
        std::fs::write(&f, b"{\"a\":1}\xff").unwrap();
        match read_config_file(f.to_str().unwrap()) {
            ConfigFile::Text(t) => assert!(t.starts_with("{\"a\":1}")),
            o => panic!("{o:?}"),
        }
        assert!(matches!(read_config_file(d.to_str().unwrap()), ConfigFile::Unreadable(_)));
    }

    #[tokio::test]
    async fn save_creates_parents_and_merges() {
        let d = tmp("save");
        let f = d.join("a/b/config.json");
        let ps = f.to_str().unwrap();
        save_config(ps, &ConfigChange::Mode(DiffMode::Staged)).await.unwrap();
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
        assert_eq!(v["view"]["mode"], "staged");
        assert_eq!(v["version"], 1);
        save_config(ps, &ConfigChange::Split(true)).await.unwrap();
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
        assert_eq!(v["view"]["mode"], "staged");
        assert_eq!(v["view"]["split"], true);
        let leftovers: Vec<_> = std::fs::read_dir(f.parent().unwrap()).unwrap().collect();
        assert_eq!(leftovers.len(), 1);
    }

    #[tokio::test]
    async fn save_unreadable_leaves_file() {
        let d = tmp("bad");
        let f = d.join("c.json");
        std::fs::write(&f, "[1]").unwrap();
        let r = save_config(f.to_str().unwrap(), &ConfigChange::Split(true)).await;
        assert_eq!(r, Err(ConfigSaveError::Unreadable));
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "[1]");
        std::fs::write(&f, "{oops").unwrap();
        let r = save_config(f.to_str().unwrap(), &ConfigChange::Split(true)).await;
        assert_eq!(r, Err(ConfigSaveError::Unreadable));
    }

    #[tokio::test]
    async fn save_io_error() {
        let d = tmp("io");
        let blocker = d.join("file");
        std::fs::write(&blocker, "x").unwrap();
        let r = save_config(blocker.join("sub/c.json").to_str().unwrap(), &ConfigChange::Split(true)).await;
        assert_eq!(r, Err(ConfigSaveError::Io(IoReason::NotDirectory)));
        let r = save_config(blocker.join("c.json").to_str().unwrap(), &ConfigChange::Split(true)).await;
        assert_eq!(r, Err(ConfigSaveError::Io(IoReason::NotDirectory)));
    }
}
