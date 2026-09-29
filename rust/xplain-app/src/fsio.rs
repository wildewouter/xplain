//! Plain file IO for effects.
//!
//! Spec: F-BROWSE-01 (`ReadFile`, raw bytes), F-COMMENT-09 (browse read errors), F-EXPORT-01
//! (`WriteExport`), Messages (IoReason mapping).
//! Owner: component B (startup/IO).
//! Must not: interpret contents (NUL detection, line split are core), format messages, or show OS text.

use std::path::{Path, PathBuf};

use tokio::io::AsyncWriteExt;
use xplain_core::errors::IoReason;

/// Map an io error to its user-facing reason (by kind only).
pub fn io_reason(e: std::io::Error) -> IoReason {
    IoReason::from_io_error(&e)
}

/// Read whole file. Directory -> `IsDirectory` etc. by error kind (`IoReason::from_io_error`).
pub async fn read_file(path: &str) -> Result<Vec<u8>, IoReason> {
    tokio::fs::read(path).await.map_err(io_reason)
}

/// Write `contents` to `path` (create/truncate). Parent dir is not created.
pub async fn write_export(path: &str, contents: &str) -> Result<(), IoReason> {
    tokio::fs::write(path, contents).await.map_err(io_reason)
}

/// Write `contents` to `path` via a sibling temp file and rename, so readers never see a partial file.
/// `mode` (unix permission bits) applies to the new file; the temp file is removed on failure.
pub async fn atomic_write(path: &Path, contents: &[u8], mode: Option<u32>) -> std::io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(format!(".{}.tmp", std::process::id()));
    let tmp = PathBuf::from(tmp);
    let res = async {
        let mut opts = tokio::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        if let Some(m) = mode {
            opts.mode(m);
        }
        let mut f = opts.open(&tmp).await?;
        f.write_all(contents).await?;
        f.flush().await?;
        drop(f);
        tokio::fs::rename(&tmp, path).await
    }
    .await;
    if res.is_err() {
        let _ = tokio::fs::remove_file(&tmp).await;
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::tmp as tmp_dir;

    fn tmp(name: &str) -> PathBuf {
        tmp_dir("fsio", name)
    }

    #[tokio::test]
    async fn atomic_write_replaces_and_leaves_no_temp() {
        let d = tmp("atomic");
        let f = d.join("a.json");
        atomic_write(&f, b"one", Some(0o600)).await.unwrap();
        atomic_write(&f, b"two", Some(0o600)).await.unwrap();
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "two");
        assert_eq!(std::fs::read_dir(&d).unwrap().count(), 1);
        let bad = d.join("no/such/dir/x");
        assert!(atomic_write(&bad, b"x", None).await.is_err());
        assert_eq!(std::fs::read_dir(&d).unwrap().count(), 1);
    }

    #[tokio::test]
    async fn read_ok_and_errors() {
        let d = tmp("read");
        let f = d.join("a.txt");
        std::fs::write(&f, b"hi\xff").unwrap();
        assert_eq!(read_file(f.to_str().unwrap()).await.unwrap(), b"hi\xff");
        assert_eq!(read_file(d.join("nope").to_str().unwrap()).await, Err(IoReason::NotFound));
        let r = read_file(d.to_str().unwrap()).await;
        assert!(matches!(r, Err(IoReason::IsDirectory | IoReason::Failed)));
        let r = read_file(f.join("x").to_str().unwrap()).await;
        assert!(matches!(r, Err(IoReason::NotDirectory | IoReason::NotFound)));
    }

    #[tokio::test]
    async fn write_truncates_and_no_mkdir() {
        let d = tmp("write");
        let f = d.join("e.md");
        write_export(f.to_str().unwrap(), "long content").await.unwrap();
        write_export(f.to_str().unwrap(), "short").await.unwrap();
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "short");
        let r = write_export(d.join("sub/x").to_str().unwrap(), "x").await;
        assert_eq!(r, Err(IoReason::NotFound));
    }
}
