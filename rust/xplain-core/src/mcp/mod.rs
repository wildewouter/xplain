//! MCP protocol logic as pure state machine: request checks, JSON-RPC dispatch, tools, hub queue,
//! long-poll bookkeeping, client list, counters, token helpers. No sockets.
//!
//! Spec: F-MCPSRV-01..11, F-ASK-01/09, F-MCPUI-03 (stop semantics), F-RELOAD-02. Owner: component `agent` (E).
//! This file holds the boundary types and entry points; implementation is split into the submodules below
//! (`http` request checks, `rpc` JSON-RPC envelope + methods, `tools` tool table and calls, `hub` queue/pollers/
//! clients/counters, `token` token + port helpers).
//! Must not: open sockets, read time, generate randomness (runtime supplies both in [`HttpRequest`]).

pub mod http;
pub mod hub;
pub mod rpc;
pub mod token;
pub mod tools;

use crate::comments::PaneSide;
use crate::effect::Effect;
use crate::event::{ReqId, TimerId};

/// Identifies one HTTP connection/request the runtime is holding. Unique per request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConnId(pub u64);

/// A running server's coordinates (F-MCPSRV-01). `port` is the effective bound port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpEndpoint {
    pub url: String,
    pub token: String,
    pub port: u16,
}

/// One HTTP request as read by the runtime (already fully read, up to the size cap).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub conn: ConnId,
    pub method: String,
    /// Request target including query.
    pub path: String,
    /// Header names lower-cased, values as sent, in order.
    pub headers: Vec<(String, String)>,
    /// Up to 1048576 bytes.
    pub body: Vec<u8>,
    /// Body exceeded 1048576 bytes: runtime stopped reading; core answers 413 (F-MCPSRV-02.5).
    pub body_too_large: bool,
    /// TCP remote port, for `anon:<port>` clients (UNSPEC-12).
    pub remote_port: u16,
    /// 16 random bytes for session UUIDs (keeps core deterministic).
    pub entropy: [u8; 16],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    /// Extra headers besides `content-type: application/json`, which core adds when `body` is non-empty
    /// (`202` has none). Names lower-case (`www-authenticate`, `allow`, `mcp-session-id`, `connection`).
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// What the UI must learn from an MCP call (applied by the reducer, never by the runtime).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HubEvent {
    Answer {
        thread_id: String,
        turn: u32,
        text: String,
    },
    Annotate {
        file: String,
        line: u32,
        text: String,
        side: PaneSide,
        number: Option<u32>,
    },
    FilesChanged {
        paths: Vec<String>,
    },
    /// A question was delivered to a client (UI status -> `streaming`, agent name = client name).
    Delivered {
        thread_id: String,
        turn: u32,
        client_name: String,
    },
    /// Client list / counters changed (redraw MCP modal).
    ClientsChanged,
}

/// Outcome of feeding one request or timer to the hub.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct McpOutput {
    /// `HttpReply` / `SetTimer` / `CancelTimer` effects to append to the update's effects.
    pub effects: Vec<Effect>,
    pub events: Vec<HubEvent>,
}

/// A question queued for the agent (payload of F-MCPSRV-06). Built by the ask flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutQuestion {
    pub thread_id: String,
    pub turn: u32,
    /// Fully composed `<q>` text (context included per F-MCPSRV-06).
    pub question: String,
    pub follow_up: bool,
    /// `(turn, question, answer)` of earlier turns, oldest first (core trims to last 5 / 4000 chars).
    pub previous: Vec<(u32, String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    /// A `next_question` long poll is waiting.
    pub polling: bool,
}

/// Hub + server state (fields outlined; the mcp lead owns the internals).
#[derive(Debug, Clone, Default)]
pub struct McpState {
    /// Server listening.
    pub running: bool,
    /// Start requested, result pending (`… starting`).
    pub starting: Option<ReqId>,
    pub endpoint: Option<McpEndpoint>,
    /// Last start failure, shown in the modal until next start attempt (F-MCPUI-01).
    pub start_error: Option<String>,
    pub clients: Vec<ClientInfo>,
    pub queue: Vec<OutQuestion>,
    pub delivered: u32,
    /// Internal hub bookkeeping (pollers, sticky map, sessions); owned by `hub.rs`.
    pub inner: hub::HubInner,
}

