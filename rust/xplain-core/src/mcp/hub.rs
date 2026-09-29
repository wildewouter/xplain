//! Hub bookkeeping: question queue, parked long polls, sticky thread->client map, sessions, client list,
//! counters.
//!
//! Spec: F-MCPSRV-06 (queue order, poll wake/close/timeout, sticky routing, `previous` trimming to last 5 /
//! 4000 chars), F-MCPSRV-09 (`get_questions`), F-MCPSRV-11 (clients, `polling`, counters `delivered`),
//! F-MCPUI-03 (stop answers polls `closed`). Oracle: `src/mcp/hub.ts`. Owner: component `agent` (E).
//! Must not: build HTTP responses beyond the effect helpers it is given by rpc.rs.

use super::{ConnId, OutQuestion};

/// One parked `next_question` long poll.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Poller {
    pub conn: ConnId,
    pub client_id: String,
    /// JSON-RPC id to answer with (raw JSON text).
    pub rpc_id: String,
}

/// Internal state of the hub (add fields as needed).
#[derive(Debug, Clone, Default)]
pub struct HubInner {
    pub pollers: Vec<Poller>,
    /// `thread_id -> client_id` stickiness.
    pub sticky: Vec<(String, String)>,
    /// Delivered questions kept for `get_questions`.
    pub delivered_log: Vec<OutQuestion>,
}

impl HubInner {
    /// Trim `previous` to the last 5 entries of at most 4000 chars each (F-MCPSRV-06).
    pub fn trim_history(_q: &mut OutQuestion) {
        todo!("F-MCPSRV-06")
    }
}
