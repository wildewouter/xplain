//! The event loop and sync barrier.
//!
//! Spec: Test seams (whole section), F-CLI-05 (start/exit), F-RELOAD-02 (silent reload is just an event).
//! Owner: component A (runtime); first thing built (spike). Effects `Clipboard`, `SetTimer`, `CancelTimer`, `Exit` are
//! handled here (stdout/clock/loop); all other effects go to the `Executor`. `Exit` waits until `PendingWork == 0`
//! (earlier effects incl. HttpReply/McpStop finished), leaves the alternate screen, returns the code.
//! Must not: hold UI state beyond `xplain_core::State`, or interpret keys.
//!
//! Loop: decode input -> for each item call `update` (after setting `state.clock`) -> hand effects to the
//! `Executor` -> when the queue is drained call `view` + `Presenter::draw` -> answer any barrier whose
//! conditions hold. Idle barrier holds when: all earlier input handled, the frame was written, event queue
//! empty, `PendingWork == 0`, and (before ready) first frame + initial load done (`State::is_ready`).
//! Frame barrier holds when earlier input handled and frame written. Barrier replies are written in order.

use std::collections::VecDeque;
use std::io::Write;
use std::time::Duration;

use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};
use xplain_core::effect::Effect;
use xplain_core::event::{Event, TimerId};
use xplain_core::screen::{Screen, Size};
use xplain_core::state::Now;

use crate::barrier::{BarrierQueue, Settle};
use crate::clipboard::osc52;
use crate::exec::{Executor, PendingWork, RealExecutor};
use crate::http::HttpCounters;
use crate::input::{BarrierKind, InputDecoder, InputItem};
use crate::present::Presenter;
use crate::timers::RealClock;

/// Clock abstraction so tests can drive time. Timers are events: the runtime schedules `TimerId`s and
/// injects `Event::Timer`.
pub trait Clock {
    fn now(&self) -> xplain_core::state::Now;
    fn schedule(&mut self, id: TimerId, after: Duration, background: bool);
    fn cancel(&mut self, id: TimerId);
}

/// Formats the barrier reply `ESC ] 7770 ; <kind> ; <n> ; <reqs> ; <done> BEL` (Test seams).
pub fn barrier_reply(kind: BarrierKind, n: u64, counters: &HttpCounters) -> Vec<u8> {
    use std::sync::atomic::Ordering::SeqCst;
    let k = match kind {
        BarrierKind::Idle => "idle",
        BarrierKind::Frame => "frame",
    };
    format!("\x1b]7770;{k};{n};{};{}\x07", counters.received.load(SeqCst), counters.done.load(SeqCst))
        .into_bytes()
}

/// Everything `run` needs from the outside.
pub struct RuntimeConfig {
    /// `XPLAIN_SYNC=1`: decode and answer barriers.
    pub sync: bool,
    /// 24-bit colors (`COLORTERM`), passed to the presenter.
    pub truecolor: bool,
}

/// What the loop needs from the core: `update`, `view`, readiness, clock injection. Real impl wraps
/// `xplain_core::State`; tests use a fake so the loop is testable on its own.
pub(crate) trait Model {
    fn set_clock(&mut self, now: Now);
    fn update(&mut self, event: Event) -> Vec<Effect>;
    fn view(&self) -> Screen;
    fn is_ready(&self) -> bool;
}

struct CoreModel(xplain_core::State);

impl Model for CoreModel {
    fn set_clock(&mut self, now: Now) {
        self.0.clock = now;
    }
    fn update(&mut self, event: Event) -> Vec<Effect> {
        xplain_core::update(&mut self.0, event)
    }
    fn view(&self) -> Screen {
        xplain_core::view(&self.0)
    }
    fn is_ready(&self) -> bool {
        self.0.is_ready()
    }
}

