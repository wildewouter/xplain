//! Text helpers shared by the MCP hub, JSON-RPC layer and tool calls: agent-text sanitizing and char caps.
//!
//! Spec: F-MCPSRV-07/08 (strip ANSI escapes and control chars, cap 20000 chars). Owner: component `agent`
//! (E). Must not: know queues, tools or HTTP.

/// Max chars of agent text (F-MCPSRV-07).
pub const MAX_TEXT: usize = 20000;

/// First `n` chars.
pub fn cap_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// Strip ANSI escapes and control chars (keep `\n`, `\t`), cap 20000 chars (F-MCPSRV-07/08).
pub fn sanitize(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\u{1b}' {
            i += esc_len(&chars[i..]);
            continue;
        }
        let cp = c as u32;
        let ctrl = matches!(cp, 0x00..=0x08 | 0x0b..=0x1f | 0x7f..=0x9f);
        if !ctrl {
            out.push(c);
        }
        i += 1;
    }
    cap_chars(&out, MAX_TEXT)
}

/// Length of the escape sequence starting at `c[0] == ESC` (OSC, CSI, two-char escape, lone ESC).
fn esc_len(c: &[char]) -> usize {
    match c.get(1) {
        Some(']') => {
            // OSC: body up to BEL or ESC \ ; otherwise ESC ] alone (two-char escape)
            let mut j = 2;
            while j < c.len() && c[j] != '\u{7}' && c[j] != '\u{1b}' {
                j += 1;
            }
            match (c.get(j), c.get(j + 1)) {
                (Some('\u{7}'), _) => j + 1,
                (Some('\u{1b}'), Some('\\')) => j + 2,
                _ => 2,
            }
        }
        Some('[') => {
            let mut j = 2;
            while j < c.len() && ('\u{30}'..='\u{3f}').contains(&c[j]) {
                j += 1;
            }
            while j < c.len() && ('\u{20}'..='\u{2f}').contains(&c[j]) {
                j += 1;
            }
            if j < c.len() && ('\u{40}'..='\u{7e}').contains(&c[j]) { j + 1 } else { 1 }
        }
        Some(x) if ('@'..='Z').contains(x) || ('\\'..='_').contains(x) => 2,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f_mcpsrv_07_sanitize() {
        assert_eq!(sanitize("a\u{1b}[31mred\u{1b}[0m\tb\nc\rd\u{7}e"), "ared\tb\ncde");
        assert_eq!(sanitize("x\u{1b}]0;title\u{7}y"), "xy");
        assert_eq!(sanitize("x\u{1b}]0;title\u{1b}\\y"), "xy");
        assert_eq!(sanitize("x\u{1b}]open"), "xopen");
        assert_eq!(sanitize("x\u{1b}[31"), "x[31");
        assert_eq!(sanitize("a\u{1b}Mb"), "ab");
        assert_eq!(sanitize("a\u{1b}"), "a");
        assert_eq!(sanitize("a\u{85}\u{9f}b\u{a0}"), "ab\u{a0}");
        assert_eq!(sanitize(&"x".repeat(25000)).len(), 20000);
    }
}
