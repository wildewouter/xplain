//! Help: key table, contexts, footer hints, help panel state machine.
//!
//! Spec: F-HELP-01 (panel `?`, levels), F-HELP-02 (contexts + priority), F-HELP-03 (entries), F-HELP-04
//! (footer hints per state), F-LAYOUT-02 item 5. Oracle: `src/keys.ts` (KEYS, helpCtx, keysFor, footerFor)
//! and `src/components/HelpModal.tsx` (grouping only; drawing is `view::help_panel`).
//! Owner: component `navops` (C). Must not render.

use crate::effect::Fx;
use crate::keys::{Key, KeyEvent};
use crate::state::{EditorKind, HelpLevel, Overlay, State};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HelpCtx {
    Cursor,
    Visual,
    Comment,
    Editor,
    Find,
    Goto,
    Picker,
    Search,
    Mcp,
    Config,
    Dialog,
    Browse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelpEntry {
    pub group: &'static str,
    pub keys: String,
    pub desc: String,
}

/// Footer item ids (`BarId` in keys.ts).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Bar {
    Scroll,
    Help,
    Move,
    Ask,
    Comments,
    Pane,
    End,
    Edit,
    Delete,
    Follow,
    Back,
    Send,
    Save,
    Cancel,
}

/// Footer variants (`Bar` in keys.ts).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BarKind {
    Cursor,
    Split,
    Visual,
    Comment,
    Ask,
    Edit,
}

fn bar_text(b: Bar) -> &'static str {
    match b {
        Bar::Scroll => "j/k scroll",
        Bar::Help => "? help",
        Bar::Move => "hjkl move",
        Bar::Ask => "enter ask",
        Bar::Comments => "J/K comments",
        Bar::Pane => "p pane",
        Bar::End => "v/esc end",
        Bar::Edit => "e edit",
        Bar::Delete => "D delete",
        Bar::Follow => "a ask/follow up",
        Bar::Back => "esc back",
        Bar::Send => "enter send",
        Bar::Save => "tab save/ask",
        Bar::Cancel => "esc cancel",
    }
}

fn bar_items(k: BarKind) -> &'static [Bar] {
    match k {
        BarKind::Cursor => &[Bar::Move, Bar::Ask, Bar::Comments, Bar::Help],
        BarKind::Split => &[Bar::Move, Bar::Ask, Bar::Comments, Bar::Pane, Bar::Help],
        BarKind::Visual => &[Bar::End, Bar::Ask, Bar::Move, Bar::Help],
        BarKind::Comment => &[Bar::Edit, Bar::Delete, Bar::Follow, Bar::Scroll, Bar::Back, Bar::Help],
        BarKind::Ask => &[Bar::Send, Bar::Save, Bar::Cancel],
        BarKind::Edit => &[Bar::Send, Bar::Cancel],
    }
}

/// Footers that can show in each help context; modals and text inputs keep the view's footer, which does
/// not describe their keys.
fn ctx_bars(c: HelpCtx) -> &'static [BarKind] {
    match c {
        HelpCtx::Cursor => &[BarKind::Cursor, BarKind::Split],
        HelpCtx::Browse => &[BarKind::Cursor],
        HelpCtx::Visual => &[BarKind::Visual],
        HelpCtx::Comment => &[BarKind::Comment],
        HelpCtx::Editor => &[BarKind::Ask, BarKind::Edit],
        _ => &[],
    }
}

fn in_bar(c: HelpCtx, id: Option<Bar>) -> bool {
    let Some(id) = id else { return false };
    let bars = ctx_bars(c);
    !bars.is_empty() && bars.iter().all(|b| bar_items(*b).contains(&id))
}

/// One row of the key table (`Key` in keys.ts).
struct Def {
    g: &'static str,
    k: &'static str,
    d: &'static str,
    c: &'static [HelpCtx],
    vim: bool,
    bar: Option<Bar>,
    rest: Option<(&'static str, &'static str)>,
}

const fn def(g: &'static str, k: &'static str, d: &'static str, c: &'static [HelpCtx]) -> Def {
    Def { g, k, d, c, vim: false, bar: None, rest: None }
}

