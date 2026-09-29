//! Terminal input decoding: bytes -> keys/paste/resize/barriers.
//!
//! Spec: Test seams (barrier bytes `ESC [ 9 9 9 9 ~` idle / `ESC [ 9 9 9 8 ~` frame, never keys; lone ESC
//! directly before a barrier = Escape key; no escape timeout), F-NAV-07, F-CLI-05 (Ctrl+C), UNSPEC-8/26.
//! Owner: component A (runtime).
//! Must not: talk to core state or the terminal; pure byte decoder so it is unit-testable byte-for-byte.
//! Without `XPLAIN_SYNC` barrier bytes are decoded like any other unknown CSI (no special meaning).

use xplain_core::keys::{Key, KeyEvent, Mods};
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
    /// Inside a bracketed paste: bytes collected so far (after `ESC[200~`).
    paste: Option<Vec<u8>>,
}

const PASTE_END: &[u8] = b"\x1b[201~";

/// Result of trying to parse one item at the head of the buffer.
enum Parsed {
    /// Consumed `n` bytes, optionally producing an item.
    Item(usize, Option<InputItem>),
    /// Paste start consumed `n` bytes.
    PasteStart(usize),
    /// Need more bytes.
    More,
}

fn mods_from(param: Option<u32>) -> Mods {
    let m = param.unwrap_or(1).saturating_sub(1);
    Mods { shift: m & 1 != 0, alt: m & 2 != 0, ctrl: m & 4 != 0 }
}

fn key_item(key: Key, mods: Mods) -> Option<InputItem> {
    Some(InputItem::Key(KeyEvent { key, mods }))
}

/// Parse one plain (non-ESC) key at the head of `b`.
fn parse_plain(b: &[u8]) -> Parsed {
    let Some(&c) = b.first() else { return Parsed::More };
    let plain = |k: Key| Parsed::Item(1, key_item(k, Mods::default()));
    match c {
        0x00 => Parsed::Item(1, Some(InputItem::Key(KeyEvent::ctrl(' ')))),
        0x08 | 0x7f => plain(Key::Backspace),
        0x09 => plain(Key::Tab),
        0x0a | 0x0d => plain(Key::Enter),
        0x01..=0x1a => Parsed::Item(1, Some(InputItem::Key(KeyEvent::ctrl((c - 1 + b'a') as char)))),
        0x1c..=0x1f => Parsed::Item(1, Some(InputItem::Key(KeyEvent::ctrl((c + 0x40) as char)))),
        0x20..=0x7e => Parsed::Item(1, Some(InputItem::Key(KeyEvent::ch(c as char)))),
        0xc2..=0xf4 => {
            let need = if c >= 0xf0 {
                4
            } else if c >= 0xe0 {
                3
            } else {
                2
            };
            if b.len() < need {
                // Only wait when the bytes so far are valid continuation bytes.
                return if b[1..].iter().all(|x| x & 0xc0 == 0x80) {
                    Parsed::More
                } else {
                    Parsed::Item(1, None)
                };
            }
            match std::str::from_utf8(&b[..need]) {
                Ok(s) => match s.chars().next() {
                    Some(ch) => Parsed::Item(need, Some(InputItem::Key(KeyEvent::ch(ch)))),
                    None => Parsed::Item(need, None),
                },
                Err(_) => Parsed::Item(1, None),
            }
        }
        _ => Parsed::Item(1, None),
    }
}

fn csi_key(params: &[u32], has_params: bool, fin: u8) -> Option<(Key, Mods)> {
    let p0 = params.first().copied();
    let mods = mods_from(params.get(1).copied());
    let key = match fin {
        b'A' => Key::Up,
        b'B' => Key::Down,
        b'C' => Key::Right,
        b'D' => Key::Left,
        b'H' => Key::Home,
        b'F' => Key::End,
        b'Z' => return Some((Key::BackTab, Mods { shift: false, ..mods })),
        b'~' if has_params => match p0? {
            1 | 7 => Key::Home,
            3 => Key::Delete,
            4 | 8 => Key::End,
            5 => Key::PageUp,
            6 => Key::PageDown,
            _ => return None,
        },
        _ => return None,
    };
    Some((key, mods))
}

