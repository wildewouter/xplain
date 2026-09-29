//! Integration CLI process runner.
//!
//! Spec: F-INTEG-01..04 (commands come ready-made as `CommandSpec`; run in `cwd`/env), F-INTEG-02
//! (not on PATH -> `CommandError::NotFound`), UNSPEC-37 (timeout from `spec.timeout_ms`, kill child,
//! `CommandError::Timeout`), Test seams (PATH holds only fakes: never assume other binaries).
//! Owner: component C (mcp/exec).
//! Must not: know agent names or argv shapes, interpret output, or fail with runtime error text beyond
//! `CommandError::Other(message)` (message only for spawn errors other than not-found).

use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;
use xplain_core::errors::IoReason;
use xplain_core::integration::{CommandError, CommandOutput, CommandResult, CommandSpec};

/// Spawn, capture stdout/stderr as lossy utf-8, wait (with timeout). stdin is closed/null.
pub async fn run_command(spec: &CommandSpec) -> CommandResult {
    let mut cmd = Command::new(&spec.program);
    cmd.args(&spec.args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Some(cwd) = &spec.cwd {
        cmd.current_dir(cwd);
    }
    for (k, v) in &spec.env {
        cmd.env(k, v);
    }
    let child = cmd.spawn().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => CommandError::NotFound,
        _ => CommandError::Other(IoReason::from_io_error(&e).as_str().to_string()),
    })?;
    // Dropping the wait future on timeout drops the child, which kills it (kill_on_drop).
    let waited = tokio::time::timeout(Duration::from_millis(spec.timeout_ms), child.wait_with_output()).await;
    match waited {
        Err(_) => Err(CommandError::Timeout),
        Ok(Err(e)) => Err(CommandError::Other(IoReason::from_io_error(&e).as_str().to_string())),
        Ok(Ok(out)) => Ok(CommandOutput {
            code: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sh(script: &str, timeout_ms: u64) -> CommandSpec {
        CommandSpec {
            program: "sh".into(),
            args: vec!["-c".into(), script.into()],
            cwd: None,
            env: vec![],
            timeout_ms,
        }
    }

    #[tokio::test]
    async fn captures_output_and_code() {
        let r = run_command(&sh("echo out; echo err >&2; exit 3", 5000)).await;
        assert_eq!(r, Ok(CommandOutput { code: 3, stdout: "out\n".into(), stderr: "err\n".into() }));
    }

    #[tokio::test]
    async fn cwd_and_env_applied() {
        let mut s = sh("pwd; echo $XP_TEST", 5000);
        s.cwd = Some("/".into());
        s.env = vec![("XP_TEST".into(), "hello".into())];
        let r = run_command(&s).await;
        assert_eq!(r.map(|o| o.stdout), Ok("/\nhello\n".to_string()));
    }

    #[tokio::test]
    async fn stdin_is_null() {
        let r = run_command(&sh("cat; echo done", 5000)).await;
        assert_eq!(r.map(|o| o.stdout), Ok("done\n".to_string()));
    }

    #[tokio::test]
    async fn lossy_utf8() {
        let r = run_command(&sh("printf '\\377a'", 5000)).await;
        assert_eq!(r.map(|o| o.stdout), Ok("\u{fffd}a".to_string()));
    }

    #[tokio::test]
    async fn timeout_kills() {
        let t = std::time::Instant::now();
        let r = run_command(&sh("sleep 30", 150)).await;
        assert_eq!(r, Err(CommandError::Timeout));
        assert!(t.elapsed() < Duration::from_secs(5));
    }

    #[tokio::test]
    async fn not_found() {
        let mut s = sh("", 1000);
        s.program = "xplain-no-such-binary-zzz".into();
        assert_eq!(run_command(&s).await, Err(CommandError::NotFound));
    }

    #[tokio::test]
    async fn other_spawn_error() {
        let s = CommandSpec { program: "/".into(), args: vec![], cwd: None, env: vec![], timeout_ms: 1000 };
        assert!(matches!(run_command(&s).await, Err(CommandError::Other(_))));
    }
}
