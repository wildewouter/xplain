//! Small text helpers shared by nav, view and layout code.
//!
//! Spec: F-EDGE-08 (content chars, tabs), F-LAYOUT-07 (truncation), UNSPEC-28 (column unit = chars),
//! F-CURSOR-04 (char classes). Owner: component `nav` (B).
//! Must not: depend on state or terminal types.

/// Tabs to 2 spaces (F-LAYOUT-03). Other chars untouched.
pub fn expand_tabs(_s: &str) -> String {
    todo!("F-LAYOUT-03 tabs")
}

/// Display width in terminal cells (unicode-width; control chars 0).
pub fn cell_width(_s: &str) -> usize {
    todo!("unicode-width")
}

/// Cut `s` to `width` cells; when it does not fit, `width-1` cells plus `…` (F-LAYOUT-07). `width == 0` -> "".
pub fn truncate_ellipsis(_s: &str, _width: usize) -> String {
    todo!("F-LAYOUT-07")
}

/// Word class for `w`/`b`/`e` (F-CURSOR-04): 0 = blank/none, 1 = word char (`[A-Za-z0-9_]`), 2 = other.
pub fn char_class(_c: Option<char>) -> u8 {
    todo!("F-CURSOR-04")
}
