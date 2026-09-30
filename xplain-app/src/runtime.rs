//! The event loop.
//!
//! Spec: F-CLI-05 (start/exit), F-RELOAD-02 (silent reload is just an event).
//! Owner: component A (runtime); first thing built (spike). Effects `Clipboard`, `SetTimer`, `CancelTimer`, `Exit` are
//! handled here (stdout/clock/loop); all other effects go to the `Executor`. `Exit` waits until `PendingWork == 0`
//! (earlier effects incl. HttpReply/McpStop finished), leaves the alternate screen, returns the code.
//! Must not: hold UI state beyond `xplain_core::State`, or interpret keys.
//!
//! Loop: decode ALL queued input chunks -> for each item call `update` (after setting `state.clock`) -> hand effects to the
//! `Executor` -> when the queue is drained call `view` + `Presenter::draw`.

use std::collections::VecDeque;
use std::io::Write;
use std::time::Duration;

use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};
use xplain_core::effect::Effect;
use xplain_core::event::{Event, TimerId};
use xplain_core::screen::{Screen, Size};
use xplain_core::state::Now;

use crate::clipboard::osc52;
use crate::exec::{Executor, PendingWork, RealExecutor};
use crate::input::{InputDecoder, InputItem};
use crate::present::Presenter;
use crate::term::RawMode;
use crate::timers::RealClock;

/// Clock abstraction so tests can drive time. Timers are events: the runtime schedules `TimerId`s and
/// injects `Event::Timer`.
pub trait Clock {
    fn now(&self) -> xplain_core::state::Now;
    fn schedule(&mut self, id: TimerId, after: Duration, background: bool);
    fn cancel(&mut self, id: TimerId);
    /// Cancel every armed timer (releases their pending-work units); used when the loop is exiting.
    fn cancel_all(&mut self);
}

/// Everything `run` needs from the outside.
pub struct RuntimeConfig {
    /// 24-bit colors (`COLORTERM`), passed to the presenter.
    pub truecolor: bool,
}

/// What the loop needs from the core: `update`, `view`, clock injection. Real impl wraps
/// `xplain_core::State`; tests use a fake so the loop is testable on its own.
pub(crate) trait Model {
    fn set_clock(&mut self, now: Now);
    fn update(&mut self, event: Event) -> Vec<Effect>;
    fn view(&self) -> Screen;
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
}

/// Receivers feeding the loop: `events` results/timers/HTTP events from executor and clock, `input` raw stdin
/// bytes, `resize` terminal size changes.
pub struct Inputs {
    pub events: UnboundedReceiver<Event>,
    pub input: UnboundedReceiver<Vec<u8>>,
    pub resize: UnboundedReceiver<Size>,
}

/// Every collaborator of the loop, injected so tests can fake them: executor, clock, pending-work counter, input
/// channels, output writer and the raw-mode guard the presenter takes over.
pub struct Io<E, C, W> {
    pub exec: E,
    pub clock: C,
    pub pending: PendingWork,
    pub inputs: Inputs,
    pub out: W,
    pub raw: RawMode,
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

    /// Exit: cancel timers, wait (bounded) for earlier work (HttpReply, McpStop, ...), restore the terminal,
    /// return the code.
    async fn finish(mut self, code: i32) -> i32 {
        self.clock.cancel_all();
        let _ = tokio::time::timeout(EXIT_GRACE, self.pending.wait_idle()).await;
        let _ = self.presenter.leave(&mut self.out);
        code
    }
}

/// Longest exit waits for outstanding work before restoring the terminal anyway.
const EXIT_GRACE: Duration = Duration::from_secs(5);

