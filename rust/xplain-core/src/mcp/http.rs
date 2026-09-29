//! HTTP-level request checks for the MCP endpoint.
//!
//! Spec: F-MCPSRV-02 (path, method, Host/Origin checks, bearer auth, content type, body size -> 413, exact
//! status codes and headers such as `www-authenticate`, `allow`). Owner: component `agent` (E).
//! Must not: parse JSON-RPC (rpc.rs) or open sockets.

use super::{HttpRequest, HttpResponse};

const LOOPBACK: [&str; 4] = ["localhost", "127.0.0.1", "[::1]", "::1"];

/// Run F-MCPSRV-02 in spec order. `Err(response)` = reject with that response; `Ok(())` = go on to JSON-RPC.
/// `token` is the active bearer token, `port` the effective bound port (Host header check).
pub fn precheck(req: &HttpRequest, token: &str, _port: u16) -> Result<(), HttpResponse> {
    let host_ok = header(req, "host").is_some_and(host_loopback);
    let origin_ok = header(req, "origin").is_none_or(origin_loopback);
    if !host_ok || !origin_ok {
        return Err(error_response(403, Vec::new(), "forbidden"));
    }
    if path_of(&req.path) != "/mcp" {
        return Err(error_response(404, Vec::new(), "not found"));
    }
    if !bearer_ok(header(req, "authorization"), token) {
        return Err(error_response(401, vec![("www-authenticate".into(), "Bearer".into())], "unauthorized"));
    }
    if req.method != "POST" {
        return Err(error_response(405, vec![("allow".into(), "POST".into())], "method not allowed"));
    }
    if req.body_too_large {
        return Err(error_response(413, vec![("connection".into(), "close".into())], "body too large"));
    }
    Ok(())
}

/// JSON response builder: `content-type: application/json` first, then `headers`.
pub fn json_response(status: u16, headers: Vec<(String, String)>, body: &serde_json::Value) -> HttpResponse {
    raw_response(status, headers, body.to_string())
}

/// Like [`json_response`] for an already serialized body.
pub fn raw_response(status: u16, headers: Vec<(String, String)>, body: String) -> HttpResponse {
    let mut all = Vec::with_capacity(headers.len() + 1);
    if !body.is_empty() {
        all.push(("content-type".to_string(), "application/json".to_string()));
    }
    all.extend(headers);
    HttpResponse { status, headers: all, body: body.into_bytes() }
}

fn error_response(status: u16, headers: Vec<(String, String)>, msg: &str) -> HttpResponse {
    json_response(status, headers, &serde_json::json!({ "error": msg }))
}

fn header<'a>(req: &'a HttpRequest, name: &str) -> Option<&'a str> {
    req.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
}

/// Request target without query (and fragment).
fn path_of(target: &str) -> &str {
    target.split(['?', '#']).next().unwrap_or("")
}

fn bearer_ok(auth: Option<&str>, token: &str) -> bool {
    if token.is_empty() {
        return false;
    }
    let Some(given) = auth.and_then(|a| a.strip_prefix("Bearer ")) else {
        return false;
    };
    ct_eq(given.as_bytes(), token.as_bytes())
}

/// Comparison that does not stop at the first difference.
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    let mut diff = a.len() ^ b.len();
    for i in 0..a.len().max(b.len()) {
        diff |= usize::from(a.get(i).copied().unwrap_or(0) ^ b.get(i).copied().unwrap_or(0));
    }
    diff == 0
}

fn host_loopback(h: &str) -> bool {
    if h == "::1" {
        return true;
    }
    hostname_of_authority(h).is_some_and(|n| LOOPBACK.contains(&n.as_str()))
}

fn origin_loopback(o: &str) -> bool {
    let Some((scheme, rest)) = o.split_once("://") else {
        return false;
    };
    let mut chars = scheme.chars();
    let scheme_ok = chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    scheme_ok && hostname_of_authority(rest).is_some_and(|n| LOOPBACK.contains(&n.as_str()))
}

