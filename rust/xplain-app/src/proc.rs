//! Integration CLI process runner.
//!
//! Spec: F-INTEG-01..04 (commands come ready-made as `CommandSpec`; run in `cwd`/env), F-INTEG-02
//! (not on PATH -> `CommandError::NotFound`), UNSPEC-37 (timeout from `spec.timeout_ms`, kill child,
//! `CommandError::Timeout`), Test seams (PATH holds only fakes: never assume other binaries).
//! Owner: component C (mcp/exec).
//! Must not: know agent names or argv shapes, interpret output, or fail with runtime error text beyond
//! `CommandError::Other(message)` (message only for spawn errors other than not-found).

use xplain_core::integration::{CommandResult, CommandSpec};

/// Spawn, capture stdout/stderr as lossy utf-8, wait (with timeout). stdin is closed/null.
pub async fn run_command(_spec: &CommandSpec) -> CommandResult {
    todo!("spawn + timeout")
}
