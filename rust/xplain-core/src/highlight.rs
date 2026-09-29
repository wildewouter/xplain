//! Syntax highlighting: text line -> colored spans, by path or fence language, per theme.
//!
//! Spec: UNSPEC-31 (syntax colors are free but must not alter text), F-EDGE-08, F-ASK-06 (fence languages).
//! Oracle: `src/highlight.ts` (lang table, fence aliases). Owner: component `viewrows` (F2).
//! Uses syntect with the pure-Rust regex backend (feature `default-fancy`); syntax set loaded lazily in a
//! `OnceLock`. Must not: change the text, panic on odd input (fall back to plain), or touch state.

use crate::screen::Color;
use crate::theme::ThemeId;

/// One highlighted span; concatenated `text` equals the input line exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HlSpan {
    pub text: String,
    pub fg: Option<Color>,
    pub bold: bool,
    pub italic: bool,
}

/// Language id for a file path by extension (same table as `langs` in highlight.ts), `None` = plain.
pub fn language_for_path(_path: &str) -> Option<&'static str> {
    todo!("lang table")
}

/// Language id for a markdown fence info string (aliases like `shell`, `golang`), `None` when unknown.
pub fn language_for_fence(_name: &str) -> Option<&'static str> {
    todo!("fence table")
}

/// Highlight one line. Blank input or unknown language -> one plain span.
pub fn highlight_line(_text: &str, _lang: Option<&str>, _theme: ThemeId) -> Vec<HlSpan> {
    todo!("syntect")
}
