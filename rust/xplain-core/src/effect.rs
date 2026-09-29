//! Everything core asks the outside world to do. The only output of `update` besides state.
//!
//! Spec: Test seams (pending work definition), F-MODE-01/02, F-CONFIG-05, F-EXPORT-01, F-MCPSRV-*,
//! F-INTEG-*. Owner: core lead (types frozen at skeleton).
//! Must not: perform anything. Effects run strictly in the order returned by one `update` call.
//! Ordering guarantee the runtime must keep: `HttpReply` effects preceding `McpStop` are written
//! completely before the server closes (F-MCPUI-03); `Exit` runs after all earlier effects finished.

use crate::config::ConfigChange;
use crate::diff::DiffSpec;
use crate::event::{ReqId, TimerId};
use crate::integration::CommandSpec;
use crate::mcp::{ConnId, HttpResponse};

/// Effect list built by handlers within one `update` call (execution order).
pub type Fx = Vec<Effect>;

#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Run git per `spec` (cwd check first), read untracked files if `spec.wants_untracked()`.
    /// Result: `Event::DiffLoaded`. Counts as pending work.
    LoadDiff {
        req: ReqId,
        spec: DiffSpec,
    },
    /// `git ls-files --cached --others --exclude-standard` in `cwd`, sorted, unique.
    /// Result: `Event::FilesListed`. Pending work.
    ListFiles {
        req: ReqId,
        cwd: Option<String>,
    },
    /// Read a file (absolute or cwd-joined path chosen by core). Result: `Event::FileRead`. Pending work.
    ReadFile {
        req: ReqId,
        path: String,
    },
    /// Read-merge-write the config file: runtime reads `path`, calls `config::apply_patch`, creates parent
    /// dirs, writes. Result: `Event::ConfigSaved`. Pending work.
    SaveConfig {
        req: ReqId,
        path: String,
        change: ConfigChange,
    },
    /// Write the export markdown. Result: `Event::ExportWritten`. Pending work.
    WriteExport {
        req: ReqId,
        path: String,
        contents: String,
    },
    /// OSC 52 clipboard write: runtime base64-encodes UTF-8 and writes `ESC ] 52 ; c ; <b64> BEL`.
    Clipboard(String),
    /// Run an integration CLI. Result: `Event::CommandDone`. Pending work.
    RunCommand {
        req: ReqId,
        cmd: CommandSpec,
    },
    /// Start the MCP server on `port` (`0` = any). Runtime resolves/writes the token file (F-MCPSRV-01)
    /// using `mcp::token` helpers, binds, and reports `Event::McpStarted`. Pending work.
    McpStart {
        req: ReqId,
        port: u16,
        state_dir: String,
    },
    /// Close listener and connections after all previously emitted `HttpReply` were written.
    /// Result: `Event::McpStopped`. Pending work.
    McpStop {
        req: ReqId,
    },
    /// Write the HTTP response for a request (immediately or after parking). Counts toward `done`.
    HttpReply {
        conn: ConnId,
        response: HttpResponse,
    },
    /// Arm a timer (re-arming the same id replaces it). Result: `Event::Timer(id)`.
    /// `background = true` timers (spinner, long-poll waits) never count as pending work.
    SetTimer {
        id: TimerId,
        after_ms: u64,
        background: bool,
    },
    CancelTimer(TimerId),
    /// Leave the alternate screen and exit with `code` once earlier effects are finished.
    Exit {
        code: i32,
    },
}
