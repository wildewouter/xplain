//! Pure sync-barrier bookkeeping (no IO).
//!
//! Spec: Test seams (barrier numbering `n` counts both kinds 1-based, replies in order, one per barrier,
//! frame vs idle conditions, barriers may arrive before the first frame).
//! Owner: component A (runtime).
//! Must not: write bytes, touch counters, or know about state. `runtime` feeds it facts, it says which
//! replies are due.

use std::collections::VecDeque;

use crate::input::BarrierKind;

/// Facts the runtime knows at a settle point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settle {
    /// All input items before the barrier went through `update` and their frame was written.
    /// (The runtime only calls `due` after draining input up to the barrier, so this is "first frame
    /// written and no dirty state".)
    pub frame_written: bool,
    /// `frame_written` and event queue empty and `PendingWork == 0` and `State::is_ready()`.
    pub idle: bool,
}

#[derive(Debug, Default)]
pub struct BarrierQueue {
    count: u64,
    waiting: VecDeque<(BarrierKind, u64)>,
}

impl BarrierQueue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an arrived barrier; returns its number `n`.
    pub fn push(&mut self, _kind: BarrierKind) -> u64 {
        let _ = (&self.count, &self.waiting);
        todo!("number and queue")
    }

    /// Pop, in arrival order, every head barrier whose condition holds (`Frame` needs `frame_written`,
    /// `Idle` needs `idle`); stops at the first unsatisfied head so replies stay ordered.
    pub fn due(&mut self, _settle: Settle) -> Vec<(BarrierKind, u64)> {
        todo!("ordered release")
    }

    pub fn is_empty(&self) -> bool {
        self.waiting.is_empty()
    }
}
