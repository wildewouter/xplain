//! Everything that can happen to the app. The only input of `update`.
//!
//! Spec: Test seams (timers, requests), all groups indirectly. Owner: core lead (types frozen at skeleton;
//! add variants only via the core lead, since the runtime must handle/produce them).
//! Must not: carry runtime error types (use [`IoReason`] / final message strings), or reference terminal
//! or socket types.
//!
//! Rules for results of async work: each effect that yields a result carries a [`ReqId`]; the result
//! event echoes it. Core tracks what each request was for in `State::pending` and ignores stale ids.

use crate::diff::RawDiff;
use crate::errors::IoReason;
use crate::integration::CommandResult;
use crate::keys::KeyEvent;
use crate::mcp::{ConnId, HttpRequest, McpEndpoint};
use crate::screen::Size;

/// Correlates an effect with its result event. Allocated by `State::next_req`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ReqId(pub u64);

/// Timers are events so tests can use a fake clock (send `Event::Timer(id)` by hand).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimerId {
    /// 80 ms spinner animation (F-ASK-05). Cosmetic: never counts as pending work.
    Spinner,
    /// `next_question` wait timeout for the long poll held on this connection (F-MCPSRV-06).
    PollTimeout(ConnId),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// Sent once by the runtime after the first full frame ("Loading...") was written.
    /// Core reacts with MCP autostart (F-MCPUI-04).
    Started,
    Key(KeyEvent),
    /// Bracketed paste. Newline runs collapse per input (F-FIND-01, F-COMMENT-02).
    Paste(String),
    Resize(Size),
    Timer(TimerId),

    /// Result of `Effect::LoadDiff`. `Err` is the final display text of the error screen
    /// (git stderr verbatim, `cannot run git: <reason>`, `cannot open directory <dir>: <reason>`).
    DiffLoaded {
        req: ReqId,
        result: Result<RawDiff, String>,
    },
    /// Result of `Effect::ListFiles` (sorted, unique). Errors leave the list empty, so only the Ok
    /// path carries data; the runtime sends `Ok(vec![])` on failure.
    FilesListed {
        req: ReqId,
        files: Vec<String>,
    },
    /// Result of `Effect::ReadFile` (raw bytes; core does NUL detection and line splitting).
    FileRead {
        req: ReqId,
        result: Result<Vec<u8>, IoReason>,
    },
    ConfigSaved {
        req: ReqId,
        result: Result<(), crate::config::ConfigSaveError>,
    },
    ExportWritten {
        req: ReqId,
        result: Result<(), IoReason>,
    },
    /// Result of `Effect::RunCommand` (integration CLIs).
    CommandDone {
        req: ReqId,
        result: CommandResult,
    },
    /// Result of `Effect::McpStart`. `Err` is the final user-facing message (port busy text,
    /// `cannot listen on ...`, `cannot write <state dir>/mcp.json: <reason>`).
    McpStarted {
        req: ReqId,
        result: Result<McpEndpoint, String>,
    },
    McpStopped {
        req: ReqId,
    },
    /// A fully read MCP HTTP request (body read done). Runtime already counted it in `reqs`.
    McpHttp(HttpRequest),
    /// The connection of a parked request went away before its reply was written (F-MCPSRV-06).
    McpConnClosed(ConnId),
}
