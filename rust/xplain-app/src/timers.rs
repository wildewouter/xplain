//! Timers and wall clock for the real runtime.
//!
//! Spec: Test seams (spinner not pending work; `background` timers), F-ASK-05 (80 ms spinner),
//! F-MCPSRV-06 (poll timeout timers), F-EXPORT-01 (local time offset for `state.clock`).
//! Owner: component A (runtime).
//! Must not: decide when to arm timers (core emits `SetTimer`/`CancelTimer`). Non-background timers count
//! as pending work from arm until their `Event::Timer` was enqueued or they were cancelled/replaced;
//! background timers never do. Re-arming an id replaces the old timer.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::sync::mpsc::UnboundedSender;
use xplain_core::event::{Event, TimerId};
use xplain_core::state::Now;

use crate::exec::PendingWork;
use crate::runtime::Clock;

type Active = (tokio::task::JoinHandle<()>, Arc<AtomicBool>);

/// Tokio-backed [`Clock`].
pub struct RealClock {
    tx: UnboundedSender<Event>,
    pending: PendingWork,
    utc_offset_secs: i32,
    timers: HashMap<TimerId, Active>,
}

impl RealClock {
    /// Local UTC offset must be determined here, once, before other threads exist when possible
    /// (the `time` crate refuses local offsets in multithreaded processes); fall back to 0.
    pub fn new(tx: UnboundedSender<Event>, pending: PendingWork) -> Self {
        let utc_offset_secs = time::UtcOffset::current_local_offset()
            .map(|o| o.whole_seconds())
            .ok()
            .or_else(offset_from_date)
            .unwrap_or(0);
        RealClock { tx, pending, utc_offset_secs, timers: HashMap::new() }
    }

    /// Stop timer `id` (if any) and release its pending count.
    fn drop_timer(&mut self, id: TimerId) {
        if let Some((handle, counted)) = self.timers.remove(&id) {
            handle.abort();
            release(&counted, &self.pending);
        }
    }
}

/// Fallback when the `time` crate refuses (multithreaded process): ask `date +%z`.
fn offset_from_date() -> Option<i32> {
    let out = std::process::Command::new("date").arg("+%z").output().ok()?;
    parse_offset(String::from_utf8_lossy(&out.stdout).trim())
}

/// `+HHMM` / `-HHMM` to seconds.
fn parse_offset(s: &str) -> Option<i32> {
    let (sign, rest) = match s.as_bytes().first()? {
        b'+' => (1, &s[1..]),
        b'-' => (-1, &s[1..]),
        _ => return None,
    };
    if rest.len() != 4 || !rest.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let h: i32 = rest[..2].parse().ok()?;
    let m: i32 = rest[2..].parse().ok()?;
    Some(sign * (h * 3600 + m * 60))
}

/// Release the pending count once (whoever gets there first: fired task or cancel/replace).
fn release(counted: &AtomicBool, pending: &PendingWork) {
    if counted.swap(false, Ordering::SeqCst) {
        pending.end();
    }
}

impl Clock for RealClock {
    fn now(&self) -> Now {
        let unix_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        Now { unix_ms, utc_offset_secs: self.utc_offset_secs }
    }

    fn schedule(&mut self, id: TimerId, after: Duration, background: bool) {
        self.drop_timer(id);
        let counted = Arc::new(AtomicBool::new(!background));
        if !background {
            self.pending.begin();
        }
        let tx = self.tx.clone();
        let pending = self.pending.clone();
        let flag = counted.clone();
        let handle = tokio::spawn(async move {
            tokio::time::sleep(after).await;
            let _ = tx.send(Event::Timer(id));
            release(&flag, &pending);
        });
        self.timers.insert(id, (handle, counted));
    }

    fn cancel(&mut self, id: TimerId) {
        self.drop_timer(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc::unbounded_channel;

    #[test]
    fn f_export_01_parse_offset() {
        assert_eq!(parse_offset("+0000"), Some(0));
        assert_eq!(parse_offset("+0530"), Some(19800));
        assert_eq!(parse_offset("-0800"), Some(-28800));
        assert_eq!(parse_offset("0100"), None);
        assert_eq!(parse_offset("+01"), None);
        assert_eq!(parse_offset(""), None);
    }

    #[test]
    fn test_seams_now_is_sane() {
        let (tx, _rx) = unbounded_channel();
        let c = RealClock::new(tx, PendingWork::default());
        let n = c.now();
        assert!(n.unix_ms > 1_600_000_000_000);
        assert!(n.utc_offset_secs.abs() <= 26 * 3600);
        assert_eq!(c.now().utc_offset_secs, n.utc_offset_secs);
    }

    #[tokio::test]
    async fn test_seams_timer_fires_and_pending_released() {
        let (tx, mut rx) = unbounded_channel();
        let p = PendingWork::default();
        let mut c = RealClock::new(tx, p.clone());
        c.schedule(TimerId::Spinner, Duration::from_millis(10), false);
        assert_eq!(p.count(), 1);
        assert_eq!(rx.recv().await, Some(Event::Timer(TimerId::Spinner)));
        assert_eq!(p.count(), 0);
    }

    #[tokio::test]
    async fn test_seams_background_timer_not_pending() {
        let (tx, mut rx) = unbounded_channel();
        let p = PendingWork::default();
        let mut c = RealClock::new(tx, p.clone());
        c.schedule(TimerId::Spinner, Duration::from_millis(10), true);
        assert_eq!(p.count(), 0);
        assert_eq!(rx.recv().await, Some(Event::Timer(TimerId::Spinner)));
        assert_eq!(p.count(), 0);
    }

    #[tokio::test]
    async fn test_seams_cancel_releases_pending_and_never_fires() {
        let (tx, mut rx) = unbounded_channel();
        let p = PendingWork::default();
        let mut c = RealClock::new(tx, p.clone());
        c.schedule(TimerId::Spinner, Duration::from_millis(30), false);
        assert_eq!(p.count(), 1);
        c.cancel(TimerId::Spinner);
        assert_eq!(p.count(), 0);
        c.cancel(TimerId::Spinner);
        assert_eq!(p.count(), 0);
        tokio::time::sleep(Duration::from_millis(60)).await;
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_seams_rearm_replaces() {
        let (tx, mut rx) = unbounded_channel();
        let p = PendingWork::default();
        let mut c = RealClock::new(tx, p.clone());
        c.schedule(TimerId::Spinner, Duration::from_millis(20), false);
        c.schedule(TimerId::Spinner, Duration::from_millis(20), false);
        assert_eq!(p.count(), 1);
        assert_eq!(rx.recv().await, Some(Event::Timer(TimerId::Spinner)));
        tokio::time::sleep(Duration::from_millis(40)).await;
        assert!(rx.try_recv().is_err());
        assert_eq!(p.count(), 0);
    }
}
