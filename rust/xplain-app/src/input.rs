//! Terminal input decoding: bytes -> keys/paste/resize/barriers.
//!
//! Spec: Test seams (barrier bytes `ESC [ 9 9 9 9 ~` idle / `ESC [ 9 9 9 8 ~` frame, never keys; lone ESC
//! directly before a barrier = Escape key; no escape timeout), F-NAV-07, F-CLI-05 (Ctrl+C), UNSPEC-8/26.
//! Owner: component A (runtime).
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
/// Supported input (spec-relevant): printable UTF-8 (split across reads), Enter (CR/LF), Tab, BackTab (`ESC [ Z`),
/// Backspace (0x7f/0x08), Delete, arrows, Home/End (CSI and SS3 forms, `~` forms), PageUp/PageDown, Esc,
/// Alt+char (ESC prefix), ctrl letters (0x01..0x1a; Ctrl+C is `KeyEvent::ctrl('c')`), modifiers via
/// `CSI 1;<m>X`, bracketed paste `ESC [ 200 ~ .. ESC [ 201 ~` -> one `Paste`. Unknown CSI/SS3 sequences are
/// swallowed (no keys). Escape without following byte stays buffered (no timeout, except before a barrier).
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
