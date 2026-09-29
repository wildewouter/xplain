//! Layout of comment boxes and the editor box under code rows, as toolkit-neutral styled lines.
//!
//! Spec: F-COMMENT-04 (box render), F-COMMENT-10 (agent note), F-ASK-05 (thread body, status, spinner text),
//! F-ASK-06 (code blocks), F-ASK-07 (thread scroll window, `… +N more`), F-ASK-08 (copy buttons),
//! F-ASK-09 (answer arrival display), F-NAV-09 (heights for viewport). Oracle: `src/components/answerView.ts`
//! (wrapText, answerView, richLines, threadBody, windowBody) and `src/components/AskBox.tsx` (askH, sentH).
//! Owner: component `comments` (D). Pure: state -> lines. `view::thread_box` (component `viewrows`) only maps
//! tones to theme colors and draws; `nav::viewport` only uses the heights.
//! Must not: know theme colors or the canvas.
//!
//! Tone conventions for the view (`view::thread_box`):
//! - `Border`: dim border or gutter glyph; in a `focused` box: accent + bold. The editor box (empty `id`) uses
//!   the modal border color and modal background/foreground.
//! - `Bold`: accent + bold (focused head). `Accent`: accent. `Dim`: dim. `Error`: the deletions color.
//! - `Button`: accent; `ButtonSelected`: accent, reverse, bold. `Code`: code text, highlighted by `lang`.
//! - `Caret`: inverse cell. Spinner glyphs are already resolved from `state.spinner`.
//!
//! Every line is exactly `width` chars wide (borders included), padded with spaces.

mod body;
mod boxes;
mod text;
mod window;

pub use body::*;
pub use boxes::*;
pub use text::*;
pub use window::*;

use crate::state::State;

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

/// Editor rows without the selection preview: borders, input, hint.
pub const ASK_H: usize = 4;
/// Selected lines shown in the editor before `… +N more` (F-COMMENT-01).
pub const ASK_MAX: usize = 5;
/// Quoted selection lines shown in a comment box (F-COMMENT-04).
pub const SENT_MAX: usize = 3;
/// Unfocused body lines before `… +N more` (F-COMMENT-04).
pub const BODY_CAP: usize = 14;
pub const COPY_BTN: &str = "[ copy ]";
/// `│ ` before each code line.
pub const CODE_GUTTER: usize = 2;
pub(super) const SPINNER: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// Box width: max(10, cols-1) (F-COMMENT-01/04).
pub fn box_width(state: &State) -> usize {
    (state.size.cols as usize).saturating_sub(1).max(10)
}

#[cfg(test)]
pub(super) mod fixtures {
    use super::*;
    use crate::comments::testutil::{ctx, state_with};

    pub fn text(l: &BoxLine) -> String {
        l.spans.iter().map(|s| s.text.as_str()).collect()
    }

    pub fn texts(b: &ThreadBox) -> Vec<String> {
        b.lines.iter().map(text).collect()
    }

    pub fn st() -> State {
        state_with((1..=5).map(ctx).collect())
    }
}
