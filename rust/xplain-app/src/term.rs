//! Raw terminal plumbing: raw mode, size query, stdin reader thread, resize notification.
//!
//! Spec: F-CLI-05 (raw mode when stdin is a TTY; non-TTY: keys ignored, UI still renders),
//! F-LAYOUT-01 (size, default 80x24 when unknown or zero).
//! Owner: component A (runtime).
//! Must not: decode bytes (that is `input`), draw (that is `present`), use `unsafe`. Use crossterm's
//! terminal functions and std/tokio IO; restoring raw mode must also happen on drop/panic.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use tokio::sync::mpsc::UnboundedSender;
use xplain_core::screen::Size;

/// Terminal size, `Size::DEFAULT` when unknown (each dimension defaults separately when 0).
pub fn size() -> Size {
    match crossterm::terminal::size() {
        Ok((c, r)) => normalize_size(c, r),
        Err(_) => Size::DEFAULT,
    }
}

/// Zero dimensions fall back to the defaults (F-LAYOUT-01).
pub fn normalize_size(cols: u16, rows: u16) -> Size {
    Size {
        cols: if cols == 0 { Size::DEFAULT.cols } else { cols },
        rows: if rows == 0 { Size::DEFAULT.rows } else { rows },
    }
}

/// True when stdin is a TTY (raw mode possible).
pub fn stdin_is_tty() -> bool {
    use std::io::IsTerminal;
    std::io::stdin().is_terminal()
}

/// Raw mode guard; restores on drop. `enable` is a no-op guard when stdin is not a TTY.
pub struct RawMode {
    active: bool,
}

impl RawMode {
    pub fn enable() -> RawMode {
        let active = stdin_is_tty() && crossterm::terminal::enable_raw_mode().is_ok();
        RawMode { active }
    }

    /// Raw mode that never touched the terminal (tests, non-tty use).
    pub fn inactive() -> RawMode {
        RawMode { active: false }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Restore the terminal. Idempotent.
    pub fn disable(&mut self) {
        if self.active {
            let _ = crossterm::terminal::disable_raw_mode();
            self.active = false;
        }
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        self.disable();
    }
}

/// Last terminal size seen, shared by the stdin reader and the SIGWINCH watcher so a resize is reported once.
/// Chains a panic hook that restores the terminal (raw mode off, main screen, cursor) before the previous hook
/// prints the message. Needed because the release profile aborts on panic, so no `Drop` runs.
pub fn install_panic_hook() {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        prev(info);
    }));
}

/// Best-effort, idempotent terminal restore straight on stdout.
pub fn restore_terminal() {
    use std::io::Write;
    let _ = crossterm::terminal::disable_raw_mode();
    let mut out = std::io::stdout();
    let _ = out.write_all(crate::present::LEAVE);
    let _ = out.flush();
}

#[derive(Debug, Clone)]
pub struct SizeTracker(Arc<AtomicU32>);

impl SizeTracker {
    pub fn new(initial: Size) -> Self {
        SizeTracker(Arc::new(AtomicU32::new(pack(initial))))
    }

    /// Record `size`; true when it differs from the last one recorded.
    pub fn update(&self, size: Size) -> bool {
        self.0.swap(pack(size), Ordering::SeqCst) != pack(size)
    }
}

fn pack(s: Size) -> u32 {
    ((s.cols as u32) << 16) | s.rows as u32
}

pub fn spawn_stdin_reader_sized(
    tx: UnboundedSender<Vec<u8>>,
    resize: Option<(UnboundedSender<Size>, SizeTracker)>,
) {
    use std::io::Read;
    let _ = std::thread::Builder::new().name("stdin-reader".into()).spawn(move || {
        let mut stdin = std::io::stdin();
        let mut buf = [0u8; 4096];
        loop {
            match stdin.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if let Some((rtx, tracker)) = &resize {
                        let now = size();
                        if tracker.update(now) && rtx.send(now).is_err() {
                            break;
                        }
                    }
                    if tx.send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(_) => break,
            }
        }
    });
}

/// Spawn a SIGWINCH watcher that sends the new size on each change (tokio signal), deduplicating against
/// `tracker`. Must be called inside a tokio runtime.
pub fn spawn_resize_watcher_tracked(tx: UnboundedSender<Size>, tracker: SizeTracker) {
    use tokio::signal::unix::{SignalKind, signal};
    let Ok(mut sig) = signal(SignalKind::window_change()) else { return };
    tokio::spawn(async move {
        while sig.recv().await.is_some() {
            let now = size();
            if tracker.update(now) && tx.send(now).is_err() {
                break;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f_layout_01_zero_size_defaults() {
        assert_eq!(normalize_size(0, 0), Size { cols: 80, rows: 24 });
        assert_eq!(normalize_size(0, 40), Size { cols: 80, rows: 40 });
        assert_eq!(normalize_size(120, 0), Size { cols: 120, rows: 24 });
        assert_eq!(normalize_size(100, 30), Size { cols: 100, rows: 30 });
    }

    #[test]
    fn f_layout_01_size_never_zero() {
        let s = size();
        assert!(s.cols > 0 && s.rows > 0);
    }

    #[test]
    fn f_cli_05_raw_mode_noop_without_tty_and_idempotent() {
        let mut r = RawMode::enable();
        if !stdin_is_tty() {
            assert!(!r.is_active());
        }
        r.disable();
        r.disable();
        assert!(!r.is_active());
    }

    #[test]
    fn f_layout_01_size_tracker_reports_changes_once() {
        let t = SizeTracker::new(Size { cols: 80, rows: 24 });
        assert!(!t.update(Size { cols: 80, rows: 24 }));
        assert!(t.update(Size { cols: 100, rows: 24 }));
        assert!(!t.update(Size { cols: 100, rows: 24 }));
        assert!(t.update(Size { cols: 100, rows: 30 }));
    }

    #[tokio::test]
    async fn f_layout_01_resize_watcher_spawns() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        spawn_resize_watcher_tracked(tx, SizeTracker::new(size()));
        assert!(rx.try_recv().is_err());
    }
}
