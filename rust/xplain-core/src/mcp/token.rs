//! Token and port helpers (pure).
//!
//! Spec: F-MCPSRV-01 (token file reuse rule, base64url 43 chars from 32 random bytes), Test seams
//! (`XPLAIN_MCP_PORT`), F-INTEG-05/06 (masking the token in shown text). Owner: component `agent` (E).
//! The boundary fns `parse_port` / `plan_token` live in `mcp/mod.rs` and delegate here.

/// base64url without padding.
pub fn base64url(_bytes: &[u8]) -> String {
    todo!("F-MCPSRV-01")
}

/// Replace every occurrence of `token` in `text` by a masked form for display (F-INTEG-05, F-MCPUI-01).
pub fn mask(_text: &str, _token: &str) -> String {
    todo!("F-INTEG-05 masking")
}

/// Contents of `mcp.json` for a token (`{"token": ...}` pretty form per spec).
pub fn file_contents(_token: &str) -> String {
    todo!("F-MCPSRV-01")
}
