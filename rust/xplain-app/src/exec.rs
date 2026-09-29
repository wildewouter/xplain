//! Effect executor: turns each core `Effect` into real work and result `Event`s.
//!
//! Spec: F-MODE-01/02/04 (git, untracked listing, `--cwd` check, error texts), F-BROWSE-01 (file read),
//! F-CONFIG-05 (read-merge-write), F-EXPORT-01 (write), F-INTEG-* (process spawn, timeouts, not-found),
//! Clipboard (OSC 52), Messages (IoReason mapping), UNSPEC-37 (timeouts: git 60 s, integrations 20 s).
//! Owner: component C (mcp/exec).
//! Must not: decide behavior. It maps effect -> IO -> event and reports pending-work counts so the barrier
//! logic in `runtime` can tell when everything settled. Never surfaces OS error text.

use tokio::sync::mpsc::UnboundedSender;
use xplain_core::effect::Effect;
use xplain_core::event::Event;

use crate::http::HttpCounters;

/// Runs effects. `dispatch` must return immediately; results come back through `tx` as events.
/// Implementations register every pending (non-background) piece of work with the [`PendingWork`] counter
/// before returning and decrement it only after the result event was sent.
pub trait Executor {
    fn dispatch(&mut self, effect: Effect);
}

/// Shared counter of outstanding non-background async work (git loads, file IO, MCP start/stop,
/// integration commands, HTTP request body reads). Idle barrier replies wait for zero.
#[derive(Debug, Clone, Default)]
pub struct PendingWork {
    inner: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl PendingWork {
    pub fn begin(&self) {
        self.inner.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
    pub fn end(&self) {
        self.inner.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
    pub fn count(&self) -> usize {
        self.inner.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// The real executor (tokio tasks). Thin dispatcher: each effect calls one function of `git`, `fsio`,
/// `config_io`, `proc`, `token`/`http` in a spawned task, wraps the result into the matching `Event` with the
/// effect's `ReqId`, sends it, then `pending.end()`.
///
/// Effects handled here: LoadDiff, ListFiles, ReadFile, SaveConfig, WriteExport, RunCommand, McpStart,
/// McpStop, HttpReply. NOT handled here (the runtime loop owns them because they touch stdout, timers or
/// the loop itself): Clipboard, SetTimer, CancelTimer, Exit; `dispatch` ignores them.
/// Ordering: `HttpReply` before `McpStop` must be fully written before the server closes; `McpStop`
/// and `McpStart` are serialized against each other. `HttpReply` counts as pending work until written.
pub struct RealExecutor {
    _tx: UnboundedSender<Event>,
    _pending: PendingWork,
    _counters: HttpCounters,
    _state_dir_note: (),
}

impl RealExecutor {
    pub fn new(tx: UnboundedSender<Event>, pending: PendingWork, counters: HttpCounters) -> Self {
        RealExecutor { _tx: tx, _pending: pending, _counters: counters, _state_dir_note: () }
    }
}

impl Executor for RealExecutor {
    fn dispatch(&mut self, _effect: Effect) {
        todo!("execute effects")
    }
}
