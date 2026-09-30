//! Effect executor: turns each core `Effect` into real work and result `Event`s.
//!
//! Spec: F-MODE-01/02/04 (git, untracked listing, `--cwd` check, error texts), F-BROWSE-01 (file read),
//! F-CONFIG-05 (read-merge-write), F-EXPORT-01 (write), F-INTEG-* (process spawn, timeouts, not-found),
//! Clipboard (OSC 52), Messages (IoReason mapping), UNSPEC-37 (timeouts: git 60 s, integrations 20 s).
//! Owner: component C (mcp/exec).
//! Must not: decide behavior. It maps effect -> IO -> event and reports pending-work counts so `runtime` can
//! wait for outstanding work on exit. Never surfaces OS error text.

use std::sync::{Arc, Mutex, MutexGuard};

use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use xplain_core::effect::Effect;
use xplain_core::event::{Event, ReqId};

use crate::http::McpServer;
use crate::{config_io, fsio, git, proc};

/// Runs effects. `dispatch` must return immediately; results come back through `tx` as events.
/// Implementations register every pending (non-background) piece of work with the [`PendingWork`] counter
/// before returning and decrement it only after the result event was sent.
pub trait Executor {
    fn dispatch(&mut self, effect: Effect);
}

/// Shared counter of outstanding non-background async work (git loads, file IO, MCP start/stop,
/// integration commands, HTTP request body reads). Exit waits for zero. Changes are observable
/// through [`PendingWork::subscribe`], so waiters never poll.
#[derive(Debug, Clone)]
pub struct PendingWork {
    inner: Arc<tokio::sync::watch::Sender<usize>>,
}

impl Default for PendingWork {
    fn default() -> Self {
        PendingWork { inner: Arc::new(tokio::sync::watch::channel(0).0) }
    }
}

impl PendingWork {
    pub fn begin(&self) {
        self.inner.send_modify(|n| *n += 1);
    }
    pub fn end(&self) {
        self.inner.send_modify(|n| *n = n.saturating_sub(1));
    }
    /// RAII form of `begin`/`end`: pending until dropped (also on panic or task abort).
    pub fn guard(&self) -> WorkGuard {
        self.begin();
        WorkGuard(self.clone())
    }
    pub fn count(&self) -> usize {
        *self.inner.borrow()
    }
    /// Receiver that wakes on every count change (no lost wakeups: `changed` fires for changes since the
    /// last `borrow_and_update`).
    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<usize> {
        self.inner.subscribe()
    }
    /// Resolves once the count is zero.
    pub async fn wait_idle(&self) {
        let _ = self.subscribe().wait_for(|n| *n == 0).await;
    }
}

/// Holds one unit of [`PendingWork`] until dropped.
#[derive(Debug)]
pub struct WorkGuard(PendingWork);

impl Drop for WorkGuard {
    fn drop(&mut self) {
        self.0.end();
    }
}

enum ServerCmd {
    Start { req: ReqId, port: u16, state_dir: String, work: WorkGuard },
    Stop { req: ReqId, work: WorkGuard },
}

type Slot = Arc<Mutex<Option<McpServer>>>;

fn lock(slot: &Slot) -> MutexGuard<'_, Option<McpServer>> {
    slot.lock().unwrap_or_else(|e| e.into_inner())
}

/// The real executor (tokio tasks). Thin dispatcher: each effect calls one function of `git`, `fsio`,
/// `config_io`, `proc`, `token`/`http` in a spawned task, wraps the result into the matching `Event` with the
/// effect's `ReqId`, sends it, then `pending.end()`.
///
/// Effects handled here: LoadDiff, ListFiles, ReadFile, SaveConfig, WriteExport, RunCommand, Highlight
/// (blocking pool, pure CPU), McpStart, McpStop, HttpReply. NOT handled here (the runtime loop owns them because they touch stdout, timers or
/// the loop itself): Clipboard, SetTimer, CancelTimer, Exit; `dispatch` ignores them.
/// Ordering: `HttpReply` before `McpStop` must be fully written before the server closes; `McpStop`
/// and `McpStart` are serialized against each other. `HttpReply` counts as pending work until written.
pub struct RealExecutor {
    tx: UnboundedSender<Event>,
    pending: PendingWork,
    server: Slot,
    /// Serial queue for McpStart/McpStop, created on first use.
    server_cmds: Option<UnboundedSender<ServerCmd>>,
}

impl RealExecutor {
    pub fn new(tx: UnboundedSender<Event>, pending: PendingWork) -> Self {
        RealExecutor { tx, pending, server: Arc::new(Mutex::new(None)), server_cmds: None }
    }