/// Lower-cased hostname of `[userinfo@]host[:port][/...]`, `None` when a URL parser would reject it.
fn hostname_of_authority(s: &str) -> Option<String> {
    let auth = s.split(['/', '?', '#', '\\']).next().unwrap_or("");
    let auth = auth.rsplit_once('@').map_or(auth, |(_, h)| h);
    let (host, port) = if let Some(rest) = auth.strip_prefix('[') {
        let end = rest.find(']')?;
        let host = &auth[..end + 2];
        let after = &rest[end + 1..];
        (host, after.strip_prefix(':').or(if after.is_empty() { Some("") } else { None })?)
    } else {
        match auth.split_once(':') {
            Some((h, p)) => (h, p),
            None => (auth, ""),
        }
    };
    if !port.is_empty() && (!port.bytes().all(|b| b.is_ascii_digit()) || port.parse::<u32>().ok()? > 65535) {
        return None;
    }
    if host.is_empty() {
        return None;
    }
    Some(host.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::ConnId;

    fn req(method: &str, path: &str, headers: &[(&str, &str)]) -> HttpRequest {
        HttpRequest {
            conn: ConnId(1),
            method: method.into(),
            path: path.into(),
            headers: headers.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            body: Vec::new(),
            body_too_large: false,
            remote_port: 5000,
            entropy: [0; 16],
        }
    }

    const TOKEN: &str = "tok-0123456789abcdef";

    fn good() -> Vec<(&'static str, &'static str)> {
        vec![("host", "127.0.0.1:47615"), ("authorization", "Bearer tok-0123456789abcdef")]
    }

    fn body(r: &HttpResponse) -> String {
        String::from_utf8(r.body.clone()).unwrap_or_default()
    }

    #[test]
    fn f_mcpsrv_02_ok() {
        assert!(precheck(&req("POST", "/mcp", &good()), TOKEN, 47615).is_ok());
        assert!(precheck(&req("POST", "/mcp?x=1", &good()), TOKEN, 47615).is_ok());
    }

    #[test]
    fn f_mcpsrv_02_host_matrix() {
        for (h, ok) in [
            ("localhost", true),
            ("localhost:1", true),
            ("LOCALHOST:80", true),
            ("127.0.0.1", true),
            ("[::1]:80", true),
            ("[::1]", true),
            ("::1", true),
            ("evil.com", false),
            ("localhost.evil.com", false),
            ("127.0.0.1.evil.com", false),
            ("localhost:abc", false),
            ("", false),
            ("192.168.0.1", false),
        ] {
            let r = precheck(
                &req("POST", "/mcp", &[("host", h), ("authorization", "Bearer tok-0123456789abcdef")]),
                TOKEN,
                1,
            );
            assert_eq!(r.is_ok(), ok, "host {h:?}");
            if let Err(e) = r {
                assert_eq!(e.status, 403);
                assert_eq!(body(&e), "{\"error\":\"forbidden\"}");
            }
        }
        assert_eq!(
            precheck(&req("POST", "/mcp", &[("authorization", "Bearer tok-0123456789abcdef")]), TOKEN, 1)
                .map_err(|e| e.status),
            Err(403)
        );
    }

    #[test]
    fn f_mcpsrv_02_origin_matrix() {
        for (o, ok) in [
            ("http://localhost:3000", true),
            ("http://127.0.0.1", true),
            ("http://[::1]:8080", true),
            ("https://evil.com", false),
            ("null", false),
            ("", false),
            ("http://localhost.evil.com", false),
            ("file:///x", false),
        ] {
            let mut h = good();
            h.push(("origin", o));
            let r = precheck(&req("POST", "/mcp", &h), TOKEN, 1);
            assert_eq!(r.is_ok(), ok, "origin {o:?}");
        }
    }

    #[test]
    fn f_mcpsrv_02_order_and_codes() {
        // host beats path
        let r = precheck(&req("GET", "/x", &[("host", "evil")]), TOKEN, 1).unwrap_err();
        assert_eq!(r.status, 403);
        // path beats auth
        let r = precheck(&req("GET", "/x", &[("host", "localhost")]), TOKEN, 1).unwrap_err();
        assert_eq!((r.status, body(&r).as_str()), (404, "{\"error\":\"not found\"}"));
        // auth beats method
        let r = precheck(&req("GET", "/mcp", &[("host", "localhost")]), TOKEN, 1).unwrap_err();
        assert_eq!(r.status, 401);
        assert_eq!(body(&r), "{\"error\":\"unauthorized\"}");
        assert!(r.headers.contains(&("www-authenticate".into(), "Bearer".into())));
        assert!(r.headers.contains(&("content-type".into(), "application/json".into())));
        // method
        let r = precheck(&req("GET", "/mcp", &good()), TOKEN, 1).unwrap_err();
        assert_eq!(r.status, 405);
        assert_eq!(body(&r), "{\"error\":\"method not allowed\"}");
        assert!(r.headers.contains(&("allow".into(), "POST".into())));
        // too large
        let mut big = req("POST", "/mcp", &good());
        big.body_too_large = true;
        let r = precheck(&big, TOKEN, 1).unwrap_err();
        assert_eq!(r.status, 413);
        assert_eq!(body(&r), "{\"error\":\"body too large\"}");
        assert!(r.headers.contains(&("connection".into(), "close".into())));
    }

    #[test]
    fn f_mcpsrv_02_auth_forms() {
        for a in [
            "Bearer wrong",
            "bearer tok-0123456789abcdef",
            "Bearer  tok-0123456789abcdef",
            "tok-0123456789abcdef",
            "Bearer ",
        ] {
            let r = precheck(&req("POST", "/mcp", &[("host", "localhost"), ("authorization", a)]), TOKEN, 1);
            assert_eq!(r.map_err(|e| e.status), Err(401), "{a:?}");
        }
        assert_eq!(precheck(&req("POST", "/mcp", &good()), "", 1).map_err(|e| e.status), Err(401));
    }

    #[test]
    fn f_mcpsrv_02_json_response_content_type() {
        let r = json_response(200, vec![("x".into(), "y".into())], &serde_json::json!({"a":1}));
        assert_eq!(r.headers[0], ("content-type".to_string(), "application/json".to_string()));
        assert_eq!(r.headers[1].0, "x");
        let r = raw_response(202, Vec::new(), String::new());
        assert!(r.headers.is_empty() && r.body.is_empty());
    }
}
