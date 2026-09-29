//! Review export: markdown rendering, file name, `E` key.
//!
//! Spec: F-EXPORT-01 (`E`: file `xplain-review-<local timestamp>.md` in cwd, notes, `WriteExport` effect,
//! `export failed` note), F-EXPORT-02 (exact markdown format). Oracle: `src/ask/export.ts`, `exportReview`
//! in `src/app.tsx`. Owner: component `agent` (E). Must not: do IO (effect only).

use crate::comments::Comment;
use crate::effect::Fx;
use crate::errors::IoReason;
use crate::event::ReqId;
use crate::state::{Now, State};

/// Inputs of the markdown header (F-EXPORT-02).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportMeta {
    pub cwd: String,
    pub mode: String,
    pub args: Vec<String>,
    pub now: Now,
}

/// `xplain-review-YYYYMMDD-HHMMSS.md` from local time (`unix_ms + utc_offset`) (F-EXPORT-01).
pub fn export_name(_now: &Now) -> String {
    todo!("F-EXPORT-01")
}

/// Whole markdown document (F-EXPORT-02).
pub fn render_markdown(_comments: &[Comment], _meta: &ExportMeta) -> String {
    todo!("F-EXPORT-02")
}

/// `E` in cursor context. True when consumed.
pub fn on_key(_state: &mut State, _key: crate::keys::KeyEvent, _fx: &mut Fx) -> bool {
    todo!("F-EXPORT-01")
}

/// `Event::ExportWritten`: success/failed note (F-EXPORT-01).
pub fn on_written(_state: &mut State, _req: ReqId, _result: Result<(), IoReason>) {
    todo!("F-EXPORT-01")
}
