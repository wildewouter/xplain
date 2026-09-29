//! `mcp.json` token file IO.
//!
//! Spec: F-MCPSRV-01 (dir `<state dir>` created mode 0700 only when created; file `mcp.json` chmod 0600
//! on every write; reuse decision via core `plan_token`; token from 32 random bytes; create on first
//! start, not at launch), Messages (`cannot write <state dir>/mcp.json: <reason>`).
//! Owner: component C (mcp/exec).
//! Must not: generate the token text or file content (core `plan_token`), show OS text, use `unsafe`.

/// Resolve the bearer token: read existing file, ask `plan_token`, write if needed.
/// `Err` = final message `cannot write <state_dir>/mcp.json: <reason>`.
pub async fn ensure_token(_state_dir: &str) -> Result<String, String> {
    todo!("F-MCPSRV-01")
}
