//! Terminal input decoding: bytes -> keys/paste/resize/barriers.
//!
//! Spec: Test seams (barrier bytes `ESC [ 9 9 9 9 ~` idle / `ESC [ 9 9 9 8 ~` frame, never keys; lone ESC
//! directly before a barrier = Escape key; no escape timeout), F-NAV-07, F-CLI-05 (Ctrl+C), UNSPEC-8/26.
//! Owner: app lead (input component).
//! Must not: talk to core state or the terminal; pure byte decoder so it is unit-testable byte-for-byte.
//! Without `XPLAIN_SYNC` barrier bytes are decoded like any other unknown CSI (no special meaning).

use xplain_core::keys::KeyEvent;
use xplain_core::screen::Size;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarrierKind {
    Idle,
    Frame,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InputItem {
    Key(KeyEvent),
    Paste(String),
    Resize(Size),
    /// Sync barrier; only produced when the decoder was created with `sync = true`.
    Barrier(BarrierKind),
}

/// Incremental decoder (handles sequences split across reads).
#[derive(Debug, Default)]
pub struct InputDecoder {
    pub sync: bool,
    buf: Vec<u8>,
}

impl InputDecoder {
    pub fn new(sync: bool) -> Self {
        Self { sync, buf: Vec::new() }
    }

    /// Feed raw stdin bytes. A trailing lone `ESC` stays buffered until the next byte arrives, except
    /// that `ESC` followed by a barrier yields `Key(Esc)` then `Barrier`.
    pub fn feed(&mut self, _bytes: &[u8]) -> Vec<InputItem> {
        let _ = &self.buf;
        todo!("input decoding")
    }
}
