//! In-process MCP HTTP: request builder and captured reply. No sockets: the request goes to core as
//! `Event::McpHttp`, the reply is the `Effect::HttpReply` for its connection id.

use serde_json::{Value, json};
use xplain_core::mcp::{ConnId, HttpResponse};

/// One request to send. Defaults: `POST /mcp`, `host: 127.0.0.1:<port>`, `content-type: application/json`,
/// `accept: application/json, text/event-stream`, `authorization: Bearer <token from mcp.json>`.
#[derive(Debug, Clone)]
pub struct Http {
    pub(crate) method: String,
    pub(crate) path: String,
    pub(crate) headers: Vec<(String, String)>,
    pub(crate) body: Vec<u8>,
    /// `None` = read the token from `<state>/xplain/mcp.json`; `Some(None)` = no header; `Some(Some(t))` = token.
    pub(crate) auth: Option<Option<String>>,
    pub(crate) default_host: bool,
    pub(crate) remote_port: u16,
    pub(crate) too_large: bool,
}

impl Http {
    fn base(body: Vec<u8>) -> Http {
        Http {
            method: "POST".into(),
            path: "/mcp".into(),
            headers: vec![
                ("content-type".into(), "application/json".into()),
                ("accept".into(), "application/json, text/event-stream".into()),
            ],
            body,
            auth: None,
            default_host: true,
            remote_port: 40001,
            too_large: false,
        }
    }

    /// JSON-RPC `tools/call` of `tool` with `args`.
    pub fn tool(tool: &str, args: Value) -> Http {
        Http::rpc("tools/call", json!({"name": tool, "arguments": args}))
    }

    /// JSON-RPC request `method` with `params` (`initialize`, `tools/list`, ...), request id 1.
    pub fn rpc(method: &str, params: Value) -> Http {
        Http::json(&json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}))
    }

    /// Any JSON value as the body (notifications, batches, malformed envelopes).
    pub fn json(body: &Value) -> Http {
        Http::base(body.to_string().into_bytes())
    }

    /// Raw body bytes, sent as is.
    pub fn raw(body: impl Into<Vec<u8>>) -> Http {
        Http::base(body.into())
    }

    /// A body over the size cap: core sees `body_too_large` (F-MCPSRV-02.5) with the first 1 MiB as body.
    pub fn oversized() -> Http {
        let mut h = Http::base(vec![b'x'; 1_048_576]);
        h.too_large = true;
        h
    }

    pub fn method(mut self, m: &str) -> Self {
        self.method = m.into();
        self
    }
    pub fn path(mut self, p: &str) -> Self {
        self.path = p.into();
        self
    }
    /// Add a header (name lower-cased). Replaces a default header of the same name (`host` included).
    pub fn header(mut self, name: &str, value: &str) -> Self {
        let n = name.to_ascii_lowercase();
        if n == "host" {
            self.default_host = false;
        }
        self.headers.retain(|(k, _)| *k != n);
        self.headers.push((n, value.into()));
        self
    }
    /// Remove a default header (`host`, `content-type`, `accept`).
    pub fn without_header(mut self, name: &str) -> Self {
        let n = name.to_ascii_lowercase();
        if n == "host" {
            self.default_host = false;
        }
        self.headers.retain(|(k, _)| *k != n);
        self
    }
    /// `mcp-session-id` header.
    pub fn session(self, id: &str) -> Self {
        self.header("mcp-session-id", id)
    }
    /// Send no `authorization` header.
    pub fn no_auth(mut self) -> Self {
        self.auth = Some(None);
        self
    }
    /// Send `authorization: Bearer <token>` with this token instead of the real one.
    pub fn token(mut self, t: &str) -> Self {
        self.auth = Some(Some(t.into()));
        self
    }
    /// TCP remote port (identifies anonymous clients as `anon:<port>`); default 40001.
    pub fn remote_port(mut self, p: u16) -> Self {
        self.remote_port = p;
        self
    }
}

/// The response for one request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpReply {
    pub status: u16,
    /// Lower-case names, in order.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl From<HttpResponse> for HttpReply {
    fn from(r: HttpResponse) -> Self {
        HttpReply { status: r.status, headers: r.headers, body: r.body }
    }
}

impl HttpReply {
    /// Header value by case-insensitive name.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
    /// Body as text.
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
    /// Body parsed as JSON (panics with the body when it is not JSON).
    pub fn json(&self) -> Value {
        match serde_json::from_slice(&self.body) {
            Ok(v) => v,
            Err(e) => panic!("http reply body is not JSON ({e}): {:?}", self.text()),
        }
    }
    /// `result.content[0].text` of a tool reply: parsed as JSON when it is JSON, else a string value.
    /// `Null` when the reply has no such text (errors, non-tool methods).
    pub fn tool_result(&self) -> Value {
        let body: Value = serde_json::from_slice(&self.body).unwrap_or(Value::Null);
        match body.pointer("/result/content/0/text").and_then(Value::as_str) {
            Some(t) => serde_json::from_str(t).unwrap_or_else(|_| Value::String(t.to_string())),
            None => Value::Null,
        }
    }
    /// JSON-RPC `result` object of the body.
    pub fn rpc_result(&self) -> Value {
        self.json().get("result").cloned().unwrap_or(Value::Null)
    }
}

/// Handle of a request that was sent and may still be parked (a long poll).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Pending(pub(crate) ConnId);