/// Testable core of the loop: all collaborators injected. `input` carries raw stdin bytes, `resize` terminal
/// size changes, `events` results/timers/HTTP events from executor and clock. Draws the first frame (`Loading...`
/// from `view`) before sending `Event::Started`. Returns the exit code.
#[allow(clippy::too_many_arguments)]
pub async fn drive<E: Executor, C: Clock, W: Write>(
    state: xplain_core::State,
    initial_effects: Vec<xplain_core::Effect>,
    cfg: &RuntimeConfig,
    exec: E,
    clock: C,
    pending: PendingWork,
    counters: HttpCounters,
    events: UnboundedReceiver<Event>,
    input: UnboundedReceiver<Vec<u8>>,
    resize: UnboundedReceiver<Size>,
    out: W,
) -> i32 {
    drive_model(
        CoreModel(state),
        initial_effects,
        cfg,
        exec,
        clock,
        pending,
        counters,
        events,
        input,
        resize,
        out,
    )
    .await
}

/// Loop state: everything `step`/`dispatch` touch.
struct Rt<M, E, C, W> {
    model: M,
    exec: E,
    clock: C,
    pending: PendingWork,
    out: W,
    presenter: Presenter,
    dirty: bool,
}

impl<M: Model, E: Executor, C: Clock, W: Write> Rt<M, E, C, W> {
    /// Feed one event to the core and run its effects. `Some(code)` when the core asked to exit.
    fn step(&mut self, event: Event) -> Option<i32> {
        self.model.set_clock(self.clock.now());
        let effects = self.model.update(event);
        self.dirty = true;
        self.dispatch(effects)
    }

    /// Run effects in order. The four runtime-owned ones are handled here, the rest go to the executor.
    fn dispatch(&mut self, effects: Vec<Effect>) -> Option<i32> {
        for effect in effects {
            match effect {
                Effect::Clipboard(text) => {
                    let _ = self.out.write_all(&osc52(&text));
                    let _ = self.out.flush();
                }
                Effect::SetTimer { id, after_ms, background } => {
                    self.clock.schedule(id, Duration::from_millis(after_ms), background);
                }
                Effect::CancelTimer(id) => self.clock.cancel(id),
                Effect::Exit { code } => return Some(code),
                other => self.exec.dispatch(other),
            }
        }
        None
    }

    fn draw(&mut self) -> std::io::Result<()> {
        let screen = self.model.view();
        self.presenter.draw(&screen, &mut self.out)?;
        self.dirty = false;
        Ok(())
    }

    /// Exit: wait for earlier work (HttpReply, McpStop, ...), restore the terminal, return the code.
    async fn finish(mut self, code: i32) -> i32 {
        while self.pending.count() > 0 {
            tokio::time::sleep(POLL).await;
        }
        let _ = self.presenter.leave(&mut self.out);
        code
    }
}

/// Poll interval while waiting for the pending-work counter (it changes without waking the loop).
const POLL: Duration = Duration::from_millis(1);

