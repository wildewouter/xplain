//! Key notation: literal text is one key per char, `<Name>` tokens are special keys.
//!
//! Tokens: `<Esc> <Enter> <Tab> <S-Tab> <Up> <Down> <Left> <Right> <Home> <End> <PageUp> <PageDown> <Space> <BS>
//! <Del>`, `<C-x>` (ctrl), `<A-x>` / `<M-x>` (alt), `<lt>` for a literal `<`. Unknown tokens panic (a typo must
//! never become a vacuous test).

use xplain_core::keys::{Key, KeyEvent, Mods};

/// Parse `spec` into key events. Panics on an unknown or unterminated token.
pub fn parse_keys(spec: &str) -> Vec<KeyEvent> {
    let mut out = Vec::new();
    let mut rest = spec;
    while let Some(c) = rest.chars().next() {
        if c == '<' {
            let Some(end) = rest.find('>') else { panic!("keys {spec:?}: unterminated `<` (use <lt>)") };
            out.push(token(&rest[1..end], spec));
            rest = &rest[end + 1..];
        } else {
            out.push(match c {
                '\n' | '\r' => KeyEvent::plain(Key::Enter),
                '\t' => KeyEvent::plain(Key::Tab),
                c => KeyEvent::ch(c),
            });
            rest = &rest[c.len_utf8()..];
        }
    }
    out
}

fn named(name: &str) -> Option<Key> {
    Some(match name {
        "Esc" => Key::Esc,
        "Enter" => Key::Enter,
        "Tab" => Key::Tab,
        "Up" => Key::Up,
        "Down" => Key::Down,
        "Left" => Key::Left,
        "Right" => Key::Right,
        "Home" => Key::Home,
        "End" => Key::End,
        "PageUp" => Key::PageUp,
        "PageDown" => Key::PageDown,
        "Space" => Key::Char(' '),
        "BS" => Key::Backspace,
        "Del" => Key::Delete,
        "lt" => Key::Char('<'),
        _ => return None,
    })
}

fn token(tok: &str, spec: &str) -> KeyEvent {
    if tok == "S-Tab" {
        return KeyEvent::plain(Key::BackTab);
    }
    let mut mods = Mods::default();
    let mut base = tok;
    loop {
        let Some((m, rest)) = base.split_once('-') else { break };
        if rest.is_empty() {
            break;
        }
        match m {
            "C" => mods.ctrl = true,
            "A" | "M" => mods.alt = true,
            "S" => mods.shift = true,
            _ => break,
        }
        base = rest;
    }
    let key = named(base).or_else(|| {
        let mut cs = base.chars();
        match (cs.next(), cs.next()) {
            (Some(c), None) => Some(Key::Char(if mods.ctrl { c.to_ascii_lowercase() } else { c })),
            _ => None,
        }
    });
    let Some(key) = key else { panic!("keys {spec:?}: unknown token <{tok}>") };
    if matches!(key, Key::Char(_)) {
        mods.shift = false;
    }
    KeyEvent { key, mods }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literals_and_tokens() {
        let k = parse_keys("j<Esc><C-f><Tab><S-Tab><A-x><lt>X 1");
        let want = [
            KeyEvent::ch('j'),
            KeyEvent::plain(Key::Esc),
            KeyEvent::ctrl('f'),
            KeyEvent::plain(Key::Tab),
            KeyEvent::plain(Key::BackTab),
            KeyEvent { key: Key::Char('x'), mods: Mods { alt: true, ..Mods::default() } },
            KeyEvent::ch('<'),
            KeyEvent::ch('X'),
            KeyEvent::ch(' '),
            KeyEvent::ch('1'),
        ];
        assert_eq!(k, want);
    }

    #[test]
    #[should_panic(expected = "unknown token")]
    fn unknown_token() {
        parse_keys("<Nope>");
    }

    #[test]
    #[should_panic(expected = "unterminated")]
    fn bare_lt() {
        parse_keys("a<b");
    }
}
