//! MCP protocol logic as pure state machine: request checks, JSON-RPC dispatch, tools, hub queue,
//! long-poll bookkeeping, client list, counters, token helpers. No sockets.
//!
//! Spec: F-MCPSRV-01..11, F-ASK-01/09, F-MCPUI-03 (stop semantics), F-RELOAD-02. Owner: core lead
//! (mcp component); this file holds only the boundary types and entry points, the lead may split
//! the implementation into `src/mcp/*.rs` submodules (convert this file to `mcp/mod.rs`).
//! Must not: open sockets, read time, generate randomness (runtime supplies both in [`HttpRequest`]).

use crate::comments::PaneSide;
use crate::effect::Effect;
use crate::event::ReqId;

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
    // internal (pollers, sticky map, sessions) added by the mcp lead
}

impl McpState {
    /// Handle one HTTP request: F-MCPSRV-02 checks, JSON-RPC (F-MCPSRV-03..05), tools (06..10).
    /// The active bearer token is `self.endpoint`'s; `now_ms` is `State::clock.unix_ms`.
    pub fn handle_http(&mut self, _req: HttpRequest, _now_ms: i64) -> McpOutput {
        todo!("F-MCPSRV-02..11")
    }

    /// A parked long poll timed out (`TimerId::PollTimeout`).
    pub fn poll_timeout(&mut self, _conn: ConnId) -> McpOutput {
        todo!("F-MCPSRV-06")
    }

    /// The connection of a parked poll went away.
    pub fn conn_closed(&mut self, _conn: ConnId) -> McpOutput {
        todo!("F-MCPSRV-06")
    }

    /// Queue a question for delivery (F-ASK-01); may complete a waiting poll immediately.
    pub fn enqueue(&mut self, _q: OutQuestion) -> McpOutput {
        todo!("F-ASK-01, F-MCPSRV-06")
    }

    /// Stop: answer every waiting poll `closed` (F-MCPUI-03), clear clients and queue.
    /// Returned effects contain the replies; the reducer appends `Effect::McpStop` after them.
    pub fn stop(&mut self) -> McpOutput {
        todo!("F-MCPUI-03, F-QUIT-01")
    }
}

/// Validate `XPLAIN_MCP_PORT` (Test seams): `None`/empty -> 47615; else decimal 0-65535.
/// `Err` = the full message `invalid XPLAIN_MCP_PORT "<value>" (0-65535)`.
pub fn parse_port(_raw: Option<&str>) -> Result<u16, String> {
    todo!("Test seams: XPLAIN_MCP_PORT")
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

pub fn plan_token(_existing_file: Option<&str>, _random: [u8; 32]) -> TokenPlan {
    todo!("F-MCPSRV-01")
}
