//! Timers and wall clock for the real runtime.
//!
//! Spec: Test seams (spinner not pending work; `background` timers), F-ASK-05 (80 ms spinner),
//! F-MCPSRV-06 (poll timeout timers), F-EXPORT-01 (local time offset for `state.clock`).
//! Owner: component A (runtime).
//! Must not: decide when to arm timers (core emits `SetTimer`/`CancelTimer`). Non-background timers count
//! as pending work from arm until their `Event::Timer` was enqueued or they were cancelled/replaced;
//! background timers never do. Re-arming an id replaces the old timer.

use std::time::Duration;

use tokio::sync::mpsc::UnboundedSender;
use xplain_core::event::{Event, TimerId};
use xplain_core::state::Now;

use crate::exec::PendingWork;
use crate::runtime::Clock;

/// Tokio-backed [`Clock`].
pub struct RealClock {
    _tx: UnboundedSender<Event>,
    _pending: PendingWork,
    _utc_offset_secs: i32,
}

impl RealClock {
    /// Local UTC offset must be determined here, once, before other threads exist when possible
    /// (the `time` crate refuses local offsets in multithreaded processes); fall back to 0.
    pub fn new(tx: UnboundedSender<Event>, pending: PendingWork) -> Self {
        RealClock { _tx: tx, _pending: pending, _utc_offset_secs: 0 }
    }
}

impl Clock for RealClock {
    fn now(&self) -> Now {
        todo!("unix ms + offset")
    }
    fn schedule(&mut self, _id: TimerId, _after: Duration, _background: bool) {
        todo!("spawn sleep task")
    }
    fn cancel(&mut self, _id: TimerId) {
        todo!("abort task")
    }
}