impl InputDecoder {
    pub fn new(sync: bool) -> Self {
        Self { sync, buf: Vec::new(), paste: None }
    }

    /// Feed raw stdin bytes. A trailing lone `ESC` stays buffered until the next byte arrives, except
    /// that `ESC` followed by a barrier yields `Key(Esc)` then `Barrier`.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<InputItem> {
        let mut out = Vec::new();
        self.buf.extend_from_slice(bytes);
        let mut pos = 0;
        loop {
            if let Some(p) = self.paste.as_mut() {
                p.extend_from_slice(&self.buf[pos..]);
                pos = self.buf.len();
                if let Some(at) = p.windows(PASTE_END.len()).position(|w| w == PASTE_END) {
                    let rest = p.split_off(at + PASTE_END.len());
                    p.truncate(at);
                    out.push(InputItem::Paste(String::from_utf8_lossy(p).into_owned()));
                    self.paste = None;
                    // Return the remainder to the main buffer.
                    self.buf.clear();
                    self.buf.extend_from_slice(&rest);
                    pos = 0;
                    continue;
                }
                break;
            }
            let rest = &self.buf[pos..];
            if rest.is_empty() {
                break;
            }
            let parsed = if rest[0] == 0x1b { self.parse_esc(rest) } else { parse_plain(rest) };
            match parsed {
                Parsed::More => break,
                Parsed::Item(n, item) => {
                    pos += n;
                    out.extend(item);
                }
                Parsed::PasteStart(n) => {
                    pos += n;
                    self.paste = Some(Vec::new());
                }
            }
        }
        self.buf.drain(..pos);
        out
    }

    fn parse_esc(&self, b: &[u8]) -> Parsed {
        let Some(&n) = b.get(1) else { return Parsed::More };
        match n {
            0x1b => Parsed::Item(1, key_item(Key::Esc, Mods::default())),
            b'[' => self.parse_csi(b),
            b'O' => match b.get(2) {
                None => Parsed::More,
                Some(&f) => {
                    let key = match f {
                        b'A' => Some(Key::Up),
                        b'B' => Some(Key::Down),
                        b'C' => Some(Key::Right),
                        b'D' => Some(Key::Left),
                        b'H' => Some(Key::Home),
                        b'F' => Some(Key::End),
                        _ => None,
                    };
                    Parsed::Item(3, key.and_then(|k| key_item(k, Mods::default())))
                }
            },
            _ => match parse_plain(&b[1..]) {
                Parsed::More => Parsed::More,
                Parsed::Item(len, item) => {
                    let item = match item {
                        Some(InputItem::Key(mut k)) => {
                            k.mods.alt = true;
                            Some(InputItem::Key(k))
                        }
                        other => other,
                    };
                    Parsed::Item(len + 1, item)
                }
                Parsed::PasteStart(_) => Parsed::More,
            },
        }
    }

    fn parse_csi(&self, b: &[u8]) -> Parsed {
        // b = ESC [ params* intermediates* final
        let mut i = 2;
        while i < b.len() && (0x30..=0x3f).contains(&b[i]) {
            i += 1;
        }
        let params_end = i;
        while i < b.len() && (0x20..=0x2f).contains(&b[i]) {
            i += 1;
        }
        let Some(&fin) = b.get(i) else { return Parsed::More };
        if !(0x40..=0x7e).contains(&fin) {
            // Malformed: swallow what we saw plus the offending byte.
            return Parsed::Item(i + 1, None);
        }
        let len = i + 1;
        let raw = &b[2..params_end];
        let plain_params = i == params_end;
        if self.sync && plain_params && fin == b'~' {
            match raw {
                b"9999" => return Parsed::Item(len, Some(InputItem::Barrier(BarrierKind::Idle))),
                b"9998" => return Parsed::Item(len, Some(InputItem::Barrier(BarrierKind::Frame))),
                _ => {}
            }
        }
        if !plain_params {
            return Parsed::Item(len, None);
        }
        let params: Vec<u32> = raw
            .split(|&c| c == b';')
            .map(|p| std::str::from_utf8(p).ok().and_then(|s| s.parse().ok()).unwrap_or(0))
            .collect();
        if fin == b'~' && params.first() == Some(&200) && params.len() == 1 {
            return Parsed::PasteStart(len);
        }
        let item = csi_key(&params, !raw.is_empty(), fin).and_then(|(k, m)| key_item(k, m));
        Parsed::Item(len, item)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(sync: bool, b: &[u8]) -> Vec<InputItem> {
        InputDecoder::new(sync).feed(b)
    }
    fn k(key: Key) -> InputItem {
        InputItem::Key(KeyEvent::plain(key))
    }
    fn km(key: Key, ctrl: bool, alt: bool, shift: bool) -> InputItem {
        InputItem::Key(KeyEvent { key, mods: Mods { ctrl, alt, shift } })
    }

    #[test]
    fn f_nav_07_ctrl_letters() {
        assert_eq!(keys(false, &[0x03]), vec![InputItem::Key(KeyEvent::ctrl('c'))]);
        assert_eq!(keys(false, &[0x01]), vec![InputItem::Key(KeyEvent::ctrl('a'))]);
        assert_eq!(keys(false, &[0x1a]), vec![InputItem::Key(KeyEvent::ctrl('z'))]);
        assert_eq!(keys(false, &[0x0e]), vec![InputItem::Key(KeyEvent::ctrl('n'))]);
    }

    #[test]
    fn f_nav_07_basic_keys() {
        assert_eq!(keys(false, b"\r\n"), vec![k(Key::Enter), k(Key::Enter)]);
        assert_eq!(keys(false, b"\t"), vec![k(Key::Tab)]);
        assert_eq!(keys(false, b"\x1b[Z"), vec![k(Key::BackTab)]);
        assert_eq!(keys(false, &[0x7f, 0x08]), vec![k(Key::Backspace), k(Key::Backspace)]);
        assert_eq!(keys(false, b"aJ?"), vec![k(Key::Char('a')), k(Key::Char('J')), k(Key::Char('?'))]);
    }

    #[test]
    fn f_nav_07_csi_and_ss3() {
        assert_eq!(
            keys(false, b"\x1b[A\x1b[B\x1b[C\x1b[D"),
            vec![k(Key::Up), k(Key::Down), k(Key::Right), k(Key::Left)]
        );
        assert_eq!(keys(false, b"\x1bOA\x1bOH\x1bOF"), vec![k(Key::Up), k(Key::Home), k(Key::End)]);
        assert_eq!(keys(false, b"\x1b[H\x1b[F"), vec![k(Key::Home), k(Key::End)]);
        assert_eq!(
            keys(false, b"\x1b[1~\x1b[4~\x1b[7~\x1b[8~\x1b[3~\x1b[5~\x1b[6~"),
            vec![
                k(Key::Home),
                k(Key::End),
                k(Key::Home),
                k(Key::End),
                k(Key::Delete),
                k(Key::PageUp),
                k(Key::PageDown)
            ]
        );
    }

    #[test]
    fn f_nav_07_modifiers() {
        assert_eq!(keys(false, b"\x1b[1;5A"), vec![km(Key::Up, true, false, false)]);
        assert_eq!(keys(false, b"\x1b[1;2D"), vec![km(Key::Left, false, false, true)]);
        assert_eq!(keys(false, b"\x1b[3;3~"), vec![km(Key::Delete, false, true, false)]);
        assert_eq!(keys(false, b"\x1bx"), vec![km(Key::Char('x'), false, true, false)]);
    }

    #[test]
    fn f_nav_07_unknown_swallowed() {
        assert_eq!(keys(false, b"\x1b[2~a"), vec![k(Key::Char('a'))]);
        assert_eq!(keys(false, b"\x1b[15~\x1bOPb"), vec![k(Key::Char('b'))]);
        assert_eq!(keys(false, b"\x1b[?25ha"), vec![k(Key::Char('a'))]);
    }

    #[test]
    fn f_nav_07_utf8_split_across_reads() {
        let mut d = InputDecoder::new(false);
        let e = "é€😀".as_bytes();
        let mut out = Vec::new();
        for b in e {
            out.extend(d.feed(&[*b]));
        }
        assert_eq!(out, vec![k(Key::Char('é')), k(Key::Char('€')), k(Key::Char('😀'))]);
    }

    #[test]
    fn f_nav_07_csi_split_byte_by_byte() {
        let mut d = InputDecoder::new(false);
        let mut out = Vec::new();
        for b in b"\x1b[1;5A\x1bOB" {
            out.extend(d.feed(&[*b]));
        }
        assert_eq!(out, vec![km(Key::Up, true, false, false), k(Key::Down)]);
    }

    #[test]
    fn f_cli_05_lone_esc_waits() {
        let mut d = InputDecoder::new(false);
        assert!(d.feed(b"\x1b").is_empty());
        assert_eq!(d.feed(b"[A"), vec![k(Key::Up)]);
    }

    #[test]
    fn f_cli_05_paste_single_item() {
        assert_eq!(
            keys(false, b"\x1b[200~a\nb\x1b[201~x"),
            vec![InputItem::Paste("a\nb".into()), k(Key::Char('x'))]
        );
    }

    #[test]
    fn f_cli_05_paste_split_chunks() {
        let mut d = InputDecoder::new(false);
        let all = "\x1b[200~héllo\nwörld\x1b[201~q".as_bytes();
        let mut out = Vec::new();
        for b in all {
            out.extend(d.feed(&[*b]));
        }
        assert_eq!(out, vec![InputItem::Paste("héllo\nwörld".into()), k(Key::Char('q'))]);
    }

    #[test]
    fn f_cli_05_paste_ignores_barrier_bytes() {
        let out = keys(true, b"\x1b[200~\x1b[9999~\x1b[201~");
        assert_eq!(out, vec![InputItem::Paste("\x1b[9999~".into())]);
    }

    #[test]
    fn test_seams_barriers() {
        assert_eq!(
            keys(true, b"\x1b[9999~\x1b[9998~"),
            vec![InputItem::Barrier(BarrierKind::Idle), InputItem::Barrier(BarrierKind::Frame)]
        );
        assert_eq!(keys(true, b"j\x1b[9999~k")[1], InputItem::Barrier(BarrierKind::Idle));
    }

    #[test]
    fn test_seams_barrier_split() {
        let mut d = InputDecoder::new(true);
        let mut out = Vec::new();
        for b in b"\x1b[9998~" {
            out.extend(d.feed(&[*b]));
        }
        assert_eq!(out, vec![InputItem::Barrier(BarrierKind::Frame)]);
    }

    #[test]
    fn test_seams_lone_esc_before_barrier_is_escape() {
        assert_eq!(keys(true, b"\x1b\x1b[9999~"), vec![k(Key::Esc), InputItem::Barrier(BarrierKind::Idle)]);
        let mut d = InputDecoder::new(true);
        assert!(d.feed(b"\x1b").is_empty());
        assert_eq!(d.feed(b"\x1b[9998~"), vec![k(Key::Esc), InputItem::Barrier(BarrierKind::Frame)]);
    }

    #[test]
    fn test_seams_barrier_bytes_ordinary_without_sync() {
        assert!(keys(false, b"\x1b[9999~\x1b[9998~").is_empty());
        assert_eq!(keys(false, b"\x1b[9999~a"), vec![k(Key::Char('a'))]);
    }

    #[test]
    fn unspec_8_paste_keeps_newlines() {
        assert_eq!(keys(false, b"\x1b[200~a\r\nb\x1b[201~"), vec![InputItem::Paste("a\r\nb".into())]);
    }
}