    /// Run `fut` in a task; its output event is sent, then the pending unit is released.
    fn spawn_event<F>(&self, fut: F)
    where
        F: std::future::Future<Output = Event> + Send + 'static,
    {
        let work = self.pending.guard();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let ev = fut.await;
            let _ = tx.send(ev);
            drop(work);
        });
    }

    fn server_queue(&mut self) -> UnboundedSender<ServerCmd> {
        if let Some(q) = &self.server_cmds {
            return q.clone();
        }
        let (q, mut rx) = unbounded_channel::<ServerCmd>();
        let tx = self.tx.clone();
        let pending = self.pending.clone();
        let slot = self.server.clone();
        tokio::spawn(async move {
            while let Some(cmd) = rx.recv().await {
                match cmd {
                    ServerCmd::Start { req, port, state_dir, work } => {
                        let old = lock(&slot).take();
                        if let Some(old) = old {
                            old.stop().await;
                        }
                        let started = McpServer::start(port, &state_dir, tx.clone(), pending.clone()).await;
                        let result = started.map(|(server, endpoint)| {
                            *lock(&slot) = Some(server);
                            endpoint
                        });
                        let _ = tx.send(Event::McpStarted { req, result });
                        drop(work);
                    }
                    ServerCmd::Stop { req, work } => {
                        let server = lock(&slot).take();
                        if let Some(server) = server {
                            server.stop().await;
                        }
                        let _ = tx.send(Event::McpStopped { req });
                        drop(work);
                    }
                }
            }
        });
        self.server_cmds = Some(q.clone());
        q
    }
}

