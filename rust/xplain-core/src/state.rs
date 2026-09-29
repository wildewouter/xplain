//! The single `State` value and its top-level parts.
//!
//! Spec: all interactive groups. Owner: core lead. This file fixes the *outline* (field names and
//! ownership); component workers own the internals of the sub-structs they are assigned and may add
//! fields, but must not rename or remove the top-level fields listed here without the core lead.
//! Must not: hold IO handles, closures over runtime objects, or terminal types.
//!
//! Runtime contract: before each `update` call the runtime sets `state.clock` (wall clock snapshot).
//! Core never reads the time otherwise.

use std::collections::HashMap;

use crate::comments::{Comment, PaneSide};
use crate::config::Config;
use crate::diff::FileDiff;
use crate::event::ReqId;
use crate::integration::{Integrations, RegStatus};
use crate::mcp::McpState;
use crate::options::{DiffMode, EnvInfo, Init, Options};
use crate::screen::Size;
use crate::theme::ThemeId;

/// Wall clock snapshot supplied by the runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Now {
    pub unix_ms: i64,
    /// Local offset from UTC at `unix_ms`, seconds (export file name uses local time, F-EXPORT-01).
    pub utc_offset_secs: i32,
}

/// Live copies of settings (config + flags + in-session changes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub theme: ThemeId,
    pub mode: DiffMode,
    pub split: bool,
    /// true = full-file scope.
    pub full: bool,
    pub confirm_quit: bool,
    pub mcp_autostart: bool,
}

impl From<&Config> for Settings {
    fn from(c: &Config) -> Self {
        Settings {
            theme: c.theme,
            mode: c.mode,
            split: c.split,
            full: c.full,
            confirm_quit: c.confirm_quit,
            mcp_autostart: c.mcp_autostart,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadState {
    /// Before the first result: `Loading...` frame.
    Loading,
    Ready,
    /// Whole-screen git error (F-MODE-04).
    Error(String),
}

/// Browse (file viewer) content, F-BROWSE-01.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Browse {
    pub path: String,
    pub lines: Vec<String>,
}

/// Char cursor + viewport (F-CURSOR-*, F-NAV-*).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Nav {
    /// Index into `State::files`.
    pub file_index: usize,
    /// Row index in the shown rows (diff rows or browse rows).
    pub row: usize,
    /// 0-based char column as desired (clamped when shown); `sticky_end` for `$`.
    pub col: usize,
    pub sticky_end: bool,
    pub pane: PaneChoice,
    /// First shown row (viewport top) and horizontal code shift (F-CURSOR-07).
    pub top: usize,
    pub x_shift: usize,
    /// Pending count prefix (F-CURSOR-05), 0 = none.
    pub count: u32,
    pub selection: Option<Selection>,
    /// Focused comment id (F-COMMENT-06).
    pub focused_comment: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PaneChoice {
    Old,
    #[default]
    New,
}

impl From<PaneChoice> for PaneSide {
    fn from(p: PaneChoice) -> Self {
        match p {
            PaneChoice::Old => PaneSide::Old,
            PaneChoice::New => PaneSide::New,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionKind {
    Char,
    Line,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub kind: SelectionKind,
    pub anchor_row: usize,
    pub anchor_col: usize,
}

/// Find (`/`) state, F-FIND-*.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FindState {
    /// Active term (highlights, `n`/`N`).
    pub term: Option<String>,
}

/// Modal / text input in front of the diff. Exactly one at a time (F-HELP-02 priority resolves overlaps
/// of *contexts*, not of these).
#[derive(Debug, Clone, PartialEq)]
pub enum Overlay {
    None,
    /// Find line open (`/text█`).
    Find {
        text: String,
    },
    /// Goto line open (`:text█`).
    Goto {
        text: String,
    },
    Editor(EditorState),
    Picker {
        sel: usize,
    },
    Search(SearchState),
    Config(ConfigModal),
    Mcp(McpModal),
    DeleteComment {
        id: String,
    },
    Quit,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EditorKind {
    New,
    Edit { id: String },
    FollowUp { id: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct EditorState {
    pub kind: EditorKind,
    pub text: String,
    /// Caret as char index.
    pub caret: usize,
    /// Tab toggles save/ask (new comments only).
    pub ask_mode: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SearchState {
    pub query: String,
    pub sel: usize,
    /// All files (loaded async; empty until done).
    pub files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigModal {
    pub row: usize,
    /// Choice cursor per row (6 rows).
    pub cursors: [usize; 6],
    /// Committed theme (preview reverts to it on close).
    pub committed_theme: ThemeId,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct McpModal {
    /// 0 = power row, 1.. = integrations in registry order.
    pub row: usize,
    /// Pending confirm: (integration index, register?).
    pub confirm: Option<(usize, bool)>,
    pub note: Option<String>,
    /// Preview of copied text: (label like `register command (copied)`, masked text).
    pub preview: Option<(String, String)>,
}

/// Per-integration UI state (registry order).
#[derive(Debug, Clone, Default)]
pub struct IntegrationState {
    pub status: RegStatus,
    pub busy: bool,
    /// Last result message (register/unregister/check), shown when row selected.
    pub message: Option<String>,
}

/// Help panel level (F-HELP-01): closed, L1, L2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HelpLevel {
    #[default]
    Closed,
    L1,
    L2,
}

/// What an outstanding `ReqId` was for (so results map back; stale ids are dropped).
#[derive(Debug, Clone, PartialEq)]
pub enum Pending {
    Diff,
    FileList,
    Browse { path: String },
    ConfigSave,
    Export { path: String, count: usize },
    Command { integration: usize, purpose: CommandPurpose },
    McpStart,
    McpStop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandPurpose {
    Check,
    /// Index of the step within `register_commands`.
    Register(usize),
    Unregister,
}

/// The whole application state.
#[derive(Debug)]
pub struct State {
    pub clock: Now,
    pub size: Size,
    pub env: EnvInfo,
    pub options: Options,
    pub settings: Settings,
    pub load: LoadState,
    /// First load finished (barrier "ready" rule, Test seams).
    pub ready: bool,
    pub files: Vec<FileDiff>,
    pub browse: Option<Browse>,
    pub nav: Nav,
    pub find: FindState,
    pub overlay: Overlay,
    pub help: HelpLevel,
    pub comments: Vec<Comment>,
    /// Footer note (F-LAYOUT-02).
    pub note: Option<String>,
    pub mcp: McpState,
    pub integrations: Integrations,
    pub integration_state: Vec<IntegrationState>,
    pub pending: HashMap<ReqId, Pending>,
    pub next_req: u64,
    /// Spinner frame index (F-ASK-05).
    pub spinner: usize,
}

impl State {
    /// Build the initial state and the initial effects (diff load). Loading frame is shown until
    /// `Event::DiffLoaded` arrives.
    pub fn new(_init: Init, _integrations: Integrations) -> (State, Vec<crate::Effect>) {
        todo!("initial state + LoadDiff effect")
    }

    /// Fresh request id.
    pub fn alloc_req(&mut self) -> ReqId {
        self.next_req += 1;
        ReqId(self.next_req)
    }

    /// Barrier rule (Test seams): first frame + initial load done.
    pub fn is_ready(&self) -> bool {
        self.ready
    }
}
