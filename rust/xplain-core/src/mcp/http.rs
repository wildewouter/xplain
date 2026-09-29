//! HTTP-level request checks for the MCP endpoint.
//!
//! Spec: F-MCPSRV-02 (path, method, Host/Origin checks, bearer auth, content type, body size -> 413, exact
//! status codes and headers such as `www-authenticate`, `allow`). Owner: component `agent` (E).
//! Must not: parse JSON-RPC (rpc.rs) or open sockets.

use super::{HttpRequest, HttpResponse};

/// Run F-MCPSRV-02 in spec order. `Err(response)` = reject with that response; `Ok(())` = go on to JSON-RPC.
/// `token` is the active bearer token, `port` the effective bound port (Host header check).
pub fn precheck(_req: &HttpRequest, _token: &str, _port: u16) -> Result<(), HttpResponse> {
    todo!("F-MCPSRV-02")
}

/// JSON response builder: adds `content-type: application/json` handling per `HttpResponse` docs.
pub fn json_response(
    _status: u16,
    _headers: Vec<(String, String)>,
    _body: &serde_json::Value,
) -> HttpResponse {
    todo!("F-MCPSRV-02/03")
}
