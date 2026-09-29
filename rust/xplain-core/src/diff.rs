//! Diff model and unified-diff parser.
//!
//! Spec: F-EDGE-01..06, F-MODE-01 (argv builder), F-MODE-02 (assembly of untracked entries),
//! F-SCOPE-01. Owner: core lead (diff component).
//! Must not: run git or read files. The runtime runs git (argv from [`DiffSpec::git_argv`]) and hands
//! back raw text in [`RawDiff`]; parsing and status derivation happen here.

use crate::options::DiffMode;

/// Everything the runtime needs to load a diff (payload of `Effect::LoadDiff`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffSpec {
    /// `--cwd` verbatim; `None` = process cwd. Runtime checks it exists/is dir before spawning git (F-CLI-06).
    pub cwd: Option<String>,
    pub mode: DiffMode,
    /// Full-file scope adds `-U1000000`.
    pub full: bool,
    pub git_args: Vec<String>,
}

impl DiffSpec {
    /// Untracked files are listed and appended only in mode `all` with no extra args (F-MODE-02).
    pub fn wants_untracked(&self) -> bool {
        self.mode == DiffMode::All && self.git_args.is_empty()
    }

    /// Argv after `git`: `diff --no-color --no-ext-diff [-U1000000] <base> <extra>` (F-MODE-01).
    pub fn git_argv(&self) -> Vec<String> {
        todo!("F-MODE-01")
    }

    /// Argv after `git` for one untracked file: `diff --no-index --no-color --no-ext-diff [-U1000000] -- /dev/null <name>`.
    pub fn untracked_argv(&self, _name: &str) -> Vec<String> {
        todo!("F-MODE-02")
    }
}

/// Raw git output as delivered by the runtime.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RawDiff {
    /// stdout of the tracked `git diff` command.
    pub tracked: String,
    /// stdout of each `git diff --no-index` untracked-file command, in `ls-files` order. Runtime already
    /// skipped symlinks, dirs, unreadable and files > 1048576 bytes.
    pub untracked: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Context,
    Add,
    Del,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: LineKind,
    pub old_no: Option<u32>,
    pub new_no: Option<u32>,
    /// Raw text without the leading mark; tabs kept (expanded at render time).
    pub text: String,
    /// True for the synthetic ` No newline at end of file` row (F-EDGE-06); not counted in adds/dels.
    pub no_newline_marker: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    /// Full `@@ ... @@ ...` line.
    pub header: String,
    pub lines: Vec<DiffLine>,
}

/// Why a file has no hunks (single dim note row).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Note {
    /// `Renamed, no content changes`
    RenamedNoChanges,
    /// `Binary file`
    Binary,
    /// `No textual changes`
    NoTextualChanges,
}

/// Picker status letter (F-EDGE-05).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Added,
    Deleted,
    Modified,
    Renamed,
}

impl Status {
    pub fn letter(self) -> char {
        match self {
            Status::Added => 'A',
            Status::Deleted => 'D',
            Status::Modified => 'M',
            Status::Renamed => 'R',
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    /// Display path: new path (old path for deleted files), git prefix `a/`,`b/` stripped once (F-EDGE-01).
    pub path: String,
    /// Set for renames: `path` is the new path, this the old one. Display is `<old> -> <new>`.
    pub old_path: Option<String>,
    pub status: Status,
    pub adds: u32,
    pub dels: u32,
    pub hunks: Vec<Hunk>,
    /// `Some` when `hunks` is empty.
    pub note: Option<Note>,
}

impl FileDiff {
    /// `<old> -> <new>` for renames, else `path` (header and picker).
    pub fn display_path(&self) -> String {
        match &self.old_path {
            Some(old) => format!("{old} -> {}", self.path),
            None => self.path.clone(),
        }
    }
}

/// Parse tracked + untracked git output into files (tracked first, then untracked in given order).
pub fn parse_raw(_raw: &RawDiff) -> Vec<FileDiff> {
    todo!("F-EDGE-01..06")
}

/// Parse one unified diff text possibly holding several file entries.
pub fn parse_unified(_text: &str) -> Vec<FileDiff> {
    todo!("F-EDGE-01..06")
}
