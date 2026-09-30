//! Scenario tests (MCP server: token file, request checks, JSON-RPC envelope, tools/list).
//!
//! Not portable (no sockets): `expectRefused` before start / after stop of `f-mcpsrv-01-token-created-on-start`
//! (replaced by `mcp_endpoint()` checks).

use regex::Regex;
use serde_json::{Value, json};
use std::os::unix::fs::PermissionsExt;
use xplain_sim::{CellExpect as C, Http, Sim, SimBuilder};

const PORT: &str = "47615";
const KNOWN: &str = "e2e-known-token-0123456789";

fn base() -> SimBuilder {
    Sim::builder().env("XPLAIN_MCP_PORT", PORT)
}

/// Autostarted server with a known token (config + token file).
fn known() -> SimBuilder {
    base()
        .config_json(json!({"mcp": {"autostart": true}}))
        .file("${STATE}/xplain/mcp.json", &format!(r#"{{"token": "{KNOWN}"}}"#))
}

/// Autostarted server, generated token.
fn autostart() -> SimBuilder {
    base().config_json(json!({"mcp": {"autostart": true}}))
}

fn ping(id: i64) -> Http {
    Http::json(&json!({"jsonrpc": "2.0", "id": id, "method": "ping"}))
}

fn body(v: Value) -> Http {
    Http::json(&v)
}

/// JSON subset match: objects need only the listed keys, arrays match element-wise (same length).
fn sub(got: &Value, want: &Value) -> bool {
    match (got, want) {
        (Value::Object(g), Value::Object(w)) => w.iter().all(|(k, v)| g.get(k).is_some_and(|x| sub(x, v))),
        (Value::Array(g), Value::Array(w)) => g.len() == w.len() && g.iter().zip(w).all(|(a, b)| sub(a, b)),
        (a, b) => a == b,
    }
}

#[track_caller]
fn expect_sub(got: &Value, want: Value) {
    assert!(sub(got, &want), "got:\n{got:#}\nwant (subset):\n{want:#}");
}

#[track_caller]
fn is_json(r: &xplain_sim::HttpReply) {
    let ct = r.header("content-type").unwrap_or_default();
    assert!(ct.starts_with("application/json"), "content-type {ct:?}");
}

#[track_caller]
fn no_content_type(r: &xplain_sim::HttpReply) {
    assert!(r.header("content-type").is_none(), "content-type {:?}", r.header("content-type"));
}

#[track_caller]
fn absent(v: &Value, key: &str) {
    assert!(v.get(key).is_none(), "{key} must be absent in {v}");
}

#[track_caller]
fn re_match(pattern: &str, text: &str) {
    assert!(Regex::new(pattern).is_ok_and(|r| r.is_match(text)), "/{pattern}/ does not match {text:?}");
}

const UUID: &str = r"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$";

/// Token file written by the app: 2-space JSON + trailing newline, 43-char base64url token. Returns the token.
#[track_caller]
fn check_token_file(s: &Sim, path: &str) -> String {
    let text = s.file(path);
    for p in [r#"(?m)^\{\n  ""#, r#"(?m)^  "token": "[A-Za-z0-9_-]{43}",?$"#, r"\n\}\n$"] {
        re_match(p, &text);
    }
    let v: Value = serde_json::from_str(&text).unwrap_or_default();
    let tok = v["token"].as_str().unwrap_or_default().to_string();
    re_match(r"^[A-Za-z0-9_-]{43}$", &tok);
    tok
}

fn chmod(s: &Sim, path: &str, mode: u32) {
    let p = s.path(path);
    assert!(
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(mode)).is_ok(),
        "chmod {}",
        p.display()
    );
}

// ---------------------------------------------------------------------------------------------------------------
// F-MCPSRV-01: token file

/// Existing state dir keeps its mode; new token file still gets 0600.
#[test]
fn f_mcpsrv_01_dir_mode_existing() {
    let mut s = base().file("${STATE}/xplain/other.txt", "keep\n").build();
    chmod(&s, "${STATE}/xplain", 0o755);
    s.keys("M<Enter>");
    s.assert_row_contains(0, "[mcp: on]");
    check_token_file(&s, "${STATE}/xplain/mcp.json");
    // dir mode only applied when the app creates the dir
    assert_eq!(s.file_mode("${STATE}/xplain"), 0o755);
    assert_eq!(s.file_mode("${STATE}/xplain/mcp.json"), 0o600);
    assert_eq!(s.file("${STATE}/xplain/other.txt"), "keep\n");
}

/// Empty XDG_STATE_HOME falls back to $HOME/.local/state/xplain for the token file.
#[test]
fn f_mcpsrv_01_state_home_fallback() {
    let mut s = base()
        .env("XDG_STATE_HOME", "")
        .file("${HOME}/.local/state/xplain/mcp.json", r#"{"token":"home-fallback-token-1"}"#)
        .build();
    s.keys("M<Enter>");
    s.assert_row_contains(0, "[mcp: on]");
    assert_eq!(s.file("${HOME}/.local/state/xplain/mcp.json"), r#"{"token":"home-fallback-token-1"}"#);
    assert!(!s.file_exists("${STATE}/xplain"));
    let r = s.http(ping(1).token("home-fallback-token-1"));
    assert_eq!(r.status, 200);
    expect_sub(&r.json(), json!({"id": 1, "result": {}}));
}

/// Unset XDG_STATE_HOME creates the token file under $HOME/.local/state/xplain.
#[test]
fn f_mcpsrv_01_state_home_unset() {
    let mut s = base().env_unset("XDG_STATE_HOME").build();
    assert!(!s.file_exists("${HOME}/.local/state/xplain"));
    s.keys("M<Enter>");
    s.assert_row_contains(0, "[mcp: on]");
    // 2-space indented JSON + trailing newline; fields besides token not asserted (UNSPEC-15)
    check_token_file(&s, "${HOME}/.local/state/xplain/mcp.json");
    assert!(!s.file_exists("${STATE}/xplain"));
}

/// The token file at `path` was replaced by a fresh token (mode 0600) and the new token works.
#[track_caller]
fn replaced_token_case(content: &str, old_token: Option<&str>) {
    let mut s = base().file("${STATE}/xplain/mcp.json", content).build();
    chmod(&s, "${STATE}/xplain/mcp.json", 0o644);
    s.keys("M<Enter>");
    s.assert_row_contains(0, "[mcp: on]");
    let tok = check_token_file(&s, "${STATE}/xplain/mcp.json");
    // chmod 0600 on every write
    assert_eq!(s.file_mode("${STATE}/xplain/mcp.json"), 0o600);
    // the new token (from the file) is accepted
    let r = s.http(ping(1));
    assert_eq!(r.status, 200);
    expect_sub(&r.json(), json!({"id": 1, "result": {}}));
    assert_eq!(s.mcp_endpoint().map(|e| e.token.clone()), Some(tok));
    if let Some(old) = old_token {
        // the old token is no longer valid
        let r = s.http(ping(2).token(old));
        assert_eq!(r.status, 401);
        expect_sub(&r.json(), json!({"error": "unauthorized"}));
    }
}

/// Corrupt (not JSON) token file is replaced by a fresh 43-char token, file rewritten, mode 0600.
#[test]
fn f_mcpsrv_01_token_corrupt() {
    replaced_token_case("not json {", None);
}

/// Token file created on first start (not on launch), 2-space JSON, 43-char base64url token, dir 0700 file 0600.
#[test]
fn f_mcpsrv_01_token_created_on_start() {
    let mut s = base().size(80, 24).build();
    // launch alone does not create the state dir or token file
    assert!(!s.file_exists("${STATE}/xplain"));
    // not listening before start (no sockets in the sim: the server is simply not running)
    assert!(s.mcp_endpoint().is_none());
    s.assert_row_contains(0, "[mcp: off]");
    s.keys("M<Enter>");
    s.assert_row_contains(0, "[mcp: on]");
    s.assert_contains(&format!("● on  127.0.0.1:{PORT}"));
    let tok = check_token_file(&s, "${STATE}/xplain/mcp.json");
    // dir created with mode 0700, file 0600
    assert_eq!(s.file_mode("${STATE}/xplain"), 0o700);
    assert_eq!(s.file_mode("${STATE}/xplain/mcp.json"), 0o600);
    // the server accepts the token from the file
    assert_eq!(s.mcp_endpoint().map(|e| e.token.clone()), Some(tok));
    let r = s.http(ping(1));
    assert_eq!(r.status, 200);
    expect_sub(&r.json(), json!({"jsonrpc": "2.0", "id": 1, "result": {}}));
    // stop (modal still open) - listens only while started
    s.keys("<Space>");
    s.assert_row_contains(0, "[mcp: off]");
    assert!(s.mcp_endpoint().is_none());
}

/// Short token with extra fields token file is replaced by a fresh 43-char token.
#[test]
fn f_mcpsrv_01_token_extra_fields() {
    replaced_token_case(r#"{"port":1234,"token":"0123456789abcde","x":true}"#, Some("0123456789abcde"));
}

/// Missing token field is replaced by a fresh 43-char token.
#[test]
fn f_mcpsrv_01_token_missing() {
    replaced_token_case(r#"{"port":1234}"#, None);
}

/// Non-string token file is replaced by a fresh 43-char token.
#[test]
fn f_mcpsrv_01_token_nonstring() {
    replaced_token_case(r#"{"token":12345678901234567890}"#, Some("12345678901234567890"));
}

/// Existing token of >= 16 chars is reused, file left untouched (content and mode).
#[test]
fn f_mcpsrv_01_token_reused() {
    let mut s = base().file("${STATE}/xplain/mcp.json", r#"{"token":"0123456789abcdef"}"#).build();
    chmod(&s, "${STATE}/xplain/mcp.json", 0o644);
    s.keys("M<Enter>");
    s.assert_row_contains(0, "[mcp: on]");
    // byte-identical (compact, no trailing newline); not rewritten in 2-space form
    assert_eq!(s.file("${STATE}/xplain/mcp.json"), r#"{"token":"0123456789abcdef"}"#);
    // untouched means no chmod either
    assert_eq!(s.file_mode("${STATE}/xplain/mcp.json"), 0o644);
    let r = s.http(ping(1).token("0123456789abcdef"));
    assert_eq!(r.status, 200);
    expect_sub(&r.json(), json!({"id": 1, "result": {}}));
}

/// Short (15 chars) token file is replaced by a fresh 43-char token.
#[test]
fn f_mcpsrv_01_token_short() {
    replaced_token_case(r#"{"token":"0123456789abcde"}"#, Some("0123456789abcde"));
}

/// Token file cannot be written (state dir not writable): error row "cannot write <state dir>/mcp.json:
/// permission denied".
///
/// The sim resolves the state dir against the test process, so the absolute temp path is used (long) and the modal cuts the
/// message: only its prefix is visible.
#[test]
fn f_mcpsrv_01_write_denied() {
    let mut s = base().size(80, 24).build();
    s.mkdir("${STATE}/xplain");
    chmod(&s, "${STATE}/xplain", 0o555);
    s.keys("M<Enter>");
    s.assert_row_contains(0, "[mcp: off]");
    s.assert_matches(r"│> ○ off +│\n +│ cannot write [^\n]*│\n +│ clients 0 pending 0 delivered 0");
    chmod(&s, "${STATE}/xplain", 0o755);
    assert!(s.list_dir("${STATE}/xplain").is_empty(), "{:?}", s.list_dir("${STATE}/xplain"));
}

/// Token file cannot be written (state dir path is a file): start fails, stays off, error row.
#[test]
fn f_mcpsrv_01_write_failure() {
    let mut s = base().size(80, 24).file("${STATE}/xplain", "i am a file, not a dir\n").build();
    s.keys("M<Enter>");
    // power row still off, a non-empty error row directly below it, then the clients row
    s.assert_row_contains(0, "[mcp: off]");
    s.assert_matches(r"│> ○ off +│\n +│ cannot write .*│\n +│ clients 0 pending 0 delivered 0");
    // error row in dels color (solarized)
    s.assert_cell(10, 9, C::new().fg("#dc322f"));
    assert_eq!(s.file("${STATE}/xplain"), "i am a file, not a dir\n");
}

// ---------------------------------------------------------------------------------------------------------------
// F-MCPSRV-02: request checks

/// Authorization must be exactly "Bearer <token>" else 401 unauthorized with www-authenticate Bearer.
#[test]
fn f_mcpsrv_02_auth() {
    let mut s = known().build();
    s.assert_row_contains(0, "[mcp: on]");
    let r = s.http(ping(1).no_auth());
    assert_eq!(r.status, 401);
    is_json(&r);
    assert_eq!(r.header("www-authenticate"), Some("Bearer"));
    expect_sub(&r.json(), json!({"error": "unauthorized"}));
    let r = s.http(ping(2).token("wrong-token-0123456789"));
    assert_eq!(r.status, 401);
    assert_eq!(r.header("www-authenticate"), Some("Bearer"));
    expect_sub(&r.json(), json!({"error": "unauthorized"}));
    for (i, raw) in [
        "bearer e2e-known-token-0123456789",
        "e2e-known-token-0123456789",
        "Bearer e2e-known-token-012345678",
        "Bearer e2e-known-token-01234567890",
    ]
    .into_iter()
    .enumerate()
    {
        let r = s.http(ping(3 + i as i64).no_auth().header("authorization", raw));
        assert_eq!(r.status, 401, "{raw}");
        expect_sub(&r.json(), json!({"error": "unauthorized"}));
    }
    // auth check comes before method check
    let r = s.http(ping(7).no_auth().method("GET"));
    assert_eq!(r.status, 401);
    expect_sub(&r.json(), json!({"error": "unauthorized"}));
    let r = s.http(ping(8).token(KNOWN));
    assert_eq!(r.status, 200);
    expect_sub(&r.json(), json!({"id": 8, "result": {}}));
}

/// Host header must be a loopback name (localhost, 127.0.0.1, [::1]) else 403 forbidden, checked before path and
/// auth.
#[test]
fn f_mcpsrv_02_host() {
    let mut s = known().build();
    s.assert_row_contains(0, "[mcp: on]");
    let r = s.http(ping(1).header("host", &format!("evil.example:{PORT}")));
    assert_eq!(r.status, 403);
    is_json(&r);
    let b = r.json();
    expect_sub(&b, json!({"error": "forbidden"}));
    absent(&b, "jsonrpc");
    let r = s.http(ping(2).header("host", "localhost.evil.example"));
    assert_eq!(r.status, 403);
    expect_sub(&r.json(), json!({"error": "forbidden"}));
    // 403 wins over wrong path, missing auth and wrong method
    let r = s.http(ping(3).header("host", "evil.example").path("/nope").no_auth().method("GET"));
    assert_eq!(r.status, 403);
    expect_sub(&r.json(), json!({"error": "forbidden"}));
    for (id, host) in [
        (4, format!("localhost:{PORT}")),
        (5, "localhost".to_string()),
        (6, format!("127.0.0.1:{PORT}")),
        (7, format!("[::1]:{PORT}")),
    ] {
        let r = s.http(ping(id).header("host", &host));
        assert_eq!(r.status, 200, "{host}");
        expect_sub(&r.json(), json!({"id": id, "result": {}}));
    }
    let r = s.http(ping(8).header("host", "127.0.0.2"));
    assert_eq!(r.status, 403);
    expect_sub(&r.json(), json!({"error": "forbidden"}));
}

/// Authorized non-POST request gives 405 method not allowed with allow POST.
#[test]
fn f_mcpsrv_02_method() {
    let mut s = known().build();
    s.assert_row_contains(0, "[mcp: on]");
    let r = s.http(ping(1).method("GET"));
    assert_eq!(r.status, 405);
    is_json(&r);
    assert_eq!(r.header("allow"), Some("POST"));
    expect_sub(&r.json(), json!({"error": "method not allowed"}));
    for (id, m) in [(2, "DELETE"), (3, "PUT")] {
        let r = s.http(ping(id).method(m));
        assert_eq!(r.status, 405, "{m}");
        assert_eq!(r.header("allow"), Some("POST"));
        expect_sub(&r.json(), json!({"error": "method not allowed"}));
    }
    let r = s.http(ping(4));
    assert_eq!(r.status, 200);
    expect_sub(&r.json(), json!({"id": 4, "result": {}}));
}

/// Origin present with non-loopback or unparseable hostname gives 403 forbidden; loopback origins pass.
#[test]
fn f_mcpsrv_02_origin() {
    let mut s = known().build();
    s.assert_row_contains(0, "[mcp: on]");
    let r = s.http(ping(1).header("origin", "http://evil.example"));
    assert_eq!(r.status, 403);
    is_json(&r);
    expect_sub(&r.json(), json!({"error": "forbidden"}));
    // unparseable origin counts as bad
    let r = s.http(ping(2).header("origin", "not a url"));
    assert_eq!(r.status, 403);
    expect_sub(&r.json(), json!({"error": "forbidden"}));
    // 403 wins over missing auth
    let r = s.http(ping(3).header("origin", "https://evil.example:8443").no_auth());
    assert_eq!(r.status, 403);
    expect_sub(&r.json(), json!({"error": "forbidden"}));
    for (id, o) in [
        (4, "http://localhost:3000".to_string()),
        (5, format!("http://127.0.0.1:{PORT}")),
        (6, "http://[::1]:9999".to_string()),
    ] {
        let r = s.http(ping(id).header("origin", &o));
        assert_eq!(r.status, 200, "{o}");
        expect_sub(&r.json(), json!({"id": id, "result": {}}));
    }
}

/// Body that is not JSON gives 400 JSON-RPC parse error with id null.
#[test]
fn f_mcpsrv_02_parse_error() {
    let mut s = known().build();
    s.assert_row_contains(0, "[mcp: on]");
    let want = json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": "Parse error"}});
    let r = s.http(Http::raw("not json"));
    assert_eq!(r.status, 400);
    is_json(&r);
    expect_sub(&r.json(), want.clone());
    absent(&r.json(), "result");
    for raw in [r#"{"jsonrpc":"2.0","id":1,"method":"ping""#, ""] {
        let r = s.http(Http::raw(raw));
        assert_eq!(r.status, 400, "{raw:?}");
        expect_sub(&r.json(), want.clone());
    }
    // body of exactly 1048576 bytes is read (then fails to parse)
    let r = s.http(Http::raw(vec![b'x'; 1_048_576]));
    assert_eq!(r.status, 400);
    expect_sub(&r.json(), want);
    // body over 1048576 bytes - 413, connection closed
    let r = s.http(Http::oversized());
    assert_eq!(r.status, 413);
    assert_eq!(r.header("connection"), Some("close"));
    is_json(&r);
    expect_sub(&r.json(), json!({"error": "body too large"}));
    // parse check comes after the auth check
    let r = s.http(Http::raw("not json").no_auth());
    assert_eq!(r.status, 401);
    expect_sub(&r.json(), json!({"error": "unauthorized"}));
    // the server still works afterwards
    let r = s.http(ping(1));
    assert_eq!(r.status, 200);
    expect_sub(&r.json(), json!({"result": {}}));
}

/// Path other than /mcp gives 404 not found (before auth and method checks); query string ignored.
#[test]
fn f_mcpsrv_02_path() {
    let mut s = known().build();
    s.assert_row_contains(0, "[mcp: on]");
    let r = s.http(ping(1).path("/"));
    assert_eq!(r.status, 404);
    is_json(&r);
    expect_sub(&r.json(), json!({"error": "not found"}));
    for (id, p) in [(2, "/mcp/extra"), (3, "/MCP")] {
        let r = s.http(ping(id).path(p));
        assert_eq!(r.status, 404, "{p}");
        expect_sub(&r.json(), json!({"error": "not found"}));
    }
    // path check comes before auth and method
    let r = s.http(ping(4).path("/other").no_auth().method("GET"));
    assert_eq!(r.status, 404);
    expect_sub(&r.json(), json!({"error": "not found"}));
    let r = s.http(ping(5).path("/mcp?session=1&x=y"));
    assert_eq!(r.status, 200);
    expect_sub(&r.json(), json!({"id": 5, "result": {}}));
}

// ---------------------------------------------------------------------------------------------------------------
// F-MCPSRV-03: JSON-RPC envelope

fn qtext(v: &Value) -> String {
    v["result"]["content"][0]["text"].as_str().unwrap_or_default().to_string()
}

/// Batch items run sequentially - a long poll blocks the later annotate until a question arrives.
#[test]
fn f_mcpsrv_03_batch_sequential() {
    let mut s = autostart().size(80, 24).build();
    s.assert_row_contains(0, "[mcp: on]");
    let batch = s.http_start(body(json!([
        {"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "next_question", "arguments": {"wait_seconds": 120}}},
        {"jsonrpc": "2.0", "id": 2, "method": "tools/call",
         "params": {"name": "annotate", "arguments": {"file": "README.md", "line": 2, "text": "after the poll"}}},
    ])));
    // annotate not applied while the poll waits (it has no question yet)
    s.assert_not_contains("after the poll");
    s.keys("a");
    s.keys("why?<Enter>");
    let r = s.http_await(&batch);
    assert_eq!(r.status, 200);
    let b = r.json();
    let items = b.as_array().cloned().unwrap_or_default();
    assert_eq!(items.len(), 2, "{b}");
    assert_eq!(items[0]["id"], 1);
    assert!(qtext(&items[0]).starts_with(r#"{"status":"question","thread_id":"q1""#), "{}", qtext(&items[0]));
    assert_eq!(items[1]["id"], 2);
    assert_eq!(qtext(&items[1]), r#"{"ok":true}"#);
    s.assert_contains("after the poll");
}

/// Batch body gives an array response in request order; one-item batch still an array.
#[test]
fn f_mcpsrv_03_batch() {
    let mut s = autostart().build();
    s.assert_row_contains(0, "[mcp: on]");
    let r = s.http(body(json!([
        {"jsonrpc": "2.0", "id": 11, "method": "tools/list"},
        {"jsonrpc": "2.0", "id": "b", "method": "ping"},
        {"jsonrpc": "2.0", "id": 13, "method": "no/such"},
    ])));
    assert_eq!(r.status, 200);
    is_json(&r);
    let b = r.json();
    let items = b.as_array().cloned().unwrap_or_default();
    assert_eq!(items.len(), 3, "{b}");
    assert_eq!(items[0]["jsonrpc"], "2.0");
    assert_eq!(items[0]["id"], 11);
    assert_eq!(items[0]["result"]["tools"].as_array().map(Vec::len), Some(5));
    expect_sub(&items[1], json!({"jsonrpc": "2.0", "id": "b", "result": {}}));
    absent(&items[1], "error");
    expect_sub(
        &items[2],
        json!({"jsonrpc": "2.0", "id": 13, "error": {"code": -32601, "message": "Method not found: no/such"}}),
    );
    let r = s.http(body(json!([{"jsonrpc": "2.0", "id": 1, "method": "ping"}])));
    assert_eq!(r.status, 200);
    expect_sub(&r.json(), json!([{"jsonrpc": "2.0", "id": 1, "result": {}}]));
}

/// Response id echoes request id as sent (number, string, zero, null); jsonrpc field not checked.
#[test]
fn f_mcpsrv_03_id_echo() {
    let mut s = autostart().build();
    s.assert_row_contains(0, "[mcp: on]");
    let r = s.http(body(json!({"jsonrpc": "2.0", "id": "abc-XYZ", "method": "ping"})));
    assert_eq!(r.status, 200);
    expect_sub(&r.json(), json!({"jsonrpc": "2.0", "id": "abc-XYZ", "result": {}}));
    let r = s.http(body(json!({"jsonrpc": "2.0", "id": 0, "method": "ping"})));
    assert_eq!(r.status, 200);
    expect_sub(&r.json(), json!({"id": 0, "result": {}}));
    let r = s.http(body(json!({"jsonrpc": "2.0", "id": 987_654, "method": "ping"})));
    assert_eq!(r.status, 200);
    expect_sub(&r.json(), json!({"id": 987_654, "result": {}}));
    // "id": null is a request, not a notification
    let r = s.http(body(json!({"jsonrpc": "2.0", "id": null, "method": "ping"})));
    assert_eq!(r.status, 200);
    expect_sub(&r.json(), json!({"jsonrpc": "2.0", "id": null, "result": {}}));
    // jsonrpc not checked
    let r = s.http(body(json!({"jsonrpc": "1.0", "id": 5, "method": "ping"})));
    assert_eq!(r.status, 200);
    expect_sub(&r.json(), json!({"id": 5, "result": {}}));
    absent(&r.json(), "error");
    let r = s.http(body(json!({"id": 6, "method": "ping"})));
    assert_eq!(r.status, 200);
    expect_sub(&r.json(), json!({"id": 6, "result": {}}));
    absent(&r.json(), "error");
}

/// Item that is not an object or has a non-string method gives -32600 Invalid Request (id or null).
#[test]
fn f_mcpsrv_03_invalid_request() {
    let mut s = autostart().build();
    s.assert_row_contains(0, "[mcp: on]");
    let r = s.http(body(json!({"jsonrpc": "2.0", "id": 7, "method": 5})));
    assert_eq!(r.status, 200);
    expect_sub(
        &r.json(),
        json!({"jsonrpc": "2.0", "id": 7, "error": {"code": -32600, "message": "Invalid Request"}}),
    );
    absent(&r.json(), "result");
    // item without id still answered (id null) when invalid
    let r = s.http(body(json!({"jsonrpc": "2.0", "params": {}})));
    assert_eq!(r.status, 200);
    expect_sub(
        &r.json(),
        json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32600, "message": "Invalid Request"}}),
    );
    let r = s.http(body(json!([
        1,
        "str",
        null,
        [{"jsonrpc": "2.0", "id": 1, "method": "ping"}],
        {"jsonrpc": "2.0", "id": "m", "method": ["ping"]},
        {"jsonrpc": "2.0", "method": 3},
        {"jsonrpc": "2.0", "id": 9, "method": "ping"},
    ])));
    assert_eq!(r.status, 200);
    let bad = |id: Value| json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32600, "message": "Invalid Request"}});
    expect_sub(
        &r.json(),
        json!([
            bad(json!(null)),
            bad(json!(null)),
            bad(json!(null)),
            bad(json!(null)),
            bad(json!("m")),
            bad(json!(null)),
            {"jsonrpc": "2.0", "id": 9, "result": {}},
        ]),
    );
}

/// Items without id are notifications (no response item, even unknown methods); all-notification body or []
/// gives 202 empty.
#[test]
fn f_mcpsrv_03_notifications() {
    let mut s = autostart().build();
    s.assert_row_contains(0, "[mcp: on]");
    for b in [
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({"jsonrpc": "2.0", "method": "totally/unknown"}),
        json!([]),
        json!([{"jsonrpc": "2.0", "method": "notifications/initialized"}, {"jsonrpc": "2.0", "method": "ping"}]),
    ] {
        let r = s.http(body(b.clone()));
        assert_eq!(r.status, 202, "{b}");
        no_content_type(&r);
        assert_eq!(r.text(), "", "{b}");
    }
    // notifications dropped from a mixed batch, order of the rest kept
    let r = s.http(body(json!([
        {"jsonrpc": "2.0", "method": "nope/x"},
        {"jsonrpc": "2.0", "id": 1, "method": "ping"},
        {"jsonrpc": "2.0", "method": "ping"},
        {"jsonrpc": "2.0", "id": 2, "method": "nope/y"},
    ])));
    assert_eq!(r.status, 200);
    expect_sub(
        &r.json(),
        json!([
            {"id": 1, "result": {}},
            {"id": 2, "error": {"code": -32601, "message": "Method not found: nope/y"}},
        ]),
    );
}

/// Unknown method gives -32601 Method not found with the method name cut to 100 chars.
#[test]
fn f_mcpsrv_03_unknown_method() {
    let mut s = autostart().build();
    s.assert_row_contains(0, "[mcp: on]");
    let r = s.http(body(json!({"jsonrpc": "2.0", "id": 3, "method": "resources/list"})));
    assert_eq!(r.status, 200);
    is_json(&r);
    expect_sub(
        &r.json(),
        json!({"jsonrpc": "2.0", "id": 3, "error": {"code": -32601, "message": "Method not found: resources/list"}}),
    );
    absent(&r.json(), "result");
    // 150-char method name, message keeps the first 100
    let r = s.http(body(json!({"jsonrpc": "2.0", "id": 4, "method": "m".repeat(150)})));
    assert_eq!(r.status, 200);
    expect_sub(
        &r.json(),
        json!({"id": 4, "error": {"code": -32601, "message": format!("Method not found: {}", "m".repeat(100))}}),
    );
}

// ---------------------------------------------------------------------------------------------------------------
// F-MCPSRV-04: methods

/// Later items of a batch use the session of an earlier initialize (answer divider shows its client name).
#[test]
fn f_mcpsrv_04_batch_session() {
    let mut s = autostart().size(80, 24).build();
    s.assert_row_contains(0, "[mcp: on]");
    s.keys("a");
    s.keys("what does this do?<Enter>");
    s.assert_row_contains(-1, "question sent to agent");
    let r = s.http(body(json!([
        {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"clientInfo": {"name": "batch-bot", "version": "9"}}},
        {"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "next_question", "arguments": {"wait_seconds": 1}}},
    ])));
    assert_eq!(r.status, 200);
    let b = r.json();
    let items = b.as_array().cloned().unwrap_or_default();
    assert_eq!(items.len(), 2, "{b}");
    expect_sub(&items[0], json!({"id": 1, "result": {"serverInfo": {"name": "xplain"}}}));
    assert_eq!(items[1]["id"], 2);
    assert!(qtext(&items[1]).starts_with(r#"{"status":"question","thread_id":"q1""#), "{}", qtext(&items[1]));
    // delivered question shows the agent name of the batch's session
    s.assert_contains("─ answer · batch-bot · streaming…");
}

/// Client name from clientInfo is sanitized (ANSI escapes and control chars stripped).
#[test]
fn f_mcpsrv_04_client_sanitize() {
    let mut s = autostart().size(80, 24).build();
    s.assert_row_contains(0, "[mcp: on]");
    s.keys("a");
    s.keys("explain<Enter>");
    let r = s.http(body(json!([
        {"jsonrpc": "2.0", "id": 1, "method": "initialize",
         "params": {"clientInfo": {"name": "sa\u{1b}[31mfe\u{1}\u{8}bot", "version": "1"}}},
        {"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "next_question", "arguments": {"wait_seconds": 1}}},
    ])));
    assert_eq!(r.status, 200);
    let b = r.json();
    let items = b.as_array().cloned().unwrap_or_default();
    assert_eq!(items.len(), 2, "{b}");
    assert_eq!(items[0]["id"], 1);
    assert_eq!(items[1]["id"], 2);
    assert!(qtext(&items[1]).contains(r#""status":"question""#));
    s.assert_contains("─ answer · safebot · streaming…");
    s.assert_not_contains("[31m");
    s.keys("<Esc>M");
    s.assert_matches(r"│   safebot 1 +│");
}

/// Each initialize adds a client entry named from clientInfo (non-string name unknown, version empty).
#[test]
fn f_mcpsrv_04_clients() {
    let mut s = autostart().size(80, 24).build();
    s.assert_row_contains(0, "[mcp: on]");
    let r = s.http(body(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {"protocolVersion": "2025-06-18", "clientInfo": {"name": "alpha-agent", "version": "1.2.3"}}})));
    assert_eq!(r.status, 200);
    expect_sub(&r.json(), json!({"id": 1, "result": {"serverInfo": {"name": "xplain"}}}));
    let r = s.http(body(json!({"jsonrpc": "2.0", "id": 2, "method": "initialize",
        "params": {"clientInfo": {"name": 7, "version": 8}}})));
    assert_eq!(r.status, 200);
    expect_sub(&r.json(), json!({"id": 2, "result": {"serverInfo": {"name": "xplain"}}}));
    s.keys("M");
    s.assert_contains(" clients 2 pending 0 delivered 0");
    s.assert_matches(r"│   alpha-agent 1\.2\.3 +│");
    s.assert_matches(r"│   unknown +│");
    s.keys("<Esc>");
    // same client info again = a new entry
    let r = s.http(body(json!({"jsonrpc": "2.0", "id": 3, "method": "initialize",
        "params": {"clientInfo": {"name": "alpha-agent", "version": "1.2.3"}}})));
    assert_eq!(r.status, 200);
    s.keys("M");
    s.assert_contains(" clients 3 pending 0 delivered 0");
}

const INSTRUCTIONS: &str = "xplain shows the user a live diff of the working tree. REQUIRED: after EVERY edit, create, rename or delete of a file on disk, call the `files_changed` tool (pass the changed paths). Without this call the xplain view stays stale and the user does not see your changes. Batch several edits into one call, but always call it before you answer or go idle. Questions from the user arrive via `next_question`; answer with `answer`.";

/// Initialize returns protocol version, capabilities, serverInfo, verbatim instructions and a UUID mcp-session-id.
#[test]
fn f_mcpsrv_04_initialize() {
    let mut s = autostart().build();
    s.assert_row_contains(0, "[mcp: on]");
    let r = s.http(body(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "e2e", "version": "1"}}})));
    assert_eq!(r.status, 200);
    is_json(&r);
    re_match(UUID, r.header("mcp-session-id").unwrap_or_default());
    expect_sub(
        &r.json(),
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "protocolVersion": "2025-06-18",
                "capabilities": {"tools": {"listChanged": false}},
                "serverInfo": {"name": "xplain", "version": "0.1.0"},
                "instructions": INSTRUCTIONS,
            },
        }),
    );
    // other supported versions echoed back
    let r = s.http(body(json!({"jsonrpc": "2.0", "id": 2, "method": "initialize", "params": {"protocolVersion": "2025-03-26"}})));
    assert_eq!(r.status, 200);
    re_match(UUID, r.header("mcp-session-id").unwrap_or_default());
    expect_sub(
        &r.json(),
        json!({"id": 2, "result": {"protocolVersion": "2025-03-26", "serverInfo": {"name": "xplain"}}}),
    );
    let r = s.http(body(json!({"jsonrpc": "2.0", "id": 3, "method": "initialize", "params": {"protocolVersion": "2024-11-05"}})));
    assert_eq!(r.status, 200);
    expect_sub(&r.json(), json!({"id": 3, "result": {"protocolVersion": "2024-11-05"}}));
    // unsupported or missing version falls back to 2025-06-18
    let r = s.http(body(json!({"jsonrpc": "2.0", "id": 4, "method": "initialize", "params": {"protocolVersion": "2099-01-01"}})));
    assert_eq!(r.status, 200);
    expect_sub(
        &r.json(),
        json!({"id": 4, "result": {"protocolVersion": "2025-06-18", "capabilities": {"tools": {"listChanged": false}}}}),
    );
    let r = s.http(body(json!({"jsonrpc": "2.0", "id": 5, "method": "initialize"})));
    assert_eq!(r.status, 200);
    re_match(UUID, r.header("mcp-session-id").unwrap_or_default());
    expect_sub(
        &r.json(),
        json!({"id": 5, "result": {"protocolVersion": "2025-06-18", "instructions": INSTRUCTIONS}}),
    );
    // the returned mcp-session-id identifies the client - a question polled with it shows under its name
    let r = s.mcp_rpc(
        "initialize",
        json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "idbot", "version": "1"}}),
    );
    assert_eq!(r.status, 200);
    let sid = r.header("mcp-session-id").unwrap_or_default().to_string();
    s.keys("a");
    s.keys("who asks?<Enter>");
    let r = s.http(Http::tool("next_question", json!({"wait_seconds": 1})).session(&sid));
    assert_eq!(r.status, 200);
    let q = r.tool_result();
    assert_eq!(q["status"], "question");
    assert!(q["question"].as_str().is_some_and(|t| t.contains("who asks?")), "{q}");
    s.assert_contains("─ answer · idbot · streaming… ─");
}