impl Def {
    const fn vim(mut self) -> Self {
        self.vim = true;
        self
    }
    const fn bar(mut self, b: Bar) -> Self {
        self.bar = Some(b);
        self
    }
    const fn rest(mut self, k: &'static str, d: &'static str) -> Self {
        self.rest = Some((k, d));
        self
    }
}

use HelpCtx as C;

const CB: &[HelpCtx] = &[C::Cursor, C::Browse];
const CBVC: &[HelpCtx] = &[C::Cursor, C::Browse, C::Visual, C::Comment];

/// Key table. Keys that work the same everywhere are listed only in `Cursor`; other contexts list what differs.
static KEYS: &[Def] = &[
    def("Move", "h/j/k/l", "char / line", CB).vim().bar(Bar::Move),
    def("Move", "w/b/e", "word fwd/back/end", CB).vim(),
    def("Move", "0/^/$", "start/nonblank/end", CB).vim(),
    def("Move", "d/u", "half page down/up", CB).vim(),
    def("Move", "PgDn/PgUp", "page down/up (space: down)", CB).vim(),
    def("Move", "g/G", "first / last line", CB).vim(),
    def("Move", "1-9", "count (5j, 12G)", CB).vim(),
    def("Find", "]/[", "next/prev change", &[C::Cursor]),
    def("Find", "/ n/N", "find, next/prev", &[C::Cursor]),
    def("Find", ":", "go to line", &[C::Cursor]),
    def("Find", "type", "search text", &[C::Find]),
    def("Find", "backspace", "delete char", &[C::Find]),
    def("Find", "Enter", "jump to match", &[C::Find]),
    def("Find", "esc", "cancel", &[C::Find]),
    def("Go to line", "type", "line number", &[C::Goto]),
    def("Go to line", "backspace", "delete char", &[C::Goto]),
    def("Go to line", "Enter", "go to line", &[C::Goto]),
    def("Go to line", "esc", "cancel", &[C::Goto]),
    def("Find", "tab/S-tab", "next / prev file", &[C::Cursor]),
    def("Find", "f/F", "file picker / search", &[C::Cursor]),
    def("File viewer", "esc", "back to diff", &[C::Browse]),
    def("File viewer", "s/c/m/f", "diff-only, no-op", &[C::Browse]),
    def("Comments", "v/V", "select chars/lines", CB),
    def("Comments", "Enter/a", "comment on line", CB).bar(Bar::Ask).rest("a", "comment on line"),
    def("General", "s/c/m", "split, full, staged", &[C::Cursor]),
    def("General", "t/r", "theme / reload", &[C::Cursor]),
    def("General", "p", "old/new pane", &[C::Cursor]).bar(Bar::Pane),
    def("Move", "hjkl…", "extend selection", &[C::Visual]).vim().bar(Bar::Move),
    def("Selection", "v/V", "chars/lines, end", &[C::Visual]).bar(Bar::End).rest("V", "lines, end"),
    def("Selection", "Enter/a", "comment on it", &[C::Visual]).bar(Bar::Ask).rest("a", "comment on it"),
    def("Selection", "esc", "end selection", &[C::Visual]).bar(Bar::End),
    def("Comments", "J/K", "next/prev in file", CBVC).bar(Bar::Comments),
    def("Comments", ")/(", "numbered, any file", CBVC),
    def("Comments", "E", "export comments", &[C::Cursor]),
    def("Move", "j/k d/u", "scroll thread", &[C::Comment])
        .vim()
        .bar(Bar::Scroll)
        .rest("d/u", "scroll thread"),
    def("Move", "g/G", "thread top/bottom", &[C::Comment]).vim(),
    def("Comments", "e/Enter", "edit (no replies)", &[C::Comment])
        .bar(Bar::Edit)
        .rest("Enter", "edit (no replies)"),
    def("Comments", "D", "delete (y/n)", &[C::Comment]).bar(Bar::Delete),
    def("Comments", "a/A", "ask: this / all", &[C::Comment]).bar(Bar::Follow).rest("A", "ask: all"),
    def("Comments", "up/down", "pick block to copy", &[C::Comment]),
    def("Comments", "hjkl…", "motion unfocuses", &[C::Comment]),
    def("Comments", "esc", "unpick / unfocus", &[C::Comment]).bar(Bar::Back),
    def("Editor", "type", "comment text", &[C::Editor]),
    def("Editor", "Enter", "send", &[C::Editor]).bar(Bar::Send),
    def("Editor", "tab", "save / ask agent", &[C::Editor]).bar(Bar::Save),
    def("Editor", "left/right", "move cursor", &[C::Editor]),
    def("Editor", "backspace", "delete char", &[C::Editor]),
    def("Editor", "esc", "cancel", &[C::Editor]).bar(Bar::Cancel),
    def("Move", "j/k", "move", &[C::Picker]).vim(),
    def("Move", "d/u", "half page down/up", &[C::Picker]).vim(),
    def("File picker", "Enter", "open file", &[C::Picker]),
    def("File picker", "esc/f/q", "close", &[C::Picker]),
    def("Search", "type", "filter files", &[C::Search]),
    def("Search", "backspace", "delete char", &[C::Search]),
    def("Search", "down/up", "next / prev hit", &[C::Search]),
    def("Search", "Enter", "open hit", &[C::Search]),
    def("Search", "esc", "close", &[C::Search]),
    def("Move", "j/k", "move", &[C::Mcp]).vim(),
    def("MCP (M)", "⏎/space", "server on / off", &[C::Mcp]),
    def("MCP (M)", "Enter", "register agent", &[C::Mcp]),
    def("MCP (M)", "d", "unregister agent", &[C::Mcp]),
    def("MCP (M)", "y/n", "confirm / cancel", &[C::Mcp]),
    def("MCP (M)", "c/w", "copy cmd / prompt", &[C::Mcp]),
    def("MCP (M)", "R", "refresh status", &[C::Mcp]),
    def("MCP (M)", "esc/q/M", "close", &[C::Mcp]),
    def("Move", "j/k", "select setting", &[C::Config]).vim(),
    def("Config (C)", "h/l", "change value", &[C::Config]),
    def("Config (C)", "⏎/space", "toggle / apply", &[C::Config]),
    def("Config (C)", "esc/q/C", "close", &[C::Config]),
    def("Dialogs", "y/Enter", "confirm", &[C::Dialog]),
    def("Dialogs", "n/esc", "cancel (q: quit)", &[C::Dialog]),
    def("General", "M/C", "MCP / config", &[C::Cursor]),
    def("General", "?", "help/more/close", &[C::Cursor]).bar(Bar::Help),
    def("General", "q", "quit", &[C::Cursor]),
];

