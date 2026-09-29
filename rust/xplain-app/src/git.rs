//! Runs git for diff loading and file listing.
//!
//! Spec: F-MODE-01 (argv from `DiffSpec::git_argv`), F-MODE-02 (untracked: `git ls-files --others
//! --exclude-standard -z`, regular files only (not symlinks/dirs), size <= 1048576, unreadable skipped
//! silently, order kept, per-file `DiffSpec::untracked_argv`), F-MODE-04 (error text: git stderr
//! verbatim; `cannot run git: <reason>`; error always from the tracked `git diff`), F-CLI-06 (`--cwd`
//! checked before every load, git not spawned: `cannot open directory <dir>: <reason>`), F-SEARCH-01 /
//! F-FILES (`ListFiles`: `git ls-files --cached --others --exclude-standard`, sorted unique, failure
//! -> empty), UNSPEC-37 (git timeout 60 s).
//! Owner: component B (startup/IO).
//! Must not: parse diffs (core `parse_raw`), decide when to load, modify index/worktree.
//! `git diff` exit code 1 (with `--no-index`) is success; non-zero with empty stdout and stderr text is error.

use xplain_core::diff::{DiffSpec, RawDiff};

/// Executes `Effect::LoadDiff`. `Err` = final error-screen text.
pub async fn load_diff(_spec: &DiffSpec) -> Result<RawDiff, String> {
    todo!("F-MODE-01/02/04, F-CLI-06")
}

/// Executes `Effect::ListFiles`; never fails (empty list on error), sorted, unique.
pub async fn list_files(_cwd: Option<&str>) -> Vec<String> {
    todo!("ls-files")
}