/// Ping returns an empty result object.
#[test]
fn f_mcpsrv_04_ping() {
    let mut s = autostart().build();
    s.assert_row_contains(0, "[mcp: on]");
    let r = s.http(body(json!({"jsonrpc": "2.0", "id": 42, "method": "ping"})));
    assert_eq!(r.status, 200);
    is_json(&r);
    let b = r.json();
    expect_sub(&b, json!({"jsonrpc": "2.0", "id": 42, "result": {}}));
    absent(&b["result"], "protocolVersion");
    absent(&b["result"], "tools");
    absent(&b, "error");
}

/// tools/list returns the 5 tools in order with verbatim descriptions and input schemas.
#[test]
fn f_mcpsrv_04_tools_list() {
    let mut s = autostart().build();
    s.assert_row_contains(0, "[mcp: on]");
    let r = s.http(body(json!({"jsonrpc": "2.0", "id": "tl", "method": "tools/list"})));
    assert_eq!(r.status, 200);
    is_json(&r);
    let b = r.json();
    let fenced = "Put code samples in markdown fenced blocks (```lang ... ```): xplain highlights them and gives the user a copy button.";
    // get_questions: properties has none of thread_id, wait_seconds, file, paths
    let tools = b["result"]["tools"].as_array().cloned().unwrap_or_default();
    assert_eq!(tools.len(), 5, "{b}");
    let gq = &tools[2];
    let props = gq["inputSchema"]["properties"].as_object().cloned().unwrap_or_default();
    for k in ["thread_id", "wait_seconds", "file", "paths"] {
        assert!(!props.contains_key(k), "get_questions property {k}");
    }
    assert_eq!(gq["name"], "get_questions");
    assert_eq!(
        gq["description"],
        "List questions (and follow-ups, marked follow_up) still waiting to be delivered, without consuming them. Does not replace the next_question loop."
    );
    assert_eq!(gq["inputSchema"]["type"], "object");
    assert_eq!(gq["inputSchema"]["additionalProperties"], false);
    // the other four in full (arrays match element-wise with equal length; get_questions checked above)
    let mut got = b.clone();
    got["result"]["tools"][2] = json!(null);
    let want = json!({
        "jsonrpc": "2.0",
        "id": "tl",
        "result": {"tools": [
            {
                "name": "next_question",
                "description": "Long-poll for the next question a human asked in the xplain code-review UI. Returns {status:\"question\", thread_id, turn, follow_up, question}: answer it with the answer tool. If follow_up is true it continues an earlier thread: use `previous` (earlier questions and your answers) for reference and answer with the SAME thread_id. A follow-up can also be the user replying to a note you added with annotate: the question then names the note (file, line, text). If status is \"no_question_yet\", call next_question again immediately. Keep looping: after every answer, call next_question again immediately, until status is \"closed\".",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "wait_seconds": {
                            "type": "integer",
                            "minimum": 1,
                            "maximum": 120,
                            "default": 45,
                            "description": "Max seconds to wait for a question (1-120).",
                        },
                    },
                    "additionalProperties": false,
                },
            },
            {
                "name": "answer",
                "description": format!("Send your answer for a question received from next_question, using its thread_id (for a follow-up, the same thread_id as before). Plain text or markdown. {fenced} Then call next_question again immediately."),
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "thread_id": {"type": "string", "description": "thread_id from next_question."},
                        "text": {"type": "string", "description": format!("The answer text. {fenced}")},
                    },
                    "required": ["thread_id", "text"],
                    "additionalProperties": false,
                },
            },
            null,
            {
                "name": "annotate",
                "description": "Attach a note to a line of a file in the xplain diff view. The user may reply to it; replies arrive via next_question as follow-ups. Then continue the next_question loop.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "file": {"type": "string", "description": "File path as shown in the diff."},
                        "line": {"type": "number", "description": "1-based line number."},
                        "text": {"type": "string", "description": format!("Annotation text. {fenced}")},
                        "side": {"type": "string", "enum": ["old", "new"], "description": "Diff side; default new."},
                        "number": {
                            "type": "integer",
                            "description": "Optional order label shown on the note (e.g. step 1, 2, 3); the user jumps between numbered notes in order.",
                        },
                    },
                    "required": ["file", "line", "text"],
                    "additionalProperties": false,
                },
            },
            {
                "name": "files_changed",
                "description": "REQUIRED after every file change: you MUST call this after each edit, creation, rename or deletion of any file on disk (also right after a batch of edits, before replying). It makes the xplain diff view reload; without it the user sees stale content. Pass the changed paths in `paths`. Then continue the next_question loop.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "paths": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "Paths of the files you edited, created or deleted (relative to the repo root).",
                        },
                    },
                    "additionalProperties": false,
                },
            },
        ]},
    });
    assert_eq!(got, want);
}

