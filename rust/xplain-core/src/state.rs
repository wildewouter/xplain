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
use crate::effect::Fx;
use crate::event::ReqId;
use crate::integration::{Integrations, RegStatus};
use crate::keys::KeyEvent;
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

impl Settings {
    /// Config values overridden by CLI flags (flags win, F-CLI-03).
    pub fn resolve(c: &Config, o: &Options) -> Self {
        Settings {
            theme: o.theme.unwrap_or(c.theme),
            mode: o.mode.unwrap_or(c.mode),
            split: o.split.unwrap_or(c.split),
            full: o.full.unwrap_or(c.full),
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

impl Overlay {
    /// Route `key` to the handler of the open overlay. Returns false when no overlay is open.
    pub fn route_key(state: &mut State, key: KeyEvent, fx: &mut Fx) -> bool {
        use crate::{config_ui, editor, find, mcp_ui, picker, quit, search, thread};
        match state.overlay {
            Overlay::None => return false,
            Overlay::Editor(_) => editor::on_key(state, key, fx),
            Overlay::Find { .. } => find::on_find_key(state, key, fx),
            Overlay::Goto { .. } => find::on_goto_key(state, key, fx),
            Overlay::DeleteComment { .. } => thread::on_delete_dialog_key(state, key, fx),
            Overlay::Quit => quit::on_key(state, key, fx),
            Overlay::Search(_) => search::on_key(state, key, fx),
            Overlay::Mcp(_) => mcp_ui::on_key(state, key, fx),
            Overlay::Config(_) => config_ui::on_key(state, key, fx),
            Overlay::Picker { .. } => picker::on_key(state, key, fx),
        }
        true
    }
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
    /// Private state of the search component.
    pub ext: crate::search::SearchExt,
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
    /// The next check result keeps `message` (check that follows register/unregister, F-INTEG-02).
    pub keep_note: bool,
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
    Browse {
        path: String,
    },
    /// Re-read of the browsed file on `r` (errors ignored, cursor kept).
    BrowseReload {
        path: String,
    },
    ConfigSave,
    Export {
        path: String,
        count: usize,
    },
    Command {
        integration: usize,
        purpose: CommandPurpose,
    },
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
    /// Outstanding async requests and the diff-load bookkeeping.
    pub loader: Loader,
    /// Spinner frame index (F-ASK-05).
    pub spinner: usize,
    /// Bumped whenever `files` or `browse` content is replaced (keys the rows cache).
    pub files_gen: u64,
    /// Rows of the shown file / browse view, rebuilt by `rows::ensure` (B).
    pub rows: crate::rows::Rows,
    /// Ask flow bookkeeping (E).
    pub ask: crate::ask::AskState,
    /// Thread UI state and comment id counter (D).
    pub thread: crate::thread::ThreadUi,
    /// File picker scroll (C).
    pub picker: crate::picker::PickerUi,
    /// MCP modal/autostart bookkeeping that outlives a single overlay.
    pub mcp_ui: McpUi,
    /// Syntax highlight cache, filled off the render path (`hlcache`).
    pub hl: crate::hlcache::HlCache,
}

/// Async request bookkeeping: what each outstanding `ReqId` is for, plus the diff-load parking slots.
#[derive(Debug, Default)]
pub struct Loader {
    pub pending: HashMap<ReqId, Pending>,
    pub next_req: u64,
    /// Latest diff load request; results of older ones are dropped.
    pub diff_req: Option<ReqId>,
    /// Why the latest diff load was asked.
    pub diff_kind: DiffLoadKind,
    /// Cursor/file action to apply when the next diff load lands (mode/scope change).
    pub diff_nav: Option<DiffNav>,
}

/// MCP UI state that must survive the modal closing.
#[derive(Debug, Default)]
pub struct McpUi {
    /// Token of the last MCP endpoint, for masking after the server stopped.
    pub last_token: String,
    /// Start request issued by autostart; its failure becomes a note.
    pub autostart_req: Option<ReqId>,
}

/// Why a diff load was requested: decides error handling (F-MODE-04, F-RELOAD-01/02).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiffLoadKind {
    /// First load, mode or scope change: errors replace the screen.
    #[default]
    Effect,
    /// `r`: note `reloaded`, errors become a one-line note.
    Manual,
    /// Agent `files_changed`: silent, errors ignored.
    Agent,
}

/// What to do with file index and cursor when a load requested by `m`/`c` arrives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffNav {
    /// Mode change: file index 0; cursor kept when the first file has the remembered path (F-MODE-03).
    Mode(crate::jump::CursorMemo),
    /// Scope change: stay on this path, cursor reset per F-NAV-08 (F-SCOPE-02).
    Scope(Option<String>),
}

impl State {
    /// Show `text` as the status note.
    pub fn set_note(&mut self, text: impl Into<String>) {
        self.note = Some(text.into());
    }

    /// Build the initial state and the initial effects (diff load). Loading frame is shown until
    /// `Event::DiffLoaded` arrives.
    pub fn new(init: Init, integrations: Integrations) -> (State, Vec<crate::Effect>) {
        let settings = Settings::resolve(&init.config, &init.options);
        let integration_state = integrations.iter().map(|_| IntegrationState::default()).collect();
        let mut state = State {
            clock: Now::default(),
            size: init.size,
            env: init.env,
            options: init.options,
            settings,
            load: LoadState::Loading,
            ready: false,
            files: Vec::new(),
            browse: None,
            nav: Nav::default(),
            find: FindState::default(),
            overlay: Overlay::None,
            help: HelpLevel::Closed,
            comments: Vec::new(),
            note: None,
            mcp: McpState::default(),
            integrations,
            integration_state,
            loader: Loader::default(),
            spinner: 0,
            files_gen: 0,
            rows: crate::rows::Rows::default(),
            ask: crate::ask::AskState::default(),
            thread: crate::thread::ThreadUi::default(),
            picker: crate::picker::PickerUi::default(),
            mcp_ui: McpUi::default(),
            hl: crate::hlcache::HlCache::default(),
        };
        let mut fx = Vec::new();
        crate::reload::request_load(&mut state, &mut fx);
        (state, fx)
    }