/// The loop with all collaborators injected. Draws the first frame (`Loading...` from `view`) before sending
/// `Event::Started`. Returns the exit code.
pub(crate) async fn drive_model<M: Model, E: Executor, C: Clock, W: Write>(
    model: M,
    initial_effects: Vec<Effect>,
    cfg: &RuntimeConfig,
    io: Io<E, C, W>,
) -> i32 {
    let Io { exec, clock, pending, inputs, mut out, raw } = io;
    let Inputs { mut events, mut input, mut resize } = inputs;
    let Ok(presenter) = Presenter::enter(&mut out, raw) else { return 1 };
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

    let mut decoder = InputDecoder::new();
    let mut items: VecDeque<InputItem> = VecDeque::new();
    let (mut events_open, mut input_open, mut resize_open) = (true, true, true);
    loop {
        let mut progress = false;
        while let Ok(size) = resize.try_recv() {
            progress = true;
            step!(Event::Resize(size));
        }
        // Take every queued stdin chunk before touching the screen: a held key queues many chunks while a frame
        // is drawn, and each must not cost its own frame.
        loop {
            match input.try_recv() {
                Ok(b) => {
                    progress = true;
                    items.extend(decoder.feed(&b));
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    input_open = false;
                    break;
                }
            }
        }
        while let Some(item) = items.pop_front() {
            progress = true;
            match item {
                InputItem::Key(k) => step!(Event::Key(k)),
                InputItem::Paste(t) => step!(Event::Paste(t)),
                InputItem::Resize(sz) => step!(Event::Resize(sz)),
            }
        }
        // Drain results.
        while let Ok(ev) = events.try_recv() {
            progress = true;
            step!(ev);
        }
        if rt.dirty {
            progress = true;
            if rt.draw().is_err() {
                return rt.finish(1).await;
            }
        }
        if progress {
            continue;
        }
        if !events_open && !input_open && !resize_open {
            return rt.finish(0).await;
        }
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
        }
    }
}

