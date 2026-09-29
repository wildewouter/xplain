//! OSC 52 clipboard sequence.
//!
//! Spec: Contract surface "stdout escape OSC 52", PORTING "Clipboard via OSC 52", F-ASK-08.
//! Owner: component A (runtime).
//! Must not: write to any stream or call system clipboard tools. Pure bytes; the runtime writes them to
//! the same stdout as frames, never interleaved inside a frame.

/// `ESC ] 52 ; c ; <base64 of utf-8 text> BEL`.
pub fn osc52(_text: &str) -> Vec<u8> {
    todo!("base64 + wrap")
}
