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

use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;
use xplain_core::errors::IoReason;
use xplain_core::integration::{CommandError, CommandOutput, CommandResult, CommandSpec};

/// Per-stream capture cap; output beyond it is drained and discarded so the child never blocks.
const MAX_OUTPUT: u64 = 64 * 1024 * 1024;

fn other(e: &std::io::Error) -> CommandError {
    CommandError::Other(IoReason::from_io_error(e).as_str().to_string())
}

/// Read up to [`MAX_OUTPUT`] bytes, then drain the rest.
async fn capture<R: AsyncRead + Unpin>(r: Option<R>) -> String {
    let Some(r) = r else {
        return String::new();
    };
    let mut buf = Vec::new();
    let mut limited = r.take(MAX_OUTPUT);
    let _ = limited.read_to_end(&mut buf).await;
    let mut rest = limited.into_inner();
    let _ = tokio::io::copy(&mut rest, &mut tokio::io::sink()).await;
    String::from_utf8_lossy(&buf).into_owned()
}

/// SIGKILL the whole process group (the child leads its own group), so grandchildren die too.
async fn kill_group(pid: u32) {
    let _ = Command::new("kill")
        .args(["-KILL", "--", &format!("-{pid}")])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;
}

/// Spawn, capture stdout/stderr as lossy utf-8 (bounded), wait (with timeout). stdin is closed/null.
/// The child runs in its own process group; on timeout the whole group is killed.
pub async fn run_command(spec: &CommandSpec) -> CommandResult {
    let mut cmd = Command::new(&spec.program);
    cmd.args(&spec.args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .kill_on_drop(true);
    if let Some(cwd) = &spec.cwd {
        cmd.current_dir(cwd);
    }
    for (k, v) in &spec.env {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => CommandError::NotFound,
        _ => other(&e),
    })?;
    let pid = child.id();
    let (out, err) = (child.stdout.take(), child.stderr.take());
    let run = async { tokio::join!(capture(out), capture(err), child.wait()) };
    match tokio::time::timeout(Duration::from_millis(spec.timeout_ms), run).await {
        Err(_) => {
            if let Some(pid) = pid {
                kill_group(pid).await;
            }
            let _ = child.kill().await;
            Err(CommandError::Timeout)
        }
        Ok((_, _, Err(e))) => Err(other(&e)),
        Ok((stdout, stderr, Ok(status))) => {
            Ok(CommandOutput { code: status.code().unwrap_or(-1), stdout, stderr })
        }
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
    async fn timeout_kills_grandchildren() {
        let d = crate::test_util::tmp("proc", "grand");
        let pidfile = d.join("pid");
        let script = format!("sleep 30 & echo $! > {}; wait", pidfile.display());
        let r = run_command(&sh(&script, 500)).await;
        assert_eq!(r, Err(CommandError::Timeout));
        let pid = std::fs::read_to_string(&pidfile).unwrap().trim().to_string();
        tokio::time::sleep(Duration::from_millis(200)).await;
        let alive = std::process::Command::new("kill").args(["-0", &pid]).status().unwrap().success();
        assert!(!alive, "grandchild {pid} survived");
    }

    #[tokio::test]
    async fn output_is_bounded_and_drained() {
        // 70 MiB of output exceeds the cap; the child must still finish (pipe drained).
        let r = run_command(&sh("head -c 73400320 /dev/zero | tr '\\0' a", 30_000)).await.unwrap();
        assert_eq!(r.code, 0);
        assert_eq!(r.stdout.len() as u64, MAX_OUTPUT);
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
