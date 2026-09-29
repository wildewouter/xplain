//! Small text helpers shared by nav, view and layout code.
//!
//! Spec: F-EDGE-08 (content chars, tabs), UNSPEC-28 (column unit = chars),
//! F-CURSOR-04 (char classes). Owner: component `nav` (B).
//! Must not: depend on state or terminal types.

use unicode_width::UnicodeWidthChar;

/// Tabs to 2 spaces (F-LAYOUT-03). Other chars untouched.
pub fn expand_tabs(s: &str) -> String {
    s.replace('\t', "  ")
}

/// Display width of one char in cells (control and zero-width chars 0). The single width source.
pub fn char_width(c: char) -> usize {
    c.width().unwrap_or(0)
}

/// Display width in terminal cells (unicode-width; control chars 0).
pub fn cell_width(s: &str) -> usize {
    s.chars().map(char_width).sum()
}

/// Word class for `w`/`b`/`e` (F-CURSOR-04): 0 = blank/none, 1 = word char (`[A-Za-z0-9_]`), 2 = other.
pub fn char_class(c: Option<char>) -> u8 {
    match c {
        None => 0,
        Some(c) if c.is_whitespace() => 0,
        Some(c) if c.is_ascii_alphanumeric() || c == '_' => 1,
        Some(_) => 2,
    }
}

/// Wrap `text` to `width` cells (words, hard-break long words), keeping blank lines (`wrapText`).
/// Tabs become 2 spaces, CR removed, trailing blank lines dropped (keeping at least one line).
pub fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let w = width.max(1);
    let clean = text.replace('\r', "").replace('\t', "  ");
    let mut out: Vec<String> = Vec::new();
    for raw in clean.split('\n') {
        let mut l: Vec<char> = raw.chars().collect();
        if l.is_empty() {
            out.push(String::new());
            continue;
        }
        while l.len() > w {
            // lastIndexOf(' ', w): last space at index <= w
            let mut k = (0..=w).rev().find(|&i| l.get(i) == Some(&' ')).unwrap_or(0);
            if k == 0 {
                k = w;
            }
            let head: String = l[..k].iter().collect();
            out.push(head.trim_end().to_string());
            let rest: String = l[k..].iter().collect();
            l = rest.trim_start().chars().collect();
        }
        out.push(l.into_iter().collect());
    }
    while out.len() > 1 && out.last().is_some_and(String::is_empty) {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f_layout_03_tabs_expand_to_two_spaces() {
        assert_eq!(expand_tabs("\ta\tb"), "  a  b");
        assert_eq!(expand_tabs("plain"), "plain");
    }

    #[test]
    fn f_edge_08_cell_width_wide_and_control() {
        assert_eq!(cell_width("abc"), 3);
        assert_eq!(cell_width("日本"), 4);
        assert_eq!(cell_width("a\u{7}b"), 2);
        assert_eq!(cell_width(""), 0);
    }

    #[test]
    fn f_cursor_04_char_classes() {
        assert_eq!(char_class(None), 0);
        assert_eq!(char_class(Some(' ')), 0);
        assert_eq!(char_class(Some('\t')), 0);
        assert_eq!(char_class(Some('a')), 1);
        assert_eq!(char_class(Some('_')), 1);
        assert_eq!(char_class(Some('7')), 1);
        assert_eq!(char_class(Some('-')), 2);
        assert_eq!(char_class(Some('é')), 2);
    }

    #[test]
    fn f_ask_05_wrap_text_golden() {
        assert_eq!(wrap_text("hello world foo", 11), ["hello world", "foo"]);
        assert_eq!(wrap_text("abcdefghij", 4), ["abcd", "efgh", "ij"]);
        assert_eq!(wrap_text("a\n\n\nb\n\n", 5), ["a", "", "", "b"]);
        assert_eq!(wrap_text("a\tb", 10), ["a  b"]);
        assert_eq!(wrap_text("aaaa bbbb", 4), ["aaaa", "bbbb"]);
        assert_eq!(wrap_text("x\r\ny", 10), ["x", "y"]);
        assert_eq!(wrap_text("aaa   bbb", 4), ["aaa", "bbb"], "trailing/leading spaces trimmed at the break");
        assert_eq!(wrap_text("", 5), [""]);
        assert_eq!(wrap_text("\n\n", 5), [""]);
        assert_eq!(wrap_text("ab cd", 0), ["a", "b", "c", "d"], "width clamps to 1");
    }

    #[test]
    fn wrap_text_rules() {
        assert_eq!(wrap_text("aa bb cc", 5), vec!["aa bb", "cc"]);
        assert_eq!(wrap_text("abcdefgh", 3), vec!["abc", "def", "gh"]);
        assert_eq!(wrap_text("a\n\nb\n\n", 5), vec!["a", "", "b"]);
        assert_eq!(wrap_text("a\tb", 10), vec!["a  b"]);
        assert_eq!(wrap_text("aaa   bbb", 5), vec!["aaa", "bbb"]);
    }
}
