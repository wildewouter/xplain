//! Plain file IO for effects.
//!
//! Spec: F-BROWSE-01 (`ReadFile`, raw bytes), F-COMMENT-09 (browse read errors), F-EXPORT-01
//! (`WriteExport`), Messages (IoReason mapping).
//! Owner: component B (startup/IO).
//! Must not: interpret contents (NUL detection, line split are core), format messages, or show OS text.

use xplain_core::errors::IoReason;

/// Read whole file. Directory -> `IsDirectory` etc. by error kind (`IoReason::from_io_error`).
pub async fn read_file(path: &str) -> Result<Vec<u8>, IoReason> {
    tokio::fs::read(path).await.map_err(|e| IoReason::from_io_error(&e))
}

/// Write `contents` to `path` (create/truncate). Parent dir is not created.
pub async fn write_export(path: &str, contents: &str) -> Result<(), IoReason> {
    tokio::fs::write(path, contents).await.map_err(|e| IoReason::from_io_error(&e))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("xplain-fsio-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
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