impl McpState {
    /// Handle one HTTP request: F-MCPSRV-02 checks, JSON-RPC (F-MCPSRV-03..05), tools (06..10).
    /// The active bearer token is `self.endpoint`'s; `now_ms` is `State::clock.unix_ms`.
    pub fn handle_http(&mut self, req: HttpRequest, now_ms: i64) -> McpOutput {
        let (token, port) = self.endpoint.as_ref().map_or((String::new(), 0), |e| (e.token.clone(), e.port));
        if let Err(response) = http::precheck(&req, &token, port) {
            return McpOutput {
                effects: vec![Effect::HttpReply { conn: req.conn, response }],
                events: Vec::new(),
            };
        }
        rpc::dispatch(self, req, now_ms)
    }

    /// A parked long poll timed out (`TimerId::PollTimeout`).
    pub fn poll_timeout(&mut self, conn: ConnId) -> McpOutput {
        let mut out = McpOutput::default();
        if let Some(pos) = self.inner.pollers.iter().position(|p| p.conn == conn) {
            let poller = self.inner.pollers.remove(pos);
            self.inner.events.push(HubEvent::ClientsChanged);
            self.inner.backlog.push(hub::Resolved {
                poller,
                result: rpc::timeout_result(),
                cancel_timer: false,
            });
        }
        rpc::settle(self, &mut out);
        out
    }

    /// The connection of a parked poll went away.
    pub fn conn_closed(&mut self, conn: ConnId) -> McpOutput {
        let mut out = McpOutput::default();
        if let Some(pos) = self.inner.pollers.iter().position(|p| p.conn == conn) {
            self.inner.pollers.remove(pos);
            self.inner.events.push(HubEvent::ClientsChanged);
            out.effects.push(Effect::CancelTimer(TimerId::PollTimeout(conn)));
        }
        rpc::settle(self, &mut out);
        out
    }

    /// Queue a question for delivery (F-ASK-01); may complete a waiting poll immediately.
    pub fn enqueue(&mut self, q: OutQuestion) -> McpOutput {
        let preview = q.question.clone();
        self.enqueue_with_preview(q, &preview)
    }

    /// Like [`McpState::enqueue`], with the text `get_questions` shows as `preview` (the bare message).
    pub fn enqueue_with_preview(&mut self, mut q: OutQuestion, preview: &str) -> McpOutput {
        hub::HubInner::trim_history(&mut q);
        self.inner.previews.push((q.thread_id.clone(), q.turn, tools::sanitize(preview)));
        self.queue.push(q);
        self.dispatch();
        let mut out = McpOutput::default();
        rpc::settle(self, &mut out);
        out
    }

    /// Stop: answer every waiting poll `closed` (F-MCPUI-03), clear clients and queue.
    /// Returned effects contain the replies; the reducer appends `Effect::McpStop` after them.
    pub fn stop(&mut self) -> McpOutput {
        let mut out = McpOutput::default();
        self.inner.closing = true;
        for poller in std::mem::take(&mut self.inner.pollers) {
            self.inner.backlog.push(hub::Resolved {
                poller,
                result: hub::PollResult::Closed,
                cancel_timer: true,
            });
        }
        rpc::settle(self, &mut out);
        self.inner.closing = false;
        self.clients.clear();
        self.queue.clear();
        self.inner = hub::HubInner::default();
        out.events.push(HubEvent::ClientsChanged);
        out
    }
}

/// Validate `XPLAIN_MCP_PORT` (Test seams): `None`/empty -> 47615; else decimal 0-65535.
/// `Err` = the full message `invalid XPLAIN_MCP_PORT "<value>" (0-65535)`.
pub fn parse_port(raw: Option<&str>) -> Result<u16, String> {
    token::parse_port(raw)
}

/// Final port-busy message (F-MCPUI-03).
pub fn port_busy_message(port: u16) -> String {
    format!("MCP port {port} is already in use on 127.0.0.1. Stop the other process or choose another port.")
}

/// Decide what to do with the token file content (F-MCPSRV-01): reuse a string `token` of length >= 16,
/// else a new token from `random` (32 bytes, base64url, 43 chars) and the file must be rewritten.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenPlan {
    Reuse(String),
    Write { token: String, file_contents: String },
}

pub fn plan_token(existing_file: Option<&str>, random: [u8; 32]) -> TokenPlan {
    token::plan_token(existing_file, random)
}