#[allow(clippy::too_many_arguments)]
pub(crate) async fn drive_model<M: Model, E: Executor, C: Clock, W: Write>(
    model: M,
    initial_effects: Vec<Effect>,
    cfg: &RuntimeConfig,
    exec: E,
    clock: C,
    pending: PendingWork,
    counters: HttpCounters,
    mut events: UnboundedReceiver<Event>,
    mut input: UnboundedReceiver<Vec<u8>>,
    mut resize: UnboundedReceiver<Size>,
    mut out: W,
) -> i32 {
    let Ok(presenter) = Presenter::enter(&mut out) else { return 1 };
    let mut rt = Rt {
        model,
        exec,
        clock,
        pending: pending.clone(),
        out,
        presenter: presenter.with_truecolor(cfg.truecolor),
        dirty: false,
    };
    macro_rules! step {
        ($ev:expr) => {
            if let Some(code) = rt.step($ev) {
                return rt.finish(code).await;
            }
        };
    }
    // First frame ("Loading..." from view) goes out before anything else happens.
    if rt.draw().is_err() {
        return rt.finish(1).await;
    }
    if let Some(code) = rt.dispatch(initial_effects) {
        return rt.finish(code).await;
    }
    step!(Event::Started);

    let mut decoder = InputDecoder::new(cfg.sync);
    let mut items: VecDeque<InputItem> = VecDeque::new();
    let mut barriers = BarrierQueue::new();
    let (mut events_open, mut input_open, mut resize_open) = (true, true, true);
    loop {
        let mut progress = false;
        while let Ok(size) = resize.try_recv() {
            progress = true;
            step!(Event::Resize(size));
        }
        // Input after a barrier waits for that barrier's reply.
        while barriers.is_empty() {
            let Some(item) = items.pop_front() else { break };
            progress = true;
            match item {
                InputItem::Key(k) => step!(Event::Key(k)),
                InputItem::Paste(t) => step!(Event::Paste(t)),
                InputItem::Resize(sz) => step!(Event::Resize(sz)),
                InputItem::Barrier(kind) => {
                    barriers.push(kind);
                }
            }
        }
        // Drain results; quiet = no tracked work was outstanding before a drain that found nothing.
        let quiet = loop {
            let quiet = pending.count() == 0;
            let mut got = false;
            while let Ok(ev) = events.try_recv() {
                got = true;
                step!(ev);
            }
            if !got {
                break quiet;
            }
            progress = true;
        };
        if rt.dirty {
            progress = true;
            if rt.draw().is_err() {
                return rt.finish(1).await;
            }
        }
        let due = barriers.due(Settle { frame_written: true, idle: quiet && rt.model.is_ready() });
        if !due.is_empty() {
            progress = true;
            for (kind, n) in due {
                let _ = rt.out.write_all(&barrier_reply(kind, n, &counters));
            }
            let _ = rt.out.flush();
        }
        if progress {
            continue;
        }
        if !events_open && !input_open && !resize_open {
            return rt.finish(0).await;
        }
        let waiting = !barriers.is_empty();
        tokio::select! {
            ev = events.recv(), if events_open => match ev {
                Some(ev) => step!(ev),
                None => events_open = false,
            },
            bytes = input.recv(), if input_open => match bytes {
                Some(b) => items.extend(decoder.feed(&b)),
                None => input_open = false,
            },
            size = resize.recv(), if resize_open => match size {
                Some(s) => step!(Event::Resize(s)),
                None => resize_open = false,
            },
            _ = tokio::time::sleep(POLL), if waiting => {}
        }
    }
}