/// Run the app until `Effect::Exit`. Returns the exit code. Wires the real pieces (channels, `RealClock`,
/// `RealExecutor`, `Presenter` on stdout, `term::spawn_stdin_reader_sized`/`spawn_resize_watcher_tracked`) and
/// calls [`drive_model`].
pub async fn run_loop(
    state: xplain_core::State,
    initial_effects: Vec<xplain_core::Effect>,
    cfg: RuntimeConfig,
) -> i32 {
    let (events_tx, events) = unbounded_channel();
    let (input_tx, input) = unbounded_channel();
    let (resize_tx, resize) = unbounded_channel();
    let pending = PendingWork::default();
    let clock = RealClock::new(events_tx.clone(), pending.clone());
    let exec = RealExecutor::new(events_tx, pending.clone());
    let tracker = crate::term::SizeTracker::new(crate::term::size());
    crate::term::spawn_resize_watcher_tracked(resize_tx.clone(), tracker.clone());
    crate::term::spawn_stdin_reader_sized(input_tx, Some((resize_tx, tracker)));
    let io = Io {
        exec,
        clock,
        pending,
        inputs: Inputs { events, input, resize },
        out: std::io::stdout(),
        raw: RawMode::enable(),
    };
    drive_model(CoreModel(state), initial_effects, &cfg, io).await
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
        fn cancel_all(&mut self) {
            if let TestClock::Real(c) = self {
                c.cancel_all();
            }
        }
    }

    struct Rig {
        initial: Vec<Effect>,
        real_clock: bool,
        read_delay_ms: u64,
        /// Send `q` through the event channel after this many ms (0 = never), so results can land first.
        quit_after_ms: u64,
        input: Vec<Vec<u8>>,
        resizes: Vec<Size>,
    }

    impl Rig {
        fn new(input: &[&[u8]]) -> Rig {
            Rig {
                initial: vec![],
                real_clock: false,
                read_delay_ms: 20,
                quit_after_ms: 0,
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
            let cfg = RuntimeConfig { truecolor: true };
            let io = Io {
                exec,
                clock,
                pending,
                inputs: Inputs { events: erx, input: irx, resize: rrx },
                out: out.clone(),
                raw: RawMode::inactive(),
            };
            if self.quit_after_ms > 0 {
                let (tx, d) = (etx.clone(), self.quit_after_ms);
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(d)).await;
                    let _ = tx.send(Event::Key(xplain_core::keys::KeyEvent::plain(Key::Char('q'))));
                });
            }
            let fut = drive_model(model, self.initial, &cfg, io);
            let code = tokio::time::timeout(Duration::from_secs(5), fut).await.unwrap_or(-999);
            drop((etx, itx, rtx));
            let log = log.lock().map(|l| l.clone()).unwrap_or_default();
            (code, out.text(), log)
        }
    }

    fn read_file() -> Vec<Effect> {
        vec![Effect::ReadFile { req: ReqId(1), path: "p".into() }]
    }

    #[tokio::test(start_paused = true)]
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

    #[tokio::test(start_paused = true)]
    async fn f_cli_05_exit_leaves_alt_screen_last() {
        let (code, out, _) = Rig::new(&[b"q"]).run().await;
        assert_eq!(code, 3);
        assert!(out.ends_with("\x1b[?1049l"), "{out:?}");
        assert_eq!(out.matches("\x1b[?1049l").count(), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn f_cli_05_effects_after_exit_do_not_run() {
        let (_, out, _) = Rig::new(&[b"q"]).run().await;
        assert!(!out.contains("\x1b]52;"), "{out:?}");
    }

    #[tokio::test(start_paused = true)]
    async fn f_cli_05_exit_waits_for_pending_work() {
        let mut rig = Rig::new(&[b"lq"]);
        rig.read_delay_ms = 40;
        let (code, out, log) = rig.run().await;
        assert_eq!(code, 3);
        assert!(log.contains(&"exec done".to_string()));
        assert!(out.ends_with("\x1b[?1049l"));
    }

    #[tokio::test(start_paused = true)]
    async fn f_cli_05_exit_cancels_timers_instead_of_waiting_for_them() {
        let mut rig = Rig::new(&[b"tq"]);
        rig.real_clock = true;
        let (code, out, log) = rig.run().await;
        assert_eq!(code, 3);
        assert!(out.ends_with("\x1b[?1049l"));
        assert!(!log.iter().any(|l| l.starts_with("Timer")), "{log:?}");
    }

    #[tokio::test(start_paused = true)]
    async fn f_cli_05_exit_wait_is_bounded() {
        let mut rig = Rig::new(&[b"lq"]);
        rig.read_delay_ms = 60_000;
        let (code, out, _) = rig.run().await;
        assert_eq!(code, 3, "exit must not hang on stuck work");
        assert!(out.ends_with("\x1b[?1049l"));
    }

    #[tokio::test(start_paused = true)]
    async fn f_cli_05_initial_effects_go_to_executor() {
        let mut rig = Rig::new(&[b"q"]);
        rig.initial = read_file();
        let (_, _, log) = rig.run().await;
        assert!(log.iter().any(|l| l.starts_with("exec ReadFile")));
    }

    #[tokio::test(start_paused = true)]
    async fn f_ask_08_clipboard_written_as_osc52() {
        let (_, out, log) = Rig::new(&[b"cq"]).run().await;
        assert!(out.contains("\x1b]52;c;aGk=\x07"), "{out:?}");
        assert!(!log.iter().any(|l| l.starts_with("exec")));
    }

    #[tokio::test(start_paused = true)]
    async fn f_ask_05_timers_go_to_clock_not_executor() {
        let (_, _, log) = Rig::new(&[b"tbxq"]).run().await;
        assert!(log.contains(&"schedule Spinner 40 false".to_string()));
        assert!(log.contains(&"schedule Spinner 150 true".to_string()));
        assert!(log.contains(&"cancel Spinner".to_string()));
        assert!(!log.iter().any(|l| l.starts_with("exec")));
    }

    #[tokio::test(start_paused = true)]
    async fn queued_input_chunks_cost_one_frame() {
        // Four single-key chunks are already queued: all are applied before the next draw.
        let mut rig = Rig::new(&[b"a", b"a", b"a", b"a"]);
        rig.quit_after_ms = 100;
        let (_, out, log) = rig.run().await;
        let views = log.iter().filter(|l| l.as_str() == "view").count();
        assert_eq!(views, 2, "first frame + one frame for the whole burst: {log:?}");
        assert_eq!(log.iter().filter(|l| l.starts_with("Key")).count(), 5);
        assert_eq!(out.matches("KKKK").count(), 1, "{out:?}");
    }

    #[tokio::test(start_paused = true)]
    async fn f_layout_01_resize_becomes_event() {
        let mut rig = Rig::new(&[b"q"]);
        rig.resizes = vec![Size { cols: 100, rows: 30 }];
        let (_, _, log) = rig.run().await;
        assert!(log.iter().any(|l| l.contains("Resize") && l.contains("100")), "{log:?}");
    }

    #[tokio::test(start_paused = true)]
    async fn f_cli_05_paste_becomes_one_event() {
        let (_, _, log) = Rig::new(&[b"\x1b[200~a\nb\x1b[201~q"]).run().await;
        assert!(log.iter().any(|l| l.contains("Paste(\"a\\nb\")")), "{log:?}");
    }

    #[tokio::test(start_paused = true)]
    async fn f_reload_02_result_events_redraw() {
        let mut rig = Rig::new(&[]);
        rig.initial = read_file();
        rig.read_delay_ms = 30;
        rig.quit_after_ms = 100;
        let (_, out, _) = rig.run().await;
        assert!(out.contains("Loading..."));
        assert!(out.contains("R"));
    }
}
