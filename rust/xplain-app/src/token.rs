//! `mcp.json` token file IO.
//!
//! Spec: F-MCPSRV-01 (dir `<state dir>` created mode 0700 only when created; file `mcp.json` chmod 0600
//! on every write; reuse decision via core `plan_token`; token from 32 random bytes; create on first
//! start, not at launch), Messages (`cannot write <state dir>/mcp.json: <reason>`).
//! Owner: component C (mcp/exec).
//! Must not: generate the token text or file content (core `plan_token`), show OS text, use `unsafe`.

use std::path::Path;

use xplain_core::errors::IoReason;
use xplain_core::mcp::{TokenPlan, plan_token};
use xplain_core::messages::cannot_write;

use crate::fsio::{atomic_write, io_reason};

/// Resolve the bearer token: read existing file, ask `plan_token`, write if needed.
/// `Err` = final message `cannot write <state_dir>/mcp.json: <reason>`.
pub async fn ensure_token(state_dir: &str) -> Result<String, String> {
    if state_dir.is_empty() {
        // No usable state dir (neither XDG_STATE_HOME nor HOME set): never write relative to cwd.
        return Err(cannot_write("mcp.json", IoReason::NotFound));
    }
    let dir = Path::new(state_dir);
    let file = dir.join("mcp.json");
    let existing = tokio::fs::read(&file).await.ok().map(|b| String::from_utf8_lossy(&b).into_owned());
    let random: [u8; 32] = rand::random();
    match plan_token(existing.as_deref(), random) {
        TokenPlan::Reuse(token) => Ok(token),
        TokenPlan::Write { token, file_contents } => {
            write_file(dir, &file, &file_contents)
                .await
                .map_err(|reason| cannot_write(&file.display().to_string(), reason))?;
            Ok(token)
        }
    }
}

async fn write_file(dir: &Path, file: &Path, contents: &str) -> Result<(), IoReason> {
    // Mode 0700 only for directories this call creates; an existing dir is left alone.
    tokio::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir).await.map_err(io_reason)?;
    atomic_write(file, contents.as_bytes(), Some(0o600)).await.map_err(io_reason)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn tmp(name: &str) -> std::path::PathBuf {
        crate::test_util::tmp("token", name)
    }

    fn mode(p: &Path) -> u32 {
        std::fs::metadata(p).map(|m| m.permissions().mode() & 0o777).unwrap_or(0)
    }

    #[tokio::test]
    async fn creates_dir_and_file_with_modes() {
        let d = tmp("create");
        let dir = d.join("state");
        let t = ensure_token(&dir.to_string_lossy()).await;
        let t = t.unwrap_or_default();
        assert_eq!(t.len(), 43);
        assert_eq!(mode(&dir), 0o700);
        assert_eq!(mode(&dir.join("mcp.json")), 0o600);
        let text = std::fs::read_to_string(dir.join("mcp.json")).unwrap_or_default();
        assert_eq!(text, format!("{{\n  \"token\": \"{t}\"\n}}\n"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[tokio::test]
    async fn reuses_existing_untouched() {
        let d = tmp("reuse");
        let _ = std::fs::create_dir_all(&d);
        let content = "{\"token\":\"abcdefabcdefabcdef\",\"port\":1}";
        let _ = std::fs::write(d.join("mcp.json"), content);
        let t = ensure_token(&d.to_string_lossy()).await;
        assert_eq!(t, Ok("abcdefabcdefabcdef".to_string()));
        assert_eq!(std::fs::read_to_string(d.join("mcp.json")).unwrap_or_default(), content);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[tokio::test]
    async fn short_token_rewritten_and_chmod_every_write() {
        let d = tmp("short");
        let _ = std::fs::create_dir_all(&d);
        let _ = std::fs::write(d.join("mcp.json"), "{\"token\":\"short\"}");
        let _ = std::fs::set_permissions(d.join("mcp.json"), std::fs::Permissions::from_mode(0o644));
        let t = ensure_token(&d.to_string_lossy()).await.unwrap_or_default();
        assert_eq!(t.len(), 43);
        assert_eq!(mode(&d.join("mcp.json")), 0o600);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[tokio::test]
    async fn existing_dir_mode_kept() {
        let d = tmp("dirmode");
        let _ = std::fs::create_dir_all(&d);
        let _ = std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o755));
        let _ = ensure_token(&d.to_string_lossy()).await;
        assert_eq!(mode(&d), 0o755);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[tokio::test]
    async fn write_failure_message() {
        let d = tmp("fail");
        let _ = std::fs::create_dir_all(&d);
        let blocker = d.join("f");
        let _ = std::fs::write(&blocker, "x");
        let dir = blocker.join("sub");
        let err = ensure_token(&dir.to_string_lossy()).await.err().unwrap_or_default();
        assert_eq!(err, format!("cannot write {}/mcp.json: not a directory", dir.display()));
        let _ = std::fs::remove_dir_all(&d);
    }
}
