//! Comment / thread / answer data model shared by UI, ask flow, export and MCP hub.
//!
//! Spec: F-COMMENT-03 (record), F-ASK-01/05/09 (turns, answers), F-COMMENT-10 (agent notes), F-EXPORT-02.
//! Owner: core lead (comments component). Types frozen at skeleton so mcp/export/view workers agree.
//! Must not: reference rendering or MCP transport types.

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