/// `keysFor`: entries of `ctx`; vim motions only when `motions`; entries the footer already shows are dropped
/// or reduced to their `rest`.
fn keys_for(ctx: HelpCtx, motions: bool) -> Vec<HelpEntry> {
    KEYS.iter()
        .filter(|k| k.c.contains(&ctx) && (motions || !k.vim))
        .filter_map(|k| {
            if !in_bar(ctx, k.bar) {
                Some(HelpEntry { group: k.g, keys: k.k.to_string(), desc: k.d.to_string() })
            } else {
                k.rest.map(|(rk, rd)| HelpEntry { group: k.g, keys: rk.to_string(), desc: rd.to_string() })
            }
        })
        .collect()
}

/// Context priority mirrors the key handler order (F-HELP-02).
pub fn help_ctx(state: &State) -> HelpCtx {
    match &state.overlay {
        Overlay::Editor(_) => return HelpCtx::Editor,
        Overlay::Find { .. } => return HelpCtx::Find,
        Overlay::Goto { .. } => return HelpCtx::Goto,
        Overlay::DeleteComment { .. } | Overlay::Quit => return HelpCtx::Dialog,
        Overlay::Search(_) => return HelpCtx::Search,
        Overlay::Mcp(_) => return HelpCtx::Mcp,
        Overlay::Config(_) => return HelpCtx::Config,
        Overlay::Picker { .. } => return HelpCtx::Picker,
        Overlay::None => {}
    }
    if state.nav.focused_comment.is_some() {
        HelpCtx::Comment
    } else if state.nav.selection.is_some() {
        HelpCtx::Visual
    } else if state.browse.is_some() {
        HelpCtx::Browse
    } else {
        HelpCtx::Cursor
    }
}

