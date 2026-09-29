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

use std::time::Duration;

use tokio::sync::mpsc::UnboundedReceiver;
use xplain_core::event::{Event, TimerId};
use xplain_core::screen::Size;

use crate::exec::{Executor, PendingWork};
use crate::http::HttpCounters;
use crate::input::BarrierKind;

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

/// Testable core of the loop: all collaborators injected. `input` carries raw stdin bytes, `resize` terminal
/// size changes, `events` results/timers/HTTP events from executor and clock. Draws the first frame (`Loading...`
/// from `view`) before sending `Event::Started`. Returns the exit code.
#[allow(clippy::too_many_arguments)]
pub async fn drive<E: Executor, C: Clock, W: std::io::Write>(
    _state: xplain_core::State,
    _initial_effects: Vec<xplain_core::Effect>,
    _cfg: &RuntimeConfig,
    _exec: E,
    _clock: C,
    _pending: PendingWork,
    _counters: HttpCounters,
    _events: UnboundedReceiver<Event>,
    _input: UnboundedReceiver<Vec<u8>>,
    _resize: UnboundedReceiver<Size>,
    _out: W,
) -> i32 {
    todo!("event loop + barrier")
}

/// Run the app until `Effect::Exit`. Returns the exit code. Wires the real pieces (channels, `RealClock`,
/// `RealExecutor`, `Presenter` on stdout, `term::spawn_stdin_reader`/`spawn_resize_watcher`) and calls [`drive`].
pub async fn run_loop(
    _state: xplain_core::State,
    _initial_effects: Vec<xplain_core::Effect>,
    _cfg: RuntimeConfig,
) -> i32 {
    todo!("wire real collaborators")
}