    /// Path of the shown file: browse path, else the diff file at the cursor; `None` on the no-changes screen.
    pub fn current_path(&self) -> Option<&str> {
        match &self.browse {
            Some(b) => Some(b.path.as_str()),
            None => self.files.get(self.nav.file_index).map(|f| f.path.as_str()),
        }
    }

    /// Fresh request id.
    pub fn alloc_req(&mut self) -> ReqId {
        self.loader.next_req += 1;
        ReqId(self.loader.next_req)
    }

    /// Barrier rule (Test seams): first frame + initial load done.
    pub fn is_ready(&self) -> bool {
        self.ready
    }
}

/// Shared fixtures for the shell unit tests.
#[cfg(test)]
pub(crate) mod testutil {
    use std::sync::Arc;

    use super::*;
    use crate::config::Config;
    use crate::diff::{DiffLine, FileDiff, Hunk, LineKind, Note, Status};
    use crate::integration::{
        AgentIntegration, CliRegistration, CommandOutput, CommandResult, CommandSpec, RegStatus,
    };
    use crate::keys::KeyEvent;
    use crate::mcp::McpEndpoint;
    use crate::options::Options;

    #[derive(Debug)]
    pub struct FakeAgent {
        pub id: &'static str,
        pub label: &'static str,
        pub can: bool,
    }

    pub fn spec(args: &[&str]) -> CommandSpec {
        CommandSpec {
            program: "agent".into(),
            args: args.iter().map(|s| s.to_string()).collect(),
            cwd: None,
            env: vec![],
            timeout_ms: 20_000,
        }
    }

    impl AgentIntegration for FakeAgent {
        fn id(&self) -> &'static str {
            self.id
        }
        fn label(&self) -> &'static str {
            self.label
        }
        fn poll_seconds(&self) -> u32 {
            45
        }
        fn register_text(&self, ep: &McpEndpoint) -> String {
            format!("register {} {}", ep.url, ep.token)
        }
        fn watch_prompt(&self, ep: &McpEndpoint) -> String {
            format!("watch {}", ep.token)
        }
        fn registration(&self) -> Option<&dyn CliRegistration> {
            self.can.then_some(self)
        }
    }

    impl CliRegistration for FakeAgent {
        fn needs_restart(&self) -> bool {
            true
        }
        fn check_command(&self, _ep: &McpEndpoint) -> CommandSpec {
            spec(&["get"])
        }
        fn parse_check(&self, _ep: &McpEndpoint, result: &CommandResult) -> RegStatus {
            match result {
                Ok(o) if o.code == 0 && o.stdout.contains("stale") => RegStatus::Stale,
                Ok(o) if o.code == 0 => RegStatus::Registered,
                _ => RegStatus::NotRegistered,
            }
        }
        fn register_commands(&self, _ep: &McpEndpoint) -> Vec<CommandSpec> {
            vec![spec(&["remove"]), spec(&["add"])]
        }
        fn register_hint(&self, ep: &McpEndpoint) -> String {
            format!("Registered {}; restart the agent session, then paste the watch prompt", ep.token)
        }
        fn unregister_command(&self) -> CommandSpec {
            spec(&["remove"])
        }
    }

    pub fn ok(code: i32, stdout: &str, stderr: &str) -> CommandResult {
        Ok(CommandOutput { code, stdout: stdout.into(), stderr: stderr.into() })
    }

    pub fn ep() -> McpEndpoint {
        McpEndpoint { url: "http://127.0.0.1:47615/mcp".into(), token: "secrettoken".into(), port: 47615 }
    }

    pub fn file(path: &str, lines: usize) -> FileDiff {
        let ls = (1..=lines as u32)
            .map(|n| DiffLine {
                kind: LineKind::Add,
                old_no: None,
                new_no: Some(n),
                text: format!("line {n}"),
                no_newline_marker: false,
            })
            .collect();
        FileDiff {
            path: path.into(),
            old_path: None,
            status: Status::Added,
            adds: lines as u32,
            dels: 0,
            hunks: vec![Hunk { header: format!("@@ -0,0 +1,{lines} @@"), lines: ls }],
            note: None::<Note>,
        }
    }

    pub fn fake_state() -> State {
        fake_state_with(Options::default(), Config::default())
    }

    pub fn fake_state_with(options: Options, config: Config) -> State {
        let init = Init {
            options,
            config,
            env: EnvInfo {
                abs_cwd: "/repo".into(),
                sync: false,
                mcp_port_raw: None,
                state_dir: "/state".into(),
                config_path: "/cfg/config.json".into(),
            },
            size: Size { cols: 80, rows: 24 },
        };
        let ints: Integrations = vec![
            Arc::new(FakeAgent { id: "a1", label: "Alpha", can: true }),
            Arc::new(FakeAgent { id: "b2", label: "Beta", can: false }),
        ];
        State::new(init, ints).0
    }

    /// Running-server state: endpoint set as after a successful start.
    pub fn running_state() -> State {
        let mut s = fake_state();
        s.mcp.server = crate::mcp::ServerState::Running(ep());
        s.mcp_ui.last_token = "secrettoken".into();
        s
    }

    pub fn k(c: char) -> KeyEvent {
        KeyEvent::ch(c)
    }
}