impl Executor for RealExecutor {
    fn dispatch(&mut self, effect: Effect) {
        match effect {
            Effect::LoadDiff { req, spec } => {
                self.spawn_event(
                    async move { Event::DiffLoaded { req, result: git::load_diff(&spec).await } },
                );
            }
            Effect::ListFiles { req, cwd } => {
                self.spawn_event(async move {
                    Event::FilesListed { req, files: git::list_files(cwd.as_deref()).await }
                });
            }
            Effect::ReadFile { req, path } => {
                self.spawn_event(
                    async move { Event::FileRead { req, result: fsio::read_file(&path).await } },
                );
            }
            Effect::SaveConfig { req, path, change } => {
                self.spawn_event(async move {
                    Event::ConfigSaved { req, result: config_io::save_config(&path, &change).await }
                });
            }
            Effect::WriteExport { req, path, contents } => {
                self.spawn_event(async move {
                    Event::ExportWritten { req, result: fsio::write_export(&path, &contents).await }
                });
            }
            Effect::RunCommand { req, cmd } => {
                self.spawn_event(
                    async move { Event::CommandDone { req, result: proc::run_command(&cmd).await } },
                );
            }
            Effect::Highlight { key, lang, start, lines, carry } => {
                // CPU work on the blocking pool, off the loop and the render path.
                self.spawn_event(async move {
                    let n = lines.len();
                    let job = tokio::task::spawn_blocking(move || {
                        xplain_core::highlight_lines(&lang, &lines, carry.as_ref())
                    })
                    .await;
                    let (runs, end) = job.unwrap_or_else(|_| (vec![Vec::new(); n], None));
                    Event::Highlighted { key, start, runs, end }
                });
            }
            Effect::McpStart { req, port, state_dir } => {
                let work = self.pending.guard();
                let _ = self.server_queue().send(ServerCmd::Start { req, port, state_dir, work });
            }
            Effect::McpStop { req } => {
                let work = self.pending.guard();
                let _ = self.server_queue().send(ServerCmd::Stop { req, work });
            }
            Effect::HttpReply { conn, response } => {
                // Delivered synchronously so it precedes any later McpStop; pending until written.
                if let Some(server) = lock(&self.server).as_ref() {
                    server.reply_tracked(conn, response, &self.pending);
                }
            }
            Effect::Clipboard(_) | Effect::SetTimer { .. } | Effect::CancelTimer(_) | Effect::Exit { .. } => {
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;
    use xplain_core::integration::{CommandError, CommandSpec};
    use xplain_core::mcp::{HttpResponse, McpEndpoint};

    async fn recv(rx: &mut tokio::sync::mpsc::UnboundedReceiver<Event>) -> Event {
        match tokio::time::timeout(Duration::from_secs(10), rx.recv()).await {
            Ok(Some(e)) => e,
            other => unreachable!("no event: {other:?}"),
        }
    }

    fn make() -> (RealExecutor, tokio::sync::mpsc::UnboundedReceiver<Event>, PendingWork) {
        let (tx, rx) = unbounded_channel();
        let pending = PendingWork::default();
        (RealExecutor::new(tx, pending.clone()), rx, pending)
    }

    #[test]
    fn pending_guard_counts() {
        let p = PendingWork::default();
        let g = p.guard();
        assert_eq!(p.count(), 1);
        drop(g);
        assert_eq!(p.count(), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn pending_wait_idle_wakes_on_end_without_polling() {
        let p = PendingWork::default();
        p.wait_idle().await;
        let g = p.guard();
        let mut rx = p.subscribe();
        rx.borrow_and_update();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            drop(g);
        });
        rx.changed().await.unwrap();
        assert_eq!(p.count(), 0);
        p.wait_idle().await;
    }

    #[tokio::test]
    async fn run_command_result_event_same_req() {
        let (mut ex, mut rx, pending) = make();
        let cmd = CommandSpec {
            program: "xplain-no-such-binary-zzz".into(),
            args: vec![],
            cwd: None,
            env: vec![],
            timeout_ms: 1000,
        };
        ex.dispatch(Effect::RunCommand { req: ReqId(7), cmd });
        assert_eq!(pending.count(), 1);
        match recv(&mut rx).await {
            Event::CommandDone { req, result } => {
                assert_eq!(req, ReqId(7));
                assert_eq!(result, Err(CommandError::NotFound));
            }
            other => unreachable!("{other:?}"),
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(pending.count(), 0);
    }

    #[tokio::test]
    async fn highlight_runs_off_loop_and_counts_as_pending() {
        use xplain_core::HlKey;
        use xplain_core::PaneSide;
        let (mut ex, mut rx, pending) = make();
        let key = HlKey { path: "a.rs".into(), side: PaneSide::New, hash: 7 };
        ex.dispatch(Effect::Highlight {
            key: key.clone(),
            lang: "rust".into(),
            start: 5,
            lines: vec!["fn a() {}".into(), "let x = 1;".into()],
            carry: None,
        });
        assert_eq!(pending.count(), 1);
        match recv(&mut rx).await {
            Event::Highlighted { key: k, start, runs, end } => {
                assert_eq!((k, start, runs.len()), (key, 5, 2));
                assert!(runs.iter().all(|r| !r.is_empty()));
                assert!(end.is_some());
            }
            other => unreachable!("{other:?}"),
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(pending.count(), 0);
    }

    #[tokio::test]
    async fn runtime_owned_effects_ignored() {
        let (mut ex, mut rx, pending) = make();
        ex.dispatch(Effect::Clipboard("x".into()));
        ex.dispatch(Effect::Exit { code: 0 });
        assert_eq!(pending.count(), 0);
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn mcp_start_reply_stop_order() {
        let dir = std::env::temp_dir().join(format!("xplain-exec-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("mcp.json"), "{\"token\":\"0123456789abcdef0123\"}");
        let (mut ex, mut rx, pending) = make();
        ex.dispatch(Effect::McpStart {
            req: ReqId(1),
            port: 0,
            state_dir: dir.to_string_lossy().into_owned(),
        });
        assert_eq!(pending.count(), 1);
        let ep: McpEndpoint = match recv(&mut rx).await {
            Event::McpStarted { req, result } => {
                assert_eq!(req, ReqId(1));
                result.unwrap_or_else(|e| unreachable!("{e}"))
            }
            other => unreachable!("{other:?}"),
        };
        let mut s = TcpStream::connect(("127.0.0.1", ep.port)).await.unwrap_or_else(|e| unreachable!("{e}"));
        let _ = s.write_all(b"POST /mcp HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\n\r\n").await;
        let conn = match recv(&mut rx).await {
            Event::McpHttp(r) => r.conn,
            other => unreachable!("{other:?}"),
        };
        let response = HttpResponse { status: 200, headers: vec![], body: b"bye".to_vec() };
        ex.dispatch(Effect::HttpReply { conn, response });
        ex.dispatch(Effect::McpStop { req: ReqId(2) });
        match recv(&mut rx).await {
            Event::McpStopped { req } => assert_eq!(req, ReqId(2)),
            other => unreachable!("{other:?}"),
        }
        let mut out = Vec::new();
        let _ = tokio::time::timeout(Duration::from_secs(5), s.read_to_end(&mut out)).await;
        let text = String::from_utf8_lossy(&out).into_owned();
        assert!(text.starts_with("HTTP/1.1 200 OK"), "{text}");
        assert!(text.ends_with("bye"));
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(pending.count(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
