//! Layout of comment boxes and the editor box under code rows, as toolkit-neutral styled lines.
//!
//! Spec: F-COMMENT-04 (box render), F-COMMENT-10 (agent note), F-ASK-05 (thread body, status, spinner text),
//! F-ASK-06 (code blocks), F-ASK-07 (thread scroll window, `… +N more`), F-ASK-08 (copy buttons),
//! F-ASK-09 (answer arrival display), F-NAV-09 (heights for viewport). Oracle: `src/components/answerView.ts`
//! (wrapText, answerView, richLines, threadBody, windowBody) and `src/components/AskBox.tsx` (askH, sentH).
//! Owner: component `comments` (D). Pure: state -> lines. `view::thread_box` (component `viewrows`) only maps
//! tones to theme colors and draws; `nav::viewport` only uses the heights.
//! Must not: know theme colors or the canvas.

use crate::comments::Comment;
use crate::state::{EditorState, State};

/// Semantic style of a span; the view maps it to theme colors/attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Normal,
    Dim,
    Bold,
    Accent,
    Error,
    /// Code block content (syntax highlighted when `Span::lang` is set).
    Code,
    Button,
    ButtonSelected,
    /// Editor caret cell (drawn inverse).
    Caret,
    /// Box border / gutter glyphs.
    Border,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub tone: Tone,
    /// Fence language for `Tone::Code` (highlight.rs `language_for_fence`).
    pub lang: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BoxLine {
    pub spans: Vec<Span>,
}

/// One rendered box (comment thread or editor) with its exact lines, already truncated/wrapped to `width`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ThreadBox {
    /// Comment id, or empty for the editor box.
    pub id: String,
    pub focused: bool,
    pub lines: Vec<BoxLine>,
}

/// Wrap `text` to `width` cells (words, hard-break long words), keeping blank lines (`wrapText`).
pub fn wrap_text(_text: &str, _width: usize) -> Vec<String> {
    todo!("wrapText")
}

/// Box of one comment at `width` (F-COMMENT-04, F-ASK-05..08, F-COMMENT-10). Uses thread scroll from
/// `state.thread` when focused.
pub fn comment_box(_state: &State, _comment: &Comment, _width: usize, _focused: bool) -> ThreadBox {
    todo!("F-COMMENT-04, F-ASK-05..08")
}

/// Editor box for the open editor (F-COMMENT-01/02 render: prompt, text, caret, hint line).
pub fn editor_box(_state: &State, _editor: &EditorState, _width: usize) -> ThreadBox {
    todo!("F-COMMENT-01/02")
}

/// All boxes drawn under row `row` of the current view: comment boxes in creation order, then the editor
/// box when it is anchored here. Width = `state.size.cols`.
pub fn boxes_at(_state: &State, _row: usize) -> Vec<ThreadBox> {
    todo!("F-COMMENT-04/05")
}

/// Sum of box line counts under row `row` (0 when none). Used by `nav::viewport` (F-NAV-09).
pub fn row_extra_height(_state: &State, _row: usize) -> usize {
    todo!("F-NAV-09")
}

/// Number of copy-button code blocks in a comment's thread (F-ASK-08).
pub fn block_count(_state: &State, _comment: &Comment) -> usize {
    todo!("F-ASK-08")
}

/// Text of code block `index` in the thread of `comment` (copied by the button).
pub fn block_text(_comment: &Comment, _index: usize) -> Option<String> {
    todo!("F-ASK-08")
}