/// Context title (`Diff view`, `Visual selection`, ...).
pub fn ctx_label(ctx: HelpCtx) -> &'static str {
    match ctx {
        HelpCtx::Cursor => "Diff view",
        HelpCtx::Visual => "Visual selection",
        HelpCtx::Comment => "Focused comment",
        HelpCtx::Editor => "Editor",
        HelpCtx::Find => "Find in file",
        HelpCtx::Goto => "Go to line",
        HelpCtx::Picker => "File picker",
        HelpCtx::Search => "File search",
        HelpCtx::Mcp => "MCP",
        HelpCtx::Config => "Config",
        HelpCtx::Dialog => "Confirm",
        HelpCtx::Browse => "File viewer",
    }
}

/// Entries for a context; `motions` shows vim motions (level 2) (F-HELP-03). A context without motion keys
/// shows its level-1 entries whatever `motions` says (F-HELP-01).
pub fn entries_for(ctx: HelpCtx, motions: bool) -> Vec<HelpEntry> {
    keys_for(ctx, motions && has_motions(ctx))
}

/// Whether the context has motion entries to expand (`? more`).
pub fn has_motions(ctx: HelpCtx) -> bool {
    KEYS.iter().any(|k| k.vim && k.c.contains(&ctx) && (!in_bar(ctx, k.bar) || k.rest.is_some()))
}

/// Footer key hints for the current state, e.g. `hjkl move  enter ask  J/K comments  ? help` (F-HELP-04).
pub fn footer_hints(state: &State) -> String {
    let ask = matches!(state.overlay, Overlay::Editor(_));
    let edit = matches!(&state.overlay, Overlay::Editor(e) if !matches!(e.kind, EditorKind::New));
    let kind = if ask && edit {
        BarKind::Edit
    } else if ask {
        BarKind::Ask
    } else if state.nav.focused_comment.is_some() {
        BarKind::Comment
    } else if state.nav.selection.is_some() {
        BarKind::Visual
    } else if split_pane(state) {
        BarKind::Split
    } else {
        BarKind::Cursor
    };
    bar_items(kind).iter().map(|b| bar_text(*b)).collect::<Vec<_>>().join("  ")
}

/// Split panes shown for a diff file (`canSide` in app.tsx): effective split, not browse, not the no-changes screen.
fn split_pane(state: &State) -> bool {
    crate::rows::effective_split(state)
        && state.browse.is_none()
        && state.files.get(state.nav.file_index).is_some()
}

