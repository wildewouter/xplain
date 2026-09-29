//! Row model of what the viewport shows: unified rows, split (paired) rows, browse rows, plus the row-level
//! text helpers every other module uses to talk about "row i".
//!
//! Spec: F-LAYOUT-03/04/05 (row structure, pairing), F-NAV-05 (change starts), F-CURSOR-06 (pane),
//! F-HEADER-03 (row numbers), F-FIND-02 (search texts), F-EDGE-02..04 (note row). Oracle:
//! `src/components/DiffView.tsx` (Row, SRow, toRows, toSplit, changeStarts, rowNo, paneOf, rowCode, findAll).
//! Owner: component `nav` (B).
//! Must not: render, mutate anything but `state.rows`, or know comments. Browse rows are `Line` rows of kind
//! `Context` with `new_no = index+1`.

use crate::comments::PaneSide;
use crate::diff::{FileDiff, LineKind};
use crate::state::{Browse, State};

/// One code line of a row (a diff line, or a browse line).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowLine {
    pub kind: LineKind,
    pub old_no: Option<u32>,
    pub new_no: Option<u32>,
    /// Raw text (tabs not yet expanded).
    pub text: String,
    pub no_newline_marker: bool,
}

/// One shown row. In unified/browse mode only `Hunk`, `Note`, `Line`; in effective split `Pair` replaces `Line`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShownRow {
    Hunk(String),
    Note(String),
    Line(RowLine),
    /// Split pair: `l` old side, `r` new side; a context line is `l == r`; `None` = empty cell.
    Pair {
        l: Option<RowLine>,
        r: Option<RowLine>,
    },
}

/// What the cached rows were built for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowsKey {
    pub file_index: usize,
    pub split: bool,
    pub browse: Option<String>,
    pub files_gen: u64,
}

#[derive(Debug, Clone, Default)]
pub struct Rows {
    pub rows: Vec<ShownRow>,
    /// Indices where a run of changed rows starts (F-NAV-05).
    pub change_starts: Vec<usize>,
    pub key: Option<RowsKey>,
}

/// Split layout in effect: setting on, cols >= 100, not browse (F-LAYOUT-04).
pub fn effective_split(_state: &State) -> bool {
    todo!("F-LAYOUT-04")
}

/// Rebuild `state.rows` when its key differs from the current one. Handlers call this after changing
/// `nav.file_index`, `settings.split`, `browse`, `files`; `update` calls it after every event.
pub fn ensure(_state: &mut State) {
    todo!("cache rebuild")
}

/// Rows of one diff file (note row XOR hunks); `split` pairs dels/adds (F-LAYOUT-04).
pub fn build_rows(_file: &FileDiff, _split: bool) -> Vec<ShownRow> {
    todo!("F-LAYOUT-03/04")
}

/// Browse rows (F-LAYOUT-05).
pub fn build_browse_rows(_browse: &Browse) -> Vec<ShownRow> {
    todo!("F-LAYOUT-05")
}

pub fn change_starts(_rows: &[ShownRow]) -> Vec<usize> {
    todo!("F-NAV-05")
}

/// Line number the row shows for `side` (F-HEADER-03 rules): unified new else old; pair by pane.
pub fn row_no(_row: &ShownRow, _side: PaneSide) -> Option<u32> {
    todo!("F-HEADER-03")
}

/// Pane that really carries the cursor: del-only pairs always use the left pane.
pub fn pane_of(_row: &ShownRow, _side: PaneSide) -> PaneSide {
    todo!("F-CURSOR-06")
}

/// Tab-expanded code text under the cursor for this row and pane (empty for hunk/note rows? no: hunk and
/// note rows have their text as code too, as in `rowCode`).
pub fn row_code(_row: &ShownRow, _side: PaneSide) -> String {
    todo!("rowCode")
}

/// Whether the row is a change (add/del, or pair with any non-context side).
pub fn is_change(_row: &ShownRow) -> bool {
    todo!("F-NAV-05")
}

/// Smart-case substring hits as `[start, end)` char ranges (F-FIND-02).
pub fn find_all(_text: &str, _term: &str) -> Vec<(usize, usize)> {
    todo!("F-FIND-02")
}

/// Texts of a row that find looks at (pair: both sides, deduplicated; hunk/note: none) (F-FIND-02).
pub fn search_texts(_row: &ShownRow) -> Vec<String> {
    todo!("F-FIND-02")
}
