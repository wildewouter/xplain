//! The event loop and sync barrier.
//!
//! Spec: Test seams (whole section), F-CLI-05 (start/exit), F-RELOAD-02 (silent reload is just an event).
//! Owner: app lead (runtime component); this is the first thing built (spike).
//! Must not: hold UI state beyond `xplain_core::State`, or interpret keys.
//!
//! Loop: decode input -> for each item call `update` (after setting `state.clock`) -> hand effects to the
//! `Executor` -> when the queue is drained call `view` + `Presenter::draw` -> answer any barrier whose
//! conditions hold. Idle barrier holds when: all earlier input handled, the frame was written, event queue
//! empty, `PendingWork == 0`, and (before ready) first frame + initial load done (`State::is_ready`).
//! Frame barrier holds when earlier input handled and frame written. Barrier replies are written in order.

use std::time::Duration;

use xplain_core::event::TimerId;

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
    pub sync: bool,
}

/// Run the app until `Effect::Exit`. Returns the exit code.
pub async fn run_loop(
    _state: xplain_core::State,
    _initial_effects: Vec<xplain_core::Effect>,
    _cfg: RuntimeConfig,
) -> i32 {
    todo!("event loop + barrier")
}
