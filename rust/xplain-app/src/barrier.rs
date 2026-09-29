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
    pub fn push(&mut self, kind: BarrierKind) -> u64 {
        self.count += 1;
        self.waiting.push_back((kind, self.count));
        self.count
    }

    /// Pop, in arrival order, every head barrier whose condition holds (`Frame` needs `frame_written`,
    /// `Idle` needs `idle`); stops at the first unsatisfied head so replies stay ordered.
    pub fn due(&mut self, settle: Settle) -> Vec<(BarrierKind, u64)> {
        let mut out = Vec::new();
        while let Some(&(kind, n)) = self.waiting.front() {
            let ok = match kind {
                BarrierKind::Frame => settle.frame_written,
                BarrierKind::Idle => settle.idle,
            };
            if !ok {
                break;
            }
            self.waiting.pop_front();
            out.push((kind, n));
        }
        out
    }

    pub fn is_empty(&self) -> bool {
        self.waiting.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: Settle = Settle { frame_written: false, idle: false };
    const FRAME: Settle = Settle { frame_written: true, idle: false };
    const IDLE: Settle = Settle { frame_written: true, idle: true };

    #[test]
    fn test_seams_numbering_counts_both_kinds() {
        let mut q = BarrierQueue::new();
        assert!(q.is_empty());
        assert_eq!(q.push(BarrierKind::Idle), 1);
        assert_eq!(q.push(BarrierKind::Frame), 2);
        assert_eq!(q.push(BarrierKind::Idle), 3);
        assert!(!q.is_empty());
    }

    #[test]
    fn test_seams_frame_needs_frame_written() {
        let mut q = BarrierQueue::new();
        q.push(BarrierKind::Frame);
        assert!(q.due(NONE).is_empty());
        assert_eq!(q.due(FRAME), vec![(BarrierKind::Frame, 1)]);
        assert!(q.is_empty());
    }

    #[test]
    fn test_seams_idle_needs_idle() {
        let mut q = BarrierQueue::new();
        q.push(BarrierKind::Idle);
        assert!(q.due(FRAME).is_empty());
        assert_eq!(q.due(IDLE), vec![(BarrierKind::Idle, 1)]);
    }

    #[test]
    fn test_seams_ordered_release() {
        let mut q = BarrierQueue::new();
        q.push(BarrierKind::Idle);
        q.push(BarrierKind::Frame);
        assert!(q.due(FRAME).is_empty(), "frame behind unsatisfied idle waits");
        assert_eq!(q.due(IDLE), vec![(BarrierKind::Idle, 1), (BarrierKind::Frame, 2)]);
    }

    #[test]
    fn test_seams_partial_release() {
        let mut q = BarrierQueue::new();
        q.push(BarrierKind::Frame);
        q.push(BarrierKind::Idle);
        assert_eq!(q.due(FRAME), vec![(BarrierKind::Frame, 1)]);
        assert!(!q.is_empty());
    }
}
