//! MCP HTTP server: sockets only.
//!
//! Spec: F-MCPSRV-01 (bind 127.0.0.1:<port>, token file IO using core `plan_token`, dir 0700 / file 0600),
//! F-MCPSRV-02 (socket-level: body size cap, connection close), F-MCPSRV-06 (connection drop while parked),
//! Test seams (`reqs` counted on arrival, `done` when response written or connection gone and handler
//! finished), F-MCPUI-03 (drain replies before close), Messages (`cannot listen ...`, `cannot write ...`).
//! Owner: component C (mcp/exec).
//! Must not: interpret requests. It reads a request, sends `Event::McpHttp`, and writes whatever
//! `Effect::HttpReply` says for that `ConnId`. All checks, JSON-RPC and tool logic are in core.

use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use tokio::sync::mpsc::UnboundedSender;
use xplain_core::event::Event;
use xplain_core::mcp::{ConnId, HttpResponse, McpEndpoint};

use crate::exec::PendingWork;

/// Process-wide request counters reported in every barrier reply (`<reqs>`, `<done>`).
#[derive(Debug, Clone, Default)]
pub struct HttpCounters {
    pub received: Arc<AtomicU64>,
    pub done: Arc<AtomicU64>,
}

pub struct McpServer {
    _private: (),
}

impl McpServer {
    /// Resolve/create token file, bind, spawn accept loop. `Err` = final user-facing message.
    pub async fn start(
        _port: u16,
        _state_dir: &str, // token via `crate::token::ensure_token`

        _tx: UnboundedSender<Event>,
        _counters: HttpCounters,
        _pending: PendingWork,
    ) -> Result<(McpServer, McpEndpoint), String> {
        todo!("F-MCPSRV-01")
    }

    /// Write the reply for a parked or fresh request.
    pub fn reply(&self, _conn: ConnId, _response: HttpResponse) {
        todo!("write response")
    }

    /// Close listener and connections; resolves after all earlier `reply` calls were written.
    pub async fn stop(self) {
        todo!("F-MCPUI-03")
    }
}