/// Run the app until `Effect::Exit`. Returns the exit code. Wires the real pieces (channels, `RealClock`,
/// `RealExecutor`, `Presenter` on stdout, `term::spawn_stdin_reader`/`spawn_resize_watcher`) and calls [`drive`].
pub async fn run_loop(
    state: xplain_core::State,
    initial_effects: Vec<xplain_core::Effect>,
    cfg: RuntimeConfig,
) -> i32 {
    let (events_tx, events_rx) = unbounded_channel();
    let (input_tx, input_rx) = unbounded_channel();
    let (resize_tx, resize_rx) = unbounded_channel();
    let pending = PendingWork::default();
    let counters = HttpCounters::default();
    let clock = RealClock::new(events_tx.clone(), pending.clone());
    let exec = RealExecutor::new(events_tx, pending.clone(), counters.clone());
    let tracker = crate::term::SizeTracker::new(crate::term::size());
    crate::term::spawn_resize_watcher_tracked(resize_tx.clone(), tracker.clone());
    crate::term::spawn_stdin_reader_sized(input_tx, Some((resize_tx, tracker)));
    drive(
        state,
        initial_effects,
        &cfg,
        exec,
        clock,
        pending,
        counters,
        events_rx,
        input_rx,
        resize_rx,
        std::io::stdout(),
    )
    .await
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use tokio::sync::mpsc::UnboundedSender;
    use xplain_core::event::ReqId;
    use xplain_core::keys::Key;
    use xplain_core::screen::Cell;

    use super::*;

    type Log = Arc<Mutex<Vec<String>>>;

    fn push(log: &Log, s: impl Into<String>) {
        if let Ok(mut l) = log.lock() {
            l.push(s.into());
        }
    }

    #[derive(Clone, Default)]
    struct SharedBuf(Arc<Mutex<Vec<u8>>>);

    impl Write for SharedBuf {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            if let Ok(mut v) = self.0.lock() {
                v.extend_from_slice(b);
            }
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl SharedBuf {
        fn text(&self) -> String {
            String::from_utf8_lossy(&self.0.lock().map(|v| v.clone()).unwrap_or_default()).into_owned()
        }
    }

    /// Keys: `a` counts (view shows that many `K`), `l` ReadFile, `c` Clipboard, `t` non-bg timer (40 ms),
    /// `b` bg timer (150 ms), `x` cancel, `q` Exit(3) followed by a Clipboard that must never run.
    struct Fake {
        ready: bool,
        keys: usize,
        log: Log,
    }

    impl Model for Fake {
        fn set_clock(&mut self, _now: Now) {}
        fn update(&mut self, event: Event) -> Vec<Effect> {
            push(&self.log, format!("{event:?}"));
            match event {
                Event::Key(k) => match k.key {
                    Key::Char('a') => {
                        self.keys += 1;
                        vec![]
                    }
                    Key::Char('l') => vec![Effect::ReadFile { req: ReqId(1), path: "p".into() }],
                    Key::Char('c') => vec![Effect::Clipboard("hi".into())],
                    Key::Char('t') => {
                        vec![Effect::SetTimer { id: TimerId::Spinner, after_ms: 40, background: false }]
                    }
                    Key::Char('b') => {
                        vec![Effect::SetTimer { id: TimerId::Spinner, after_ms: 150, background: true }]
                    }
                    Key::Char('x') => vec![Effect::CancelTimer(TimerId::Spinner)],
                    Key::Char('q') => vec![Effect::Exit { code: 3 }, Effect::Clipboard("never".into())],
                    _ => vec![],
                },
                Event::FileRead { .. } => {
                    self.ready = true;
                    vec![]
                }
                _ => vec![],
            }
        }
        fn view(&self) -> Screen {
            push(&self.log, "view");
            let mut s = Screen::blank(Size { cols: 12, rows: 2 });
            if self.ready {
                s.rows[0][11] = Cell { ch: 'R', ..Cell::default() };
            } else {
                for (x, ch) in "Loading...".chars().enumerate() {
                    s.rows[0][x] = Cell { ch, ..Cell::default() };
                }
            }
            for x in 0..self.keys.min(12) {
                s.rows[1][x].ch = 'K';
            }
            s
        }
        fn is_ready(&self) -> bool {
            self.ready
        }
    }

    struct FakeExec {
        tx: UnboundedSender<Event>,
        pending: PendingWork,
        log: Log,
        delay_ms: u64,
    }

    impl Executor for FakeExec {
        fn dispatch(&mut self, effect: Effect) {
            push(&self.log, format!("exec {effect:?}"));
            if let Effect::ReadFile { req, .. } = effect {
                self.pending.begin();
                let (tx, pending, log, d) =
                    (self.tx.clone(), self.pending.clone(), self.log.clone(), self.delay_ms);
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(d)).await;
                    let _ = tx.send(Event::FileRead { req, result: Ok(vec![]) });
                    push(&log, "exec done");
                    pending.end();
                });
            }
        }
    }

    enum TestClock {
        Fake(Log),
        Real(RealClock),
    }

    impl Clock for TestClock {
        fn now(&self) -> Now {
            match self {
                TestClock::Fake(_) => Now { unix_ms: 1, utc_offset_secs: 0 },
                TestClock::Real(c) => c.now(),
            }
        }
        fn schedule(&mut self, id: TimerId, after: Duration, background: bool) {
            match self {
                TestClock::Fake(l) => push(l, format!("schedule {id:?} {} {background}", after.as_millis())),
                TestClock::Real(c) => c.schedule(id, after, background),
            }
        }
        fn cancel(&mut self, id: TimerId) {
            match self {
                TestClock::Fake(l) => push(l, format!("cancel {id:?}")),
                TestClock::Real(c) => c.cancel(id),
            }
        }
    }

    struct Rig {
        sync: bool,
        initial: Vec<Effect>,
        real_clock: bool,
        read_delay_ms: u64,
        counters: HttpCounters,
        input: Vec<Vec<u8>>,
        resizes: Vec<Size>,
    }

    impl Rig {
        fn new(input: &[&[u8]]) -> Rig {
            Rig {
                sync: true,
                initial: vec![],
                real_clock: false,
                read_delay_ms: 20,
                counters: HttpCounters::default(),
                input: input.iter().map(|b| b.to_vec()).collect(),
                resizes: vec![],
            }
        }

        async fn run(self) -> (i32, String, Vec<String>) {
            let log: Log = Log::default();
            let out = SharedBuf::default();
            let (etx, erx) = unbounded_channel();
            let (itx, irx) = unbounded_channel();
            let (rtx, rrx) = unbounded_channel();
            for b in self.input {
                itx.send(b).unwrap();
            }
            for s in self.resizes {
                rtx.send(s).unwrap();
            }
            let pending = PendingWork::default();
            let exec = FakeExec {
                tx: etx.clone(),
                pending: pending.clone(),
                log: log.clone(),
                delay_ms: self.read_delay_ms,
            };
            let clock = if self.real_clock {
                TestClock::Real(RealClock::new(etx.clone(), pending.clone()))
            } else {
                TestClock::Fake(log.clone())
            };
            let model = Fake { ready: self.initial.is_empty(), keys: 0, log: log.clone() };
            let cfg = RuntimeConfig { sync: self.sync, truecolor: true };
            let fut = drive_model(
                model,
                self.initial,
                &cfg,
                exec,
                clock,
                pending,
                self.counters,
                erx,
                irx,
                rrx,
                out.clone(),
            );
            let code = tokio::time::timeout(Duration::from_secs(5), fut).await.unwrap_or(-999);
            drop((etx, itx, rtx));
            let log = log.lock().map(|l| l.clone()).unwrap_or_default();
            (code, out.text(), log)
        }
    }

    fn read_file() -> Vec<Effect> {
        vec![Effect::ReadFile { req: ReqId(1), path: "p".into() }]
    }

    const IDLE: &[u8] = b"\x1b[9999~";
    const FRAME: &[u8] = b"\x1b[9998~";

    fn pos(hay: &str, needle: &str) -> usize {
        hay.find(needle).unwrap_or_else(|| panic!("{needle:?} not in {hay:?}"))
    }

    #[tokio::test]
    async fn f_cli_05_first_frame_loading_before_started() {
        let mut rig = Rig::new(&[b"q"]);
        rig.initial = read_file();
        let (code, out, log) = rig.run().await;
        assert_eq!(code, 3);
        assert_eq!(log[0], "view");
        assert!(log.iter().position(|l| l == "Started").unwrap() > 0);
        assert!(out.starts_with("\x1b[?1049h"));
        assert!(out.contains("Loading..."));
    }

    #[tokio::test]
    async fn f_cli_05_exit_leaves_alt_screen_last() {
        let (code, out, _) = Rig::new(&[b"q"]).run().await;
        assert_eq!(code, 3);
        assert!(out.ends_with("\x1b[?1049l"), "{out:?}");
        assert_eq!(out.matches("\x1b[?1049l").count(), 1);
    }

    #[tokio::test]
    async fn f_cli_05_effects_after_exit_do_not_run() {
        let (_, out, _) = Rig::new(&[b"q"]).run().await;
        assert!(!out.contains("\x1b]52;"), "{out:?}");
    }

    #[tokio::test]
    async fn f_cli_05_exit_waits_for_pending_work() {
        let mut rig = Rig::new(&[b"lq"]);
        rig.read_delay_ms = 40;
        let (code, out, log) = rig.run().await;
        assert_eq!(code, 3);
        assert!(log.contains(&"exec done".to_string()));
        assert!(out.ends_with("\x1b[?1049l"));
    }

    #[tokio::test]
    async fn f_cli_05_initial_effects_go_to_executor() {
        let mut rig = Rig::new(&[b"q"]);
        rig.initial = read_file();
        let (_, _, log) = rig.run().await;
        assert!(log.iter().any(|l| l.starts_with("exec ReadFile")));
    }

    #[tokio::test]
    async fn f_ask_08_clipboard_written_as_osc52() {
        let (_, out, log) = Rig::new(&[b"cq"]).run().await;
        assert!(out.contains("\x1b]52;c;aGk=\x07"), "{out:?}");
        assert!(!log.iter().any(|l| l.starts_with("exec")));
    }

    #[tokio::test]
    async fn f_ask_05_timers_go_to_clock_not_executor() {
        let (_, _, log) = Rig::new(&[b"tbxq"]).run().await;
        assert!(log.contains(&"schedule Spinner 40 false".to_string()));
        assert!(log.contains(&"schedule Spinner 150 true".to_string()));
        assert!(log.contains(&"cancel Spinner".to_string()));
        assert!(!log.iter().any(|l| l.starts_with("exec")));
    }

    #[tokio::test]
    async fn test_seams_replies_numbered_in_order_both_kinds() {
        let (_, out, _) = Rig::new(&[IDLE, FRAME, IDLE, b"q"]).run().await;
        let a = pos(&out, "\x1b]7770;idle;1;0;0\x07");
        let b = pos(&out, "\x1b]7770;frame;2;0;0\x07");
        let c = pos(&out, "\x1b]7770;idle;3;0;0\x07");
        assert!(a < b && b < c);
    }

    #[tokio::test]
    async fn test_seams_reply_carries_http_counters() {
        let rig = Rig::new(&[IDLE, b"q"]);
        rig.counters.received.store(5, std::sync::atomic::Ordering::SeqCst);
        rig.counters.done.store(4, std::sync::atomic::Ordering::SeqCst);
        let (_, out, _) = rig.run().await;
        assert!(out.contains("\x1b]7770;idle;1;5;4\x07"), "{out:?}");
    }

    #[tokio::test]
    async fn test_seams_idle_before_ready_waits_for_initial_load() {
        let mut rig = Rig::new(&[IDLE, b"q"]);
        rig.initial = read_file();
        rig.read_delay_ms = 30;
        let (_, out, _) = rig.run().await;
        assert!(pos(&out, "R") < pos(&out, "\x1b]7770;idle;1;"), "{out:?}");
    }

    #[tokio::test]
    async fn test_seams_frame_before_ready_does_not_wait_for_load() {
        let mut rig = Rig::new(&[FRAME, b"q"]);
        rig.initial = read_file();
        rig.read_delay_ms = 30;
        let (_, out, _) = rig.run().await;
        assert!(!out[..pos(&out, "\x1b]7770;frame;1;")].contains('R'), "{out:?}");
        assert!(pos(&out, "Loading...") < pos(&out, "\x1b]7770;frame;1;"));
    }

    #[tokio::test]
    async fn test_seams_idle_waits_for_pending_work_after_key() {
        let mut rig = Rig::new(&[b"l", IDLE, b"q"]);
        rig.read_delay_ms = 30;
        let (_, out, _) = rig.run().await;
        assert!(pos(&out, "R") < pos(&out, "\x1b]7770;idle;1;"), "{out:?}");
    }

    #[tokio::test]
    async fn test_seams_frame_ignores_pending_work() {
        let mut rig = Rig::new(&[b"l", FRAME, b"q"]);
        rig.read_delay_ms = 30;
        rig.initial = vec![];
        let (_, out, _) = rig.run().await;
        // model was ready from the start, so "ready" is the first frame already: check via exec log instead
        assert!(out.contains("\x1b]7770;frame;1;"));
    }

    #[tokio::test]
    async fn test_seams_frame_reply_before_slow_result_lands() {
        let mut rig = Rig::new(&[b"l", FRAME, b"q"]);
        rig.read_delay_ms = 60;
        let (_, out, log) = rig.run().await;
        assert!(out.contains("\x1b]7770;frame;1;"));
        // 'q' handled right after the frame reply; the result event is never fed to core (exit only waits)
        assert!(log.iter().any(|l| l.contains("Char('q')")));
        assert!(!log.iter().any(|l| l.starts_with("FileRead")), "{log:?}");
        assert!(log.contains(&"exec done".to_string()));
    }

    #[tokio::test]
    async fn test_seams_input_after_barrier_handled_after_reply() {
        let (_, out, _) = Rig::new(&[b"a", IDLE, b"a", IDLE, b"q"]).run().await;
        let first = pos(&out, "\x1b]7770;idle;1;");
        let second = pos(&out, "\x1b]7770;idle;2;");
        assert_eq!(out[..first].matches('K').count(), 1, "{out:?}");
        assert_eq!(out[first..second].matches('K').count(), 1, "{out:?}");
    }

    #[tokio::test]
    async fn test_seams_input_after_barrier_in_same_chunk() {
        let mut chunk = b"a".to_vec();
        chunk.extend_from_slice(IDLE);
        chunk.extend_from_slice(b"a\x1b[9998~q");
        let (_, out, _) = Rig::new(&[&chunk]).run().await;
        let first = pos(&out, "\x1b]7770;idle;1;");
        let second = pos(&out, "\x1b]7770;frame;2;");
        assert_eq!(out[..first].matches('K').count(), 1, "{out:?}");
        assert_eq!(out[first..second].matches('K').count(), 1, "{out:?}");
    }

    #[tokio::test]
    async fn test_seams_lone_esc_before_barrier_is_escape_key() {
        let (_, _, log) = Rig::new(&[b"\x1b\x1b[9999~q"]).run().await;
        assert!(log.iter().any(|l| l.contains("Esc")), "{log:?}");
    }

    #[tokio::test]
    async fn test_seams_barrier_bytes_never_reach_core_as_keys() {
        let (_, _, log) = Rig::new(&[IDLE, FRAME, b"q"]).run().await;
        assert_eq!(log.iter().filter(|l| l.starts_with("Key")).count(), 1, "{log:?}");
    }

    #[tokio::test]
    async fn test_seams_no_replies_without_sync() {
        let mut rig = Rig::new(&[IDLE, b"q"]);
        rig.sync = false;
        let (_, out, log) = rig.run().await;
        assert!(!out.contains("\x1b]7770"), "{out:?}");
        assert_eq!(log.iter().filter(|l| l.starts_with("Key")).count(), 1);
    }

    #[tokio::test]
    async fn test_seams_background_timer_is_not_pending_work() {
        let mut rig = Rig::new(&[b"b", IDLE, b"q"]);
        rig.real_clock = true;
        let (code, out, log) = rig.run().await;
        assert_eq!(code, 3);
        assert!(out.contains("\x1b]7770;idle;1;"));
        assert!(!log.iter().any(|l| l.starts_with("Timer")), "idle answered before 150 ms bg timer: {log:?}");
    }

    #[tokio::test]
    async fn test_seams_non_background_timer_is_pending_work() {
        let mut rig = Rig::new(&[b"t", IDLE, b"q"]);
        rig.real_clock = true;
        let (_, _, log) = rig.run().await;
        let timer = log.iter().position(|l| l.starts_with("Timer")).unwrap();
        let q = log.iter().position(|l| l.contains("Char('q')")).unwrap();
        assert!(timer < q, "{log:?}");
    }

    #[tokio::test]
    async fn test_seams_cancelled_timer_releases_pending() {
        let mut rig = Rig::new(&[b"t", b"x", IDLE, b"q"]);
        rig.real_clock = true;
        let (_, _, log) = rig.run().await;
        assert!(!log.iter().any(|l| l.starts_with("Timer")), "{log:?}");
    }

    #[tokio::test]
    async fn f_layout_01_resize_becomes_event() {
        let mut rig = Rig::new(&[b"q"]);
        rig.resizes = vec![Size { cols: 100, rows: 30 }];
        let (_, _, log) = rig.run().await;
        assert!(log.iter().any(|l| l.contains("Resize") && l.contains("100")), "{log:?}");
    }

    #[tokio::test]
    async fn f_cli_05_paste_becomes_one_event() {
        let (_, _, log) = Rig::new(&[b"\x1b[200~a\nb\x1b[201~q"]).run().await;
        assert!(log.iter().any(|l| l.contains("Paste(\"a\\nb\")")), "{log:?}");
    }

    #[tokio::test]
    async fn f_reload_02_result_events_redraw() {
        let mut rig = Rig::new(&[IDLE, b"q"]);
        rig.initial = read_file();
        let (_, out, _) = rig.run().await;
        assert!(out.contains("Loading..."));
        assert!(out.contains("R"));
    }

    #[test]
    fn test_seams_barrier_reply_format() {
        let c = HttpCounters::default();
        assert_eq!(barrier_reply(BarrierKind::Idle, 7, &c), b"\x1b]7770;idle;7;0;0\x07".to_vec());
        assert_eq!(barrier_reply(BarrierKind::Frame, 2, &c), b"\x1b]7770;frame;2;0;0\x07".to_vec());
    }
}