/// `?` and Esc handling while the panel is open or closed. Runs first in key dispatch: returns true when the
/// key was consumed (panel open swallows everything but its own keys, F-HELP-01).
pub fn on_key(state: &mut State, key: KeyEvent, _fx: &mut Fx) -> bool {
    if key.key != Key::Char('?') || key.mods.ctrl || key.mods.alt {
        return false;
    }
    // text inputs type `?`
    if matches!(
        state.overlay,
        Overlay::Editor(_) | Overlay::Find { .. } | Overlay::Goto { .. } | Overlay::Search(_)
    ) {
        return false;
    }
    state.nav.count = 0;
    state.help = match state.help {
        HelpLevel::Closed => HelpLevel::L1,
        HelpLevel::L1 if has_motions(help_ctx(state)) => HelpLevel::L2,
        HelpLevel::L1 | HelpLevel::L2 => HelpLevel::Closed,
    };
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jump::testkit::*;
    use crate::state::{EditorState, Selection, SelectionKind};

    /// `[group] key :: desc` lines, groups in order of first item.
    fn snap(ctx: HelpCtx, motions: bool) -> Vec<String> {
        let es = entries_for(ctx, motions);
        let mut groups: Vec<&str> = Vec::new();
        for e in &es {
            if !groups.contains(&e.group) {
                groups.push(e.group);
            }
        }
        groups
            .iter()
            .flat_map(|g| {
                std::iter::once(format!("[{g}]"))
                    .chain(es.iter().filter(|e| e.group == *g).map(|e| format!("{} :: {}", e.keys, e.desc)))
            })
            .collect()
    }

    fn strs(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn editor(kind: EditorKind) -> Overlay {
        Overlay::Editor(EditorState { kind, text: String::new(), caret: 0, ask_mode: false })
    }

    const DIFF_L1: &[&str] = &[
        "[Find]",
        "]/[ :: next/prev change",
        "/ n/N :: find, next/prev",
        ": :: go to line",
        "tab/S-tab :: next / prev file",
        "f/F :: file picker / search",
        "[Comments]",
        "v/V :: select chars/lines",
        "a :: comment on line",
        ")/( :: numbered, any file",
        "E :: export comments",
        "[General]",
        "s/c/m :: split, full, staged",
        "t/r :: theme / reload",
        "p :: old/new pane",
        "M/C :: MCP / config",
        "q :: quit",
    ];

    const MOVE_L2: &[&str] = &[
        "[Move]",
        "w/b/e :: word fwd/back/end",
        "0/^/$ :: start/nonblank/end",
        "d/u :: half page down/up",
        "PgDn/PgUp :: page down/up (space: down)",
        "g/G :: first / last line",
        "1-9 :: count (5j, 12G)",
    ];

    #[test]
    fn f_help_03_diff_view() {
        assert_eq!(snap(HelpCtx::Cursor, false), strs(DIFF_L1));
        let mut l2 = strs(MOVE_L2);
        l2.extend(strs(DIFF_L1));
        assert_eq!(snap(HelpCtx::Cursor, true), l2);
    }

    #[test]
    fn f_help_03_file_viewer() {
        let l1 = [
            "[File viewer]",
            "esc :: back to diff",
            "s/c/m/f :: diff-only, no-op",
            "[Comments]",
            "v/V :: select chars/lines",
            "a :: comment on line",
            ")/( :: numbered, any file",
        ];
        assert_eq!(snap(HelpCtx::Browse, false), strs(&l1));
        let mut l2 = strs(MOVE_L2);
        l2.extend(strs(&l1));
        assert_eq!(snap(HelpCtx::Browse, true), l2);
    }

    #[test]
    fn f_help_03_visual_and_comment() {
        let visual = [
            "[Selection]",
            "V :: lines, end",
            "a :: comment on it",
            "[Comments]",
            "J/K :: next/prev in file",
            ")/( :: numbered, any file",
        ];
        assert_eq!(snap(HelpCtx::Visual, false), strs(&visual));
        assert_eq!(snap(HelpCtx::Visual, true), strs(&visual));
        let comment = [
            "[Comments]",
            "J/K :: next/prev in file",
            ")/( :: numbered, any file",
            "Enter :: edit (no replies)",
            "A :: ask: all",
            "up/down :: pick block to copy",
            "hjkl… :: motion unfocuses",
        ];
        assert_eq!(snap(HelpCtx::Comment, false), strs(&comment));
        let mut l2 = strs(&comment);
        l2.extend(strs(&["[Move]", "d/u :: scroll thread", "g/G :: thread top/bottom"]));
        assert_eq!(snap(HelpCtx::Comment, true), l2);
    }

    #[test]
    fn f_help_03_text_inputs() {
        assert_eq!(
            snap(HelpCtx::Editor, false),
            strs(&[
                "[Editor]",
                "type :: comment text",
                "tab :: save / ask agent",
                "left/right :: move cursor",
                "backspace :: delete char",
            ])
        );
        assert_eq!(
            snap(HelpCtx::Find, false),
            strs(&[
                "[Find]",
                "type :: search text",
                "backspace :: delete char",
                "Enter :: jump to match",
                "esc :: cancel"
            ])
        );
        assert_eq!(
            snap(HelpCtx::Goto, false),
            strs(&[
                "[Go to line]",
                "type :: line number",
                "backspace :: delete char",
                "Enter :: go to line",
                "esc :: cancel"
            ])
        );
        assert_eq!(
            snap(HelpCtx::Search, false),
            strs(&[
                "[Search]",
                "type :: filter files",
                "backspace :: delete char",
                "down/up :: next / prev hit",
                "Enter :: open hit",
                "esc :: close"
            ])
        );
    }

    #[test]
    fn f_help_03_modals() {
        let picker = ["[File picker]", "Enter :: open file", "esc/f/q :: close"];
        assert_eq!(snap(HelpCtx::Picker, false), strs(&picker));
        let mut l2 = strs(&["[Move]", "j/k :: move", "d/u :: half page down/up"]);
        l2.extend(strs(&picker));
        assert_eq!(snap(HelpCtx::Picker, true), l2);
        let mcp = [
            "[MCP (M)]",
            "⏎/space :: server on / off",
            "Enter :: register agent",
            "d :: unregister agent",
            "y/n :: confirm / cancel",
            "c/w :: copy cmd / prompt",
            "R :: refresh status",
            "esc/q/M :: close",
        ];
        assert_eq!(snap(HelpCtx::Mcp, false), strs(&mcp));
        let mut l2 = strs(&["[Move]", "j/k :: move"]);
        l2.extend(strs(&mcp));
        assert_eq!(snap(HelpCtx::Mcp, true), l2);
        let config = ["[Config (C)]", "h/l :: change value", "⏎/space :: toggle / apply", "esc/q/C :: close"];
        assert_eq!(snap(HelpCtx::Config, false), strs(&config));
        let mut l2 = strs(&["[Move]", "j/k :: select setting"]);
        l2.extend(strs(&config));
        assert_eq!(snap(HelpCtx::Config, true), l2);
        assert_eq!(
            snap(HelpCtx::Dialog, false),
            strs(&["[Dialogs]", "y/Enter :: confirm", "n/esc :: cancel (q: quit)"])
        );
    }

    #[test]
    fn f_help_01_has_motions_per_context() {
        use HelpCtx::*;
        for c in [Cursor, Comment, Picker, Mcp, Config, Browse] {
            assert!(has_motions(c), "{c:?}");
        }
        for c in [Visual, Editor, Find, Goto, Search, Dialog] {
            assert!(!has_motions(c), "{c:?}");
            assert_eq!(entries_for(c, true), entries_for(c, false), "{c:?}");
        }
    }

    #[test]
    fn f_help_02_labels() {
        use HelpCtx::*;
        let got: Vec<_> =
            [Cursor, Visual, Comment, Editor, Find, Goto, Picker, Search, Mcp, Config, Dialog, Browse]
                .map(ctx_label)
                .to_vec();
        assert_eq!(
            got,
            [
                "Diff view",
                "Visual selection",
                "Focused comment",
                "Editor",
                "Find in file",
                "Go to line",
                "File picker",
                "File search",
                "MCP",
                "Config",
                "Confirm",
                "File viewer"
            ]
        );
    }

    #[test]
    fn f_help_02_priority() {
        let mut s = state(vec![file("a.rs")]);
        assert_eq!(help_ctx(&s), HelpCtx::Cursor);
        s.browse = Some(crate::state::Browse { path: "x".into(), lines: vec![] });
        assert_eq!(help_ctx(&s), HelpCtx::Browse);
        s.nav.selection = Some(Selection { kind: SelectionKind::Line, anchor_row: 0, anchor_col: 0 });
        assert_eq!(help_ctx(&s), HelpCtx::Visual);
        s.nav.focused_comment = Some("q1".into());
        assert_eq!(help_ctx(&s), HelpCtx::Comment);
        s.overlay = Overlay::Picker { sel: 0 };
        assert_eq!(help_ctx(&s), HelpCtx::Picker);
        s.overlay = Overlay::Config(crate::state::ConfigModal {
            row: 0,
            cursors: [0; 6],
            committed_theme: s.settings.theme,
        });
        assert_eq!(help_ctx(&s), HelpCtx::Config);
        s.overlay = Overlay::Mcp(Default::default());
        assert_eq!(help_ctx(&s), HelpCtx::Mcp);
        s.overlay = Overlay::Search(Default::default());
        assert_eq!(help_ctx(&s), HelpCtx::Search);
        s.overlay = Overlay::Quit;
        assert_eq!(help_ctx(&s), HelpCtx::Dialog);
        s.overlay = Overlay::DeleteComment { id: "q1".into() };
        assert_eq!(help_ctx(&s), HelpCtx::Dialog);
        s.overlay = Overlay::Goto { text: String::new() };
        assert_eq!(help_ctx(&s), HelpCtx::Goto);
        s.overlay = Overlay::Find { text: String::new() };
        assert_eq!(help_ctx(&s), HelpCtx::Find);
        s.overlay = editor(EditorKind::New);
        assert_eq!(help_ctx(&s), HelpCtx::Editor);
    }

    #[test]
    fn f_help_04_footer_per_state() {
        let mut s = state(vec![file("a.rs")]);
        let base = "hjkl move  enter ask  J/K comments  ? help";
        assert_eq!(footer_hints(&s), base);
        // browse and no-changes screen keep the plain footer, also with split on and wide
        s.settings.split = true;
        s.size.cols = 120;
        assert_eq!(footer_hints(&s), "hjkl move  enter ask  J/K comments  p pane  ? help");
        let mut b = state(vec![file("a.rs")]);
        b.settings.split = true;
        b.size.cols = 120;
        b.browse = Some(crate::state::Browse { path: "x".into(), lines: vec![] });
        assert_eq!(footer_hints(&b), base);
        let mut e = state(Vec::new());
        e.settings.split = true;
        e.size.cols = 120;
        assert_eq!(footer_hints(&e), base);
        // narrow: split not effective
        s.size.cols = 80;
        assert_eq!(footer_hints(&s), base);
        s.nav.selection = Some(Selection { kind: SelectionKind::Char, anchor_row: 0, anchor_col: 0 });
        assert_eq!(footer_hints(&s), "v/esc end  enter ask  hjkl move  ? help");
        s.nav.focused_comment = Some("q1".into());
        assert_eq!(footer_hints(&s), "e edit  D delete  a ask/follow up  j/k scroll  esc back  ? help");
        s.overlay = editor(EditorKind::New);
        assert_eq!(footer_hints(&s), "enter send  tab save/ask  esc cancel");
        s.overlay = editor(EditorKind::Edit { id: "q1".into() });
        assert_eq!(footer_hints(&s), "enter send  esc cancel");
        s.overlay = editor(EditorKind::FollowUp { id: "q1".into() });
        assert_eq!(footer_hints(&s), "enter send  esc cancel");
        // modals and find/goto keep the underlying hint
        s.nav.focused_comment = None;
        s.nav.selection = None;
        for o in [Overlay::Find { text: String::new() }, Overlay::Goto { text: String::new() }, Overlay::Quit]
        {
            s.overlay = o;
            assert_eq!(footer_hints(&s), base);
        }
    }

    fn press_q(s: &mut State) -> bool {
        on_key(s, KeyEvent::ch('?'), &mut Vec::new())
    }

    #[test]
    fn f_help_01_level_machine_with_motions() {
        let mut s = state(vec![file("a.rs")]);
        s.nav.count = 4;
        assert!(press_q(&mut s));
        assert_eq!(s.help, HelpLevel::L1);
        assert_eq!(s.nav.count, 0);
        assert!(press_q(&mut s));
        assert_eq!(s.help, HelpLevel::L2);
        assert!(press_q(&mut s));
        assert_eq!(s.help, HelpLevel::Closed);
    }

    #[test]
    fn f_help_01_context_without_motions_skips_level_two() {
        let mut s = state(vec![file("a.rs")]);
        s.nav.selection = Some(Selection { kind: SelectionKind::Line, anchor_row: 0, anchor_col: 0 });
        press_q(&mut s);
        assert_eq!(s.help, HelpLevel::L1);
        press_q(&mut s);
        assert_eq!(s.help, HelpLevel::Closed);
        // level 2 stays open when the context loses motions: next `?` closes
        s.nav.selection = None;
        press_q(&mut s);
        press_q(&mut s);
        assert_eq!(s.help, HelpLevel::L2);
        s.nav.selection = Some(Selection { kind: SelectionKind::Line, anchor_row: 0, anchor_col: 0 });
        press_q(&mut s);
        assert_eq!(s.help, HelpLevel::Closed);
    }

    #[test]
    fn f_help_01_text_inputs_type_question_mark() {
        let mut s = state(vec![file("a.rs")]);
        for o in [
            Overlay::Find { text: String::new() },
            Overlay::Goto { text: String::new() },
            Overlay::Search(Default::default()),
            editor(EditorKind::New),
        ] {
            s.overlay = o;
            assert!(!press_q(&mut s));
            assert_eq!(s.help, HelpLevel::Closed);
        }
        // modals toggle it
        s.overlay = Overlay::Quit;
        assert!(press_q(&mut s));
        assert_eq!(s.help, HelpLevel::L1);
    }

    #[test]
    fn f_help_01_other_keys_pass_through() {
        let mut s = state(vec![file("a.rs")]);
        s.help = HelpLevel::L1;
        for k in [KeyEvent::ch('j'), KeyEvent::plain(Key::Esc), KeyEvent::ctrl('?')] {
            assert!(!on_key(&mut s, k, &mut Vec::new()));
        }
        assert_eq!(s.help, HelpLevel::L1);
    }
}
