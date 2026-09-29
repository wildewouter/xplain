//! Comment / thread / answer data model shared by UI, ask flow, export and MCP hub.
//!
//! Spec: F-COMMENT-03 (record), F-ASK-01/05/09 (turns, answers), F-COMMENT-10 (agent notes), F-EXPORT-02.
//! Owner: core lead (comments component). Types frozen at skeleton so mcp/export/view workers agree.
//! Must not: reference rendering or MCP transport types.
//! Component `comments` (D) also owns the functions below (creation, anchoring, lookup, deletion).

use crate::state::State;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaneSide {
    Old,
    New,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnswerStatus {
    Pending,
    Streaming,
    Done,
    /// No black-box trigger (UNSPEC-40); may be dropped.
    Error,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    pub status: AnswerStatus,
    pub text: String,
    /// MCP client name from `initialize`, if known (F-ASK-05).
    pub agent: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Turn {
    pub message: String,
    pub answer: Option<Answer>,
    /// Earlier answers on this turn, oldest first (second answer on a done turn, F-ASK-09).
    pub prior: Vec<Answer>,
}

/// Selection recorded with a comment (1-based lines and inclusive cols, F-COMMENT-03).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionInfo {
    pub start_line: Option<u32>,
    pub end_line: Option<u32>,
    pub start_col: u32,
    pub end_col: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    Human,
    /// Added by the agent via `annotate`.
    Agent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment {
    /// `q1`, `q2`, ... creation order; agent notes take ids too (F-ASK-01).
    pub id: String,
    pub file: String,
    /// Anchor row index (used when no line number).
    pub row: usize,
    pub side: PaneSide,
    pub line: Option<u32>,
    /// Anchored to a deleted line (unified del row); see F-COMMENT-05.
    pub deleted: bool,
    /// Line text or selected text.
    pub text: String,
    pub message: String,
    pub selection: Option<SelectionInfo>,
    /// Code rows around the anchor: +-3 rows / +-15 rows.
    pub context: Vec<String>,
    pub wide: Vec<String>,
    pub turns: Vec<Turn>,
    pub origin: Origin,
    /// Order label for agent notes; `(`/`)` jump (F-COMMENT-09).
    pub number: Option<u32>,
    /// Creation sequence for ordering ties.
    pub seq: u64,
}

/// Next free id `q<n>` (uses `state.thread.next_id`); also used by agent notes (F-ASK-01).
pub fn alloc_id(_state: &mut State) -> String {
    todo!("F-ASK-01 ids")
}

/// Build a comment anchored at the current cursor/selection with `message` (F-COMMENT-03, F-COMMENT-05,
/// F-VISUAL-03): line/side/deleted flags, selection info, text, `context` (+-3 rows) and `wide` (+-15 rows).
/// Does not insert it.
pub fn from_cursor(_state: &mut State, _message: &str) -> Comment {
    todo!("F-COMMENT-03/05")
}

/// Insert keeping `seq` order; returns its id.
pub fn insert(_state: &mut State, _comment: Comment) -> String {
    todo!("F-COMMENT-03")
}

pub fn find<'a>(_state: &'a State, _id: &str) -> Option<&'a Comment> {
    todo!("lookup")
}

/// Remove (F-COMMENT-08); clears focus when it was focused.
pub fn remove(_state: &mut State, _id: &str) {
    todo!("F-COMMENT-08")
}

/// Comments of the current file shown under row `row` (anchor rules F-COMMENT-05: by row index + side,
/// or by line number / nearest after reload), in creation order.
pub fn anchored_at(_state: &State, _row: usize) -> Vec<&Comment> {
    todo!("F-COMMENT-05")
}

/// Ids of the current file's comments sorted by anchor row then seq (J/K order, F-COMMENT-06).
pub fn ids_in_file(_state: &State) -> Vec<String> {
    todo!("F-COMMENT-06")
}