// ---------------------------------------------------------------------------------------------------------------
// F-MCPSRV-05: tools/call shape

/// Non-object arguments are treated as {}.
#[test]
fn f_mcpsrv_05_arguments_non_object() {
    let mut s = autostart().build();
    s.assert_row_contains(0, "[mcp: on]");
    let call = |id: i64, params: Value| {
        body(json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": params}))
    };
    let r = s.http(call(1, json!({"name": "get_questions", "arguments": "x"})));
    assert_eq!(r.status, 200);
    expect_sub(
        &r.json(),
        json!({"id": 1, "result": {"content": [{"type": "text", "text": "{\"questions\":[]}"}]}}),
    );
    let r = s.http(call(2, json!({"name": "files_changed", "arguments": ["README.md"]})));
    assert_eq!(r.status, 200);
    absent(&r.json(), "error");
    expect_sub(
        &r.json(),
        json!({"id": 2, "result": {"content": [{"type": "text", "text": "{\"ok\":true}"}]}}),
    );
    let r = s.http(call(3, json!({"name": "get_questions"})));
    assert_eq!(r.status, 200);
    expect_sub(
        &r.json(),
        json!({"id": 3, "result": {"content": [{"type": "text", "text": "{\"questions\":[]}"}]}}),
    );
    // annotate with an array argument sees {} - required-field tool error, no note added
    let r = s.http(call(4, json!({"name": "annotate", "arguments": ["README.md", 1, "hi"]})));
    assert_eq!(r.status, 200);
    absent(&r.json(), "error");
    expect_sub(
        &r.json(),
        json!({"id": 4, "result": {
            "isError": true,
            "content": [{"type": "text", "text": "file (string), line (number) and text (string) are required"}],
        }}),
    );
}

/// Tool results are content [{type text, text}] with compact JSON text; tool errors add isError true.
#[test]
fn f_mcpsrv_05_result_shape() {
    let mut s = autostart().build();
    s.assert_row_contains(0, "[mcp: on]");
    let call = |id: i64, params: Value| {
        body(json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": params}))
    };
    let r = s.http(call(1, json!({"name": "get_questions", "arguments": {}})));
    assert_eq!(r.status, 200);
    expect_sub(
        &r.json(),
        json!({"jsonrpc": "2.0", "id": 1, "result": {"content": [{"type": "text", "text": "{\"questions\":[]}"}]}}),
    );
    absent(&r.json()["result"], "isError");
    let r = s.http(call(
        2,
        json!({"name": "annotate", "arguments": {"file": "README.md", "line": 2, "text": "shape note"}}),
    ));
    assert_eq!(r.status, 200);
    expect_sub(
        &r.json(),
        json!({"id": 2, "result": {"content": [{"type": "text", "text": "{\"ok\":true}"}]}}),
    );
    absent(&r.json()["result"], "isError");
    s.assert_contains("shape note");
    // tool error (missing required args) is a result with isError, not a JSON-RPC error
    let r = s.http(call(3, json!({"name": "answer", "arguments": {}})));
    assert_eq!(r.status, 200);
    let b = r.json();
    assert_eq!(b["id"], 3);
    absent(&b, "error");
    assert_eq!(b["result"]["isError"], true);
    assert_eq!(b["result"]["content"][0]["type"], "text");
    assert!(!qtext(&b).is_empty());
}

/// Agent text is sanitized - ANSI escapes and control chars (C0 incl. VT FF CR NUL, DEL, C1) stripped, newline and
/// tab kept.
#[test]
fn f_mcpsrv_05_sanitize() {
    let mut s = autostart().size(100, 30).build();
    s.assert_row_contains(0, "[mcp: on]");
    // raw, "\e[3D" would move the terminal cursor back and "\b" too, eating chars; stripped they vanish
    let r = s.mcp_call(
        "annotate",
        json!({"file": "README.md", "line": 2,
               "text": "keep\u{1b}[3DXYZ\u{1b}[31mred\u{1b}[0m\u{1}\u{7f}\u{8}END\nsecond line"}),
    );
    assert_eq!(r.status, 200);
    assert_eq!(r.tool_result(), json!({"ok": true}));
    s.assert_contains("keepXYZredEND");
    s.assert_contains("second line");
    none(&s, &["[3D", "[31m", "[0m"]);
    s.assert_matches(r"keepXYZredEND *│?\n.*second line");
    // box default text color (modalFg) - a passed-through SGR 31 would turn "red" palette red
    s.assert_text_cell("keepXYZredEND", 7, C::new().ch('r').fg("#93a1a1"));
    s.assert_text_cell("keepXYZredEND", 10, C::new().ch('E').fg("#93a1a1"));
    // VT, FF, CR, NUL, US and C1 controls (U+0085, U+009F) stripped, TAB kept; checked in the export file, since
    // the screen can hide CR (answer render drops it) and VT/FF (terminal line moves)
    let r = s.mcp_call(
        "annotate",
        json!({"file": "README.md", "line": 1, "text": "V\u{b}F\u{c}C\rN\0U\u{1f}L\u{85}K\u{9f}END\tTAB"}),
    );
    assert_eq!(r.status, 200);
    assert_eq!(r.tool_result(), json!({"ok": true}));
    s.assert_contains("VFCNULKEND  TAB");
    s.keys("E");
    s.assert_row_matches(-1, r"^exported 2 comments -> ");
    let files: Vec<String> = s
        .list_dir(".")
        .into_iter()
        .filter(|f| f.starts_with("xplain-review-") && f.ends_with(".md"))
        .collect();
    assert_eq!(files.len(), 1, "{files:?}");
    let text = s.file(&files[0]);
    for want in ["\n> VFCNULKEND\tTAB\n", "\n> keepXYZredEND\n> second line\n"] {
        assert!(text.contains(want), "export lacks {want:?}:\n{text}");
    }
    for bad in ["\u{b}", "\u{c}", "\r", "\0", "\u{1f}", "\u{85}", "\u{9f}", "\u{1b}"] {
        assert!(!text.contains(bad), "export contains {bad:?}");
    }
}

fn none(s: &Sim, texts: &[&str]) {
    for t in texts {
        s.assert_not_contains(t);
    }
}

/// tools/call with unknown or non-string tool name gives -32602 Unknown tool (name cut to 100 chars).
#[test]
fn f_mcpsrv_05_unknown_tool() {
    let mut s = autostart().build();
    s.assert_row_contains(0, "[mcp: on]");
    let call = |id: i64, params: Value| {
        body(json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": params}))
    };
    let r = s.http(call(1, json!({"name": "no_such_tool", "arguments": {}})));
    assert_eq!(r.status, 200);
    expect_sub(
        &r.json(),
        json!({"jsonrpc": "2.0", "id": 1, "error": {"code": -32602, "message": "Unknown tool: no_such_tool"}}),
    );
    absent(&r.json(), "result");
    let r = s.http(call(2, json!({"name": 5})));
    assert_eq!(r.status, 200);
    expect_sub(&r.json(), json!({"id": 2, "error": {"code": -32602, "message": "Unknown tool: "}}));
    let r = s.http(call(3, json!({"arguments": {}})));
    assert_eq!(r.status, 200);
    expect_sub(&r.json(), json!({"id": 3, "error": {"code": -32602, "message": "Unknown tool: "}}));
    let r = s.http(call(4, json!({"name": "n".repeat(150)})));
    assert_eq!(r.status, 200);
    expect_sub(
        &r.json(),
        json!({"id": 4, "error": {"code": -32602, "message": format!("Unknown tool: {}", "n".repeat(100))}}),
    );
}

// ---------------------------------------------------------------------------------------------------------------
// F-NAV-08

/// Numbered jump `)` to a note in another file places like a file switch - top = firstChange-3, col 1, then follow
/// to the note row.
///
/// src/big.ts: 62 rows, first change start = row 30 (del v30), so top = 27. Note on new line 34 (row 35) is already
/// in view with margin, so top stays 27 (keeping README's top 0 and following would give top 22 instead).
#[test]
fn f_nav_08_numbered_jump() {
    let mut s = autostart().size(80, 24).build();
    s.assert_row_contains(0, "[mcp: on]");
    let r =
        s.mcp_call("annotate", json!({"file": "src/big.ts", "line": 34, "text": "look here", "number": 1}));
    assert_eq!(r.status, 200);
    assert_eq!(r.tool_result(), json!({"ok": true}));
    s.keys("2l");
    s.assert_row_matches(0, r"\[1/4\] \[cursor L2:C3\] README\.md");
    s.keys(")");
    s.assert_row_matches(0, r"\[2/4\] \[cursor L34:C1\] src/big\.ts ");
    s.assert_row_matches(2, r"^  27   27   const v27 = 1;");
    s.assert_row_matches(10, r"^  34   34  ▶const v34 = 1;");
    s.assert_row_contains(12, "#1 agent note L34");
    s.assert_row_matches(-1, r"^\(28-48/62\) ");
}
