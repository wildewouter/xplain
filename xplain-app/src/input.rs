//! Terminal input decoding: bytes -> keys/paste/resize.
//!
//! Spec: F-NAV-07, F-CLI-05 (Ctrl+C, lone ESC at the end of a read is Escape), UNSPEC-8/26.
//! Owner: component A (runtime).
//! Must not: talk to core state or the terminal; pure byte decoder so it is unit-testable byte-for-byte.

use xplain_core::keys::{Key, KeyEvent, Mods};
use xplain_core::screen::Size;

#[derive(Debug, Clone, PartialEq)]
pub enum InputItem {
    Key(KeyEvent),
    Paste(String),
    Resize(Size),
}

/// Incremental decoder (handles sequences split across reads).
/// Supported input (spec-relevant): printable UTF-8 (split across reads), Enter (CR/LF), Tab, BackTab (`ESC [ Z`),
/// Backspace (0x7f/0x08), Delete, arrows, Home/End (CSI and SS3 forms, `~` forms), PageUp/PageDown, Esc,
/// Alt+char (ESC prefix), ctrl letters (0x01..0x1a; Ctrl+C is `KeyEvent::ctrl('c')`), modifiers via
/// `CSI 1;<m>X`, CSI-u (`CSI code;mods u`) and modifyOtherKeys (`CSI 27;mods;code ~`) forms, bracketed paste `ESC [ 200 ~ .. ESC [ 201 ~` -> one `Paste`. Unknown CSI/SS3 sequences are
/// swallowed (no keys). A lone ESC at the END of a read chunk is Escape (no timer). ESC followed by more bytes in the
/// same chunk is a sequence or Alt+key. A sequence split across reads is a known rare edge case (yields Esc, then text).
#[derive(Debug, Default)]
pub struct InputDecoder {
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
/// Unbracketed paste: one write holding printable text and CR/LF (no controls, no ESC). Length of that
/// run, when it holds a newline and something else. Typed keys arrive one per write, so this is a paste.
fn raw_paste_len(b: &[u8]) -> Option<usize> {
    let n = b.iter().position(|&c| c == 0x1b).unwrap_or(b.len());
    let run = std::str::from_utf8(&b[..n]).ok()?;
    let has_nl = run.contains(['\r', '\n']);
    let has_text = run.chars().any(|c| !matches!(c, '\r' | '\n'));
    let clean = run.chars().all(|c| matches!(c, '\r' | '\n') || !c.is_control());
    (has_nl && has_text && clean).then_some(n)
}

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

/// Key from a CSI-u / modifyOtherKeys codepoint. `code` is the unshifted key; Shift on a letter gives the capital,
/// on other printable chars the modifier is dropped (the char is already what the layout produced).
fn code_key(code: u32, mut mods: Mods) -> Option<InputItem> {
    let key = match code {
        9 if mods.shift => {
            mods.shift = false;
            Key::BackTab
        }
        9 => Key::Tab,
        13 => Key::Enter,
        27 => Key::Esc,
        8 | 127 => Key::Backspace,
        _ => {
            let mut c = char::from_u32(code).filter(|c| !c.is_control())?;
            if mods.shift {
                c = c.to_uppercase().next().unwrap_or(c);
                mods.shift = false;
            }
            if mods.ctrl {
                c = c.to_ascii_lowercase();
            }
            Key::Char(c)
        }
    };
    key_item(key, mods)
}

impl InputDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed raw stdin bytes (one read). A trailing lone `ESC` at the end of the chunk is the Escape key.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<InputItem> {
        let mut out = Vec::new();
        // Like Ink: a raw chunk mixing text with CR/LF (before any escape) is one paste, not keys.
        let mut bytes = bytes;
        if self.buf.is_empty() && self.paste.is_none() {
            let end = bytes.iter().position(|&c| c == 0x1b).unwrap_or(bytes.len());
            let head = &bytes[..end];
            if head.iter().any(|&c| c == b'\r' || c == b'\n')
                && head.iter().any(|&c| c >= 0x20 && c != 0x7f)
                && let Ok(s) = std::str::from_utf8(head)
            {
                out.push(InputItem::Paste(s.to_string()));
                bytes = &bytes[end..];
            }
        }
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
            let parsed = if rest[0] == 0x1b {
                self.parse_esc(rest)
            } else if let Some(n) = raw_paste_len(rest) {
                let text = String::from_utf8_lossy(&rest[..n]).into_owned();
                Parsed::Item(n, Some(InputItem::Paste(text)))
            } else {
                parse_plain(rest)
            };
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
        if self.paste.is_none() && self.buf[pos..] == [0x1b] {
            pos += 1;
            out.extend(key_item(Key::Esc, Mods::default()));
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
        if !plain_params {
            return Parsed::Item(len, None);
        }
        let params: Vec<u32> = raw
            .split(|&c| c == b';')
            // Kitty sub-parameters (`2:1` = mods:event) keep only the leading number.
            .map(|p| {
                let p = p.split(|&c| c == b':').next().unwrap_or(p);
                std::str::from_utf8(p).ok().and_then(|s| s.parse().ok()).unwrap_or(0)
            })
            .collect();
        if fin == b'~' && params.first() == Some(&200) && params.len() == 1 {
            return Parsed::PasteStart(len);
        }
        if (fin == b'u' && !raw.is_empty()) || (fin == b'~' && params.len() == 3 && params[0] == 27) {
            let (code, mods) =
                if fin == b'u' { (params[0], params.get(1)) } else { (params[2], params.get(1)) };
            return Parsed::Item(len, code_key(code, mods_from(mods.copied())));
        }
        let item = csi_key(&params, !raw.is_empty(), fin).and_then(|(k, m)| key_item(k, m));
        Parsed::Item(len, item)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(b: &[u8]) -> Vec<InputItem> {
        InputDecoder::new().feed(b)
    }
    fn k(key: Key) -> InputItem {
        InputItem::Key(KeyEvent::plain(key))
    }
    fn km(key: Key, ctrl: bool, alt: bool, shift: bool) -> InputItem {
        InputItem::Key(KeyEvent { key, mods: Mods { ctrl, alt, shift } })
    }

    #[test]
    fn f_nav_07_ctrl_letters() {
        assert_eq!(keys(&[0x03]), vec![InputItem::Key(KeyEvent::ctrl('c'))]);
        assert_eq!(keys(&[0x01]), vec![InputItem::Key(KeyEvent::ctrl('a'))]);
        assert_eq!(keys(&[0x1a]), vec![InputItem::Key(KeyEvent::ctrl('z'))]);
        assert_eq!(keys(&[0x0e]), vec![InputItem::Key(KeyEvent::ctrl('n'))]);
    }

    #[test]
    fn f_comment_02_mixed_chunk_is_paste() {
        assert_eq!(keys(b"\r\nx\r\n\ny"), vec![InputItem::Paste("\r\nx\r\n\ny".into())]);
        assert_eq!(keys(b"a\rb"), vec![InputItem::Paste("a\rb".into())]);
    }

    #[test]
    fn f_nav_07_basic_keys() {
        assert_eq!(keys(b"\r\n"), vec![k(Key::Enter), k(Key::Enter)]);
        assert_eq!(keys(b"\t"), vec![k(Key::Tab)]);
        assert_eq!(keys(b"\x1b[Z"), vec![k(Key::BackTab)]);
        assert_eq!(keys(&[0x7f, 0x08]), vec![k(Key::Backspace), k(Key::Backspace)]);
        assert_eq!(keys(b"aJ?"), vec![k(Key::Char('a')), k(Key::Char('J')), k(Key::Char('?'))]);
    }

    #[test]
    fn f_nav_07_csi_and_ss3() {
        assert_eq!(
            keys(b"\x1b[A\x1b[B\x1b[C\x1b[D"),
            vec![k(Key::Up), k(Key::Down), k(Key::Right), k(Key::Left)]
        );
        assert_eq!(keys(b"\x1bOA\x1bOH\x1bOF"), vec![k(Key::Up), k(Key::Home), k(Key::End)]);
        assert_eq!(keys(b"\x1b[H\x1b[F"), vec![k(Key::Home), k(Key::End)]);
        assert_eq!(
            keys(b"\x1b[1~\x1b[4~\x1b[7~\x1b[8~\x1b[3~\x1b[5~\x1b[6~"),
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
        assert_eq!(keys(b"\x1b[1;5A"), vec![km(Key::Up, true, false, false)]);
        assert_eq!(keys(b"\x1b[1;2D"), vec![km(Key::Left, false, false, true)]);
        assert_eq!(keys(b"\x1b[3;3~"), vec![km(Key::Delete, false, true, false)]);
        assert_eq!(keys(b"\x1bx"), vec![km(Key::Char('x'), false, true, false)]);
    }

    #[test]
    fn f_nav_07_unknown_swallowed() {
        assert_eq!(keys(b"\x1b[2~a"), vec![k(Key::Char('a'))]);
        assert_eq!(keys(b"\x1b[15~\x1bOPb"), vec![k(Key::Char('b'))]);
        assert_eq!(keys(b"\x1b[?25ha"), vec![k(Key::Char('a'))]);
    }

    #[test]
    fn f_nav_07_utf8_split_across_reads() {
        let mut d = InputDecoder::new();
        let e = "é€😀".as_bytes();
        let mut out = Vec::new();
        for c in chunks(e) {
            out.extend(d.feed(c));
        }
        assert_eq!(out, vec![k(Key::Char('é')), k(Key::Char('€')), k(Key::Char('😀'))]);
    }

    #[test]
    fn f_nav_07_csi_split_byte_by_byte() {
        let mut d = InputDecoder::new();
        let mut out = Vec::new();
        for c in chunks(b"\x1b[1;5A\x1bOB") {
            out.extend(d.feed(c));
        }
        assert_eq!(out, vec![km(Key::Up, true, false, false), k(Key::Down)]);
    }

    #[test]
    fn f_cli_05_lone_esc_at_chunk_end_is_escape() {
        let mut d = InputDecoder::new();
        assert_eq!(d.feed(b"\x1b"), vec![k(Key::Esc)]);
        assert_eq!(d.feed(b"\x1b[A"), vec![k(Key::Up)]);
        assert_eq!(d.feed(b"j\x1b"), vec![k(Key::Char('j')), k(Key::Esc)]);
        assert_eq!(d.feed(b"\x1bj"), vec![km(Key::Char('j'), false, true, false)]);
    }

    #[test]
    fn f_cli_05_paste_single_item() {
        assert_eq!(
            keys(b"\x1b[200~a\nb\x1b[201~x"),
            vec![InputItem::Paste("a\nb".into()), k(Key::Char('x'))]
        );
    }

    #[test]
    fn f_cli_05_raw_paste_chunk() {
        assert_eq!(keys(b"a\r\n\r\nb\nc"), vec![InputItem::Paste("a\r\n\r\nb\nc".into())]);
        assert_eq!(keys(b"\r"), vec![k(Key::Enter)]);
        assert_eq!(keys(b"\r\n"), vec![k(Key::Enter), k(Key::Enter)]);
    }

    #[test]
    fn f_cli_05_paste_split_chunks() {
        let mut d = InputDecoder::new();
        let all = "\x1b[200~héllo\nwörld\x1b[201~q".as_bytes();
        let mut out = Vec::new();
        for c in chunks(all) {
            out.extend(d.feed(c));
        }
        assert_eq!(out, vec![InputItem::Paste("héllo\nwörld".into()), k(Key::Char('q'))]);
    }

    #[test]
    fn unspec_8_paste_keeps_newlines() {
        assert_eq!(keys(b"\x1b[200~a\r\nb\x1b[201~"), vec![InputItem::Paste("a\r\nb".into())]);
    }

    type Row = (&'static str, Vec<u8>, InputItem);

    /// Byte-level table: what common terminals (xterm, Terminal.app, iTerm2, Ghostty, kitty, WezTerm, Alacritty,
    /// tmux; default modes plus CSI-u / modifyOtherKeys) send for every key the spec binds, and the key the core
    /// expects.
    fn table() -> Vec<Row> {
        let mut t: Vec<Row> = Vec::new();
        for c in 0x20u8..=0x7e {
            t.push(("printable", vec![c], InputItem::Key(KeyEvent::ch(c as char))));
        }
        // Shifted letters the spec binds, also as CSI-u (kitty/Ghostty/WezTerm) and modifyOtherKeys (tmux).
        for c in "FJKMCEN".chars() {
            let lower = c.to_ascii_lowercase() as u32;
            let want = InputItem::Key(KeyEvent::ch(c));
            t.push(("csi-u shift letter", format!("\x1b[{lower};2u").into_bytes(), want.clone()));
            t.push(("csi-u shift press", format!("\x1b[{lower};2:1u").into_bytes(), want.clone()));
            t.push(("modifyOtherKeys shift", format!("\x1b[27;2;{}~", c as u32).into_bytes(), want));
        }
        for c in "/:?][)($^0".chars() {
            t.push((
                "csi-u symbol",
                format!("\x1b[{}u", c as u32).into_bytes(),
                InputItem::Key(KeyEvent::ch(c)),
            ));
        }
        t.push(("csi-u ctrl+f", b"\x1b[102;5u".to_vec(), InputItem::Key(KeyEvent::ctrl('f'))));
        t.push(("csi-u alt+x", b"\x1b[120;3u".to_vec(), km(Key::Char('x'), false, true, false)));
        t.push(("csi-u enter", b"\x1b[13u".to_vec(), k(Key::Enter)));
        t.push(("csi-u esc", b"\x1b[27u".to_vec(), k(Key::Esc)));
        t.push(("csi-u tab", b"\x1b[9u".to_vec(), k(Key::Tab)));
        t.push(("csi-u backtab", b"\x1b[9;2u".to_vec(), k(Key::BackTab)));
        t.push(("csi-u backspace", b"\x1b[127u".to_vec(), k(Key::Backspace)));
        t.push(("mok enter", b"\x1b[27;1;13~".to_vec(), k(Key::Enter)));
        t.push(("mok ctrl+n", b"\x1b[27;5;110~".to_vec(), InputItem::Key(KeyEvent::ctrl('n'))));
        let plain: [(&'static str, &[u8], InputItem); 29] = [
            ("enter CR", b"\r", k(Key::Enter)),
            ("enter LF", b"\n", k(Key::Enter)),
            ("tab", b"\t", k(Key::Tab)),
            ("backtab", b"\x1b[Z", k(Key::BackTab)),
            ("backspace DEL", b"\x7f", k(Key::Backspace)),
            ("backspace BS", b"\x08", k(Key::Backspace)),
            ("delete", b"\x1b[3~", k(Key::Delete)),
            ("up", b"\x1b[A", k(Key::Up)),
            ("down", b"\x1b[B", k(Key::Down)),
            ("right", b"\x1b[C", k(Key::Right)),
            ("left", b"\x1b[D", k(Key::Left)),
            ("up ss3", b"\x1bOA", k(Key::Up)),
            ("down ss3", b"\x1bOB", k(Key::Down)),
            ("right ss3", b"\x1bOC", k(Key::Right)),
            ("left ss3", b"\x1bOD", k(Key::Left)),
            ("pgup", b"\x1b[5~", k(Key::PageUp)),
            ("pgdn", b"\x1b[6~", k(Key::PageDown)),
            ("home csi", b"\x1b[H", k(Key::Home)),
            ("home 1~", b"\x1b[1~", k(Key::Home)),
            ("home 7~", b"\x1b[7~", k(Key::Home)),
            ("home ss3", b"\x1bOH", k(Key::Home)),
            ("end csi", b"\x1b[F", k(Key::End)),
            ("end 4~", b"\x1b[4~", k(Key::End)),
            ("end 8~", b"\x1b[8~", k(Key::End)),
            ("end ss3", b"\x1bOF", k(Key::End)),
            ("ctrl+up", b"\x1b[1;5A", km(Key::Up, true, false, false)),
            ("space", b" ", k(Key::Char(' '))),
            ("alt+f", b"\x1bf", km(Key::Char('f'), false, true, false)),
            ("alt+F", b"\x1bF", km(Key::Char('F'), false, true, false)),
        ];
        for (name, bytes, key) in plain {
            t.push((name, bytes.to_vec(), key));
        }
        for c in b'a'..=b'z' {
            if matches!(c, b'i' | b'm' | b'h' | b'j') {
                continue; // Tab, Enter, Backspace, LF have their own keys
            }
            t.push(("ctrl letter", vec![c - b'a' + 1], InputItem::Key(KeyEvent::ctrl(c as char))));
        }
        t
    }

    fn is_newline(b: &[u8]) -> bool {
        b == b"\r" || b == b"\n" || b == b"\x1b[13u" || b == b"\x1b[27;1;13~"
    }

    #[test]
    fn keys_table_single_read() {
        for (name, bytes, want) in table() {
            assert_eq!(keys(&bytes), vec![want], "{name}: {bytes:?}");
        }
    }

    #[test]
    fn keys_table_lone_esc() {
        assert_eq!(keys(b"\x1b"), vec![k(Key::Esc)]);
        assert_eq!(keys(b"\x1b\x1b"), vec![k(Key::Esc), k(Key::Esc)]);
    }

    #[test]
    fn keys_table_byte_by_byte() {
        for (name, bytes, want) in table() {
            let mut d = InputDecoder::new();
            let mut out = Vec::new();
            for c in chunks(&bytes) {
                out.extend(d.feed(c));
            }
            assert_eq!(out, vec![want], "{name}: {bytes:?}");
        }
    }

    #[test]
    fn keys_split_after_esc_is_escape_then_text() {
        // Known rare edge case: a read that ends right after ESC is the Escape key.
        let mut d = InputDecoder::new();
        assert_eq!(d.feed(b"\x1b"), vec![k(Key::Esc)]);
        assert_eq!(d.feed(b"[A"), vec![k(Key::Char('[')), k(Key::Char('A'))]);
    }

    /// One byte per read, except an ESC travels with the byte after it (a read ending in ESC is Escape).
    fn chunks(b: &[u8]) -> Vec<&[u8]> {
        let (mut out, mut i) = (Vec::new(), 0);
        while i < b.len() {
            let n = if b[i] == 0x1b && i + 1 < b.len() { 2 } else { 1 };
            out.push(&b[i..i + n]);
            i += n;
        }
        out
    }

    #[test]
    fn keys_table_all_in_one_read() {
        let (mut bytes, mut want) = (Vec::new(), Vec::new());
        for (_, b, w) in table() {
            // CR/LF mixed with text in one read is a paste (Ink rule); keep those out of the concatenation.
            if !is_newline(&b) {
                bytes.extend_from_slice(&b);
                want.push(w);
            }
        }
        assert_eq!(keys(&bytes), want);
    }

    #[test]
    fn keys_table_each_key_then_esc_in_one_read() {
        for (name, mut bytes, w) in table() {
            if is_newline(&bytes) {
                continue;
            }
            bytes.push(0x1b);
            assert_eq!(keys(&bytes), vec![w, k(Key::Esc)], "{name}");
        }
    }

    #[test]
    fn keys_bracketed_paste_variants() {
        assert_eq!(keys(b"\x1b[200~F/x\x1b[201~"), vec![InputItem::Paste("F/x".into())]);
        assert_eq!(keys(b"\x1b[200~a\nb\x1b[201~\x1b"), vec![InputItem::Paste("a\nb".into()), k(Key::Esc)]);
    }
}
