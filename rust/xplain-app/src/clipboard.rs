//! OSC 52 clipboard sequence.
//!
//! Spec: Contract surface "stdout escape OSC 52", PORTING "Clipboard via OSC 52", F-ASK-08.
//! Owner: component A (runtime).
//! Must not: write to any stream or call system clipboard tools. Pure bytes; the runtime writes them to
//! the same stdout as frames, never interleaved inside a frame.

/// `ESC ] 52 ; c ; <base64 of utf-8 text> BEL`.
pub fn osc52(text: &str) -> Vec<u8> {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(text.as_bytes());
    format!("\x1b]52;c;{b64}\x07").into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f_ask_08_osc52_bytes() {
        assert_eq!(osc52("hi"), b"\x1b]52;c;aGk=\x07".to_vec());
    }

    #[test]
    fn f_ask_08_osc52_utf8_and_empty() {
        assert_eq!(osc52(""), b"\x1b]52;c;\x07".to_vec());
        assert_eq!(osc52("é"), b"\x1b]52;c;w6k=\x07".to_vec());
    }
}
