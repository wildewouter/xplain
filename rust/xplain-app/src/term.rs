//! Raw terminal plumbing: raw mode, size query, stdin reader thread, resize notification.
//!
//! Spec: F-CLI-05 (raw mode when stdin is a TTY; non-TTY: keys ignored, UI still renders),
//! F-LAYOUT-01 (size, default 80x24 when unknown or zero).
//! Owner: component A (runtime).
//! Must not: decode bytes (that is `input`), draw (that is `present`), use `unsafe`. Use crossterm's
//! terminal functions and std/tokio IO; restoring raw mode must also happen on drop/panic.

use tokio::sync::mpsc::UnboundedSender;
use xplain_core::screen::Size;

/// Terminal size, `Size::DEFAULT` when unknown.
pub fn size() -> Size {
    todo!("crossterm::terminal::size with default")
}

/// True when stdin is a TTY (raw mode possible).
pub fn stdin_is_tty() -> bool {
    todo!("std::io::IsTerminal")
}

/// Raw mode guard; restores on drop. `enable` is a no-op guard when stdin is not a TTY.
pub struct RawMode {
    active: bool,
}

impl RawMode {
    pub fn enable() -> RawMode {
        RawMode { active: false }
    }
    pub fn disable(&mut self) {
        let _ = self.active;
        todo!("restore")
    }
}

/// Spawn a thread reading stdin in chunks and sending raw bytes; sends nothing more at EOF (the loop
/// keeps running; EOF of stdin does not exit, keys are simply ignored).
pub fn spawn_stdin_reader(_tx: UnboundedSender<Vec<u8>>) {
    todo!("blocking read thread")
}

/// Spawn a SIGWINCH watcher that sends the new size on each change (tokio signal).
pub fn spawn_resize_watcher(_tx: UnboundedSender<Size>) {
    todo!("SIGWINCH -> size()")
}
