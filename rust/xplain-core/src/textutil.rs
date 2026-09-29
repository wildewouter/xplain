//! Small text helpers shared by nav, view and layout code.
//!
//! Spec: F-EDGE-08 (content chars, tabs), F-LAYOUT-07 (truncation), UNSPEC-28 (column unit = chars),
//! F-CURSOR-04 (char classes). Owner: component `nav` (B).
//! Must not: depend on state or terminal types.

use unicode_width::UnicodeWidthChar;

/// Tabs to 2 spaces (F-LAYOUT-03). Other chars untouched.
pub fn expand_tabs(s: &str) -> String {
    s.replace('\t', "  ")
}

fn char_width(c: char) -> usize {
    c.width().unwrap_or(0)
}

/// Display width in terminal cells (unicode-width; control chars 0).
pub fn cell_width(s: &str) -> usize {
    s.chars().map(char_width).sum()
}

/// Cut `s` to `width` cells; when it does not fit, `width-1` cells plus `…` (F-LAYOUT-07). `width == 0` -> "".
pub fn truncate_ellipsis(s: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if cell_width(s) <= width {
        return s.to_string();
    }
    let budget = width - 1;
    let mut used = 0;
    let mut out = String::new();
    for c in s.chars() {
        let w = char_width(c);
        if used + w > budget {
            break;
        }
        used += w;
        out.push(c);
    }
    out.push('…');
    out
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
    fn f_layout_07_truncate_with_ellipsis() {
        assert_eq!(truncate_ellipsis("hello", 5), "hello");
        assert_eq!(truncate_ellipsis("hello!", 5), "hell…");
        assert_eq!(truncate_ellipsis("hello", 0), "");
        assert_eq!(truncate_ellipsis("hello", 1), "…");
        assert_eq!(truncate_ellipsis("日本語", 4), "日…");
        assert_eq!(truncate_ellipsis("", 3), "");
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
}
