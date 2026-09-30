//! Planner: reads the state and the cache, returns what to ask for and what to remember. Pure over `&State`;
//! [`super::HlCache`] applies the plans (`apply_files` / `apply_code`), so no state is moved out while planning.

use std::hash::{DefaultHasher, Hash, Hasher};

use super::{MAX_LINES, TRIGGER, WINDOW, line_key, side_ix};
use crate::comments::{Comment, PaneSide};
use crate::diff::LineKind;
use crate::highlight::{Carry, language_for_fence, language_for_path};
use crate::rows::ShownRow;
use crate::state::{LoadState, State};
use crate::thread_layout::{BodyKind, box_width, thread_body};

const SIDES: [PaneSide; 2] = [PaneSide::Old, PaneSide::New];

/// Inputs that decide whether the file planner has to look again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PlanKey {
    pub files_gen: u64,
    pub file_index: usize,
    pub top: usize,
    pub rows_high: u16,
    pub row_count: usize,
    pub split: bool,
    /// Bumped when a result lands.
    pub epoch: u64,
}

/// Inputs of the per-file stream hashes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MemoKey {
    files_gen: u64,
    file_index: usize,
    browse: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Memo {
    key: MemoKey,
    /// Content hash per side (`side_ix`).
    hashes: [u64; 2],
    big: bool,
}

/// What the planner remembers between events (separate from the cached results).
#[derive(Debug, Default)]
pub(super) struct Planner {
    pub memo: Option<Memo>,
    pub last_plan: Option<PlanKey>,
    pub code_sig: Option<u64>,
}

/// One chunk to highlight.
pub(super) struct Request {
    pub side: PaneSide,
    pub hash: u64,
    pub start: u32,
    pub lines: Vec<String>,
    pub carry: Option<Carry>,
}

pub(super) struct FilePlan {
    pub key: PlanKey,
    pub memo: Memo,
    pub path: String,
    pub lang: String,
    /// Mark the file as recently used (false for files above [`MAX_LINES`]).
    pub touch: bool,
    /// Entries (side, hash) that must exist and match the current content.
    pub seen: Vec<(PaneSide, u64)>,
    pub requests: Vec<Request>,
}

impl FilePlan {
    fn see(&mut self, side: PaneSide, hash: u64) {
        if !self.seen.contains(&(side, hash)) {
            self.seen.push((side, hash));
        }
    }
}

pub(super) struct CodePlan {
    pub sig: u64,
    pub lines: Vec<(&'static str, String)>,
}

fn current_path(state: &State) -> Option<&str> {
    match &state.browse {
        Some(b) => Some(b.path.as_str()),
        None => state.files.get(state.nav.file_index).map(|f| f.path.as_str()),
    }
}

/// (line number, text) of a side in file order: `New` = context + added (browse: all lines), `Old` = deleted.
fn stream(state: &State, side: PaneSide) -> Vec<(u32, &str)> {
    if let Some(b) = &state.browse {
        if side == PaneSide::Old {
            return Vec::new();
        }
        return b
            .lines
            .iter()
            .enumerate()
            .map(|(i, t)| (u32::try_from(i + 1).unwrap_or(u32::MAX), t.as_str()))
            .collect();
    }
    let Some(f) = state.files.get(state.nav.file_index) else { return Vec::new() };
    let mut out = Vec::new();
    for l in f.hunks.iter().flat_map(|h| &h.lines) {
        if l.no_newline_marker {
            continue;
        }
        let no = match (side, l.kind) {
            (PaneSide::New, LineKind::Add | LineKind::Context) => l.new_no,
            (PaneSide::Old, LineKind::Del) => l.old_no,
            _ => None,
        };
        if let Some(n) = no {
            out.push((n, l.text.as_str()));
        }
    }
    out
}

fn hash_stream(s: &[(u32, &str)]) -> u64 {
    let mut h = DefaultHasher::new();
    s.len().hash(&mut h);
    for (n, t) in s {
        n.hash(&mut h);
        t.hash(&mut h);
    }
    h.finish()
}

/// Contiguous runs of a stream (consecutive line numbers).
struct Seg<'a> {
    first: u32,
    lines: Vec<&'a str>,
}

fn segments<'a>(stream: &[(u32, &'a str)]) -> Vec<Seg<'a>> {
    let mut out: Vec<Seg<'a>> = Vec::new();
    for (n, t) in stream {
        match out.last_mut() {
            Some(s) if s.first.saturating_add(s.lines.len() as u32) == *n => s.lines.push(t),
            _ => out.push(Seg { first: *n, lines: vec![t] }),
        }
    }
    out
}

/// Row window `[from, to)` around the viewport, `pad` rows each way.
fn zone(state: &State, pad: usize) -> (usize, usize) {
    let top = state.nav.top;
    let h = state.size.rows as usize;
    (top.saturating_sub(pad), top.saturating_add(h).saturating_add(pad).min(state.rows.rows.len()))
}

/// (side, line number) of every shown code line in rows `[from, to)`.
fn lines_in(rows: &[ShownRow], from: usize, to: usize) -> Vec<(PaneSide, u32)> {
    let mut out = Vec::new();
    for row in rows.get(from..to.max(from)).unwrap_or_default() {
        match row {
            ShownRow::Line(l) => out.extend(line_key(l)),
            ShownRow::Pair { l, r } => {
                out.extend(l.as_ref().and_then(line_key));
                out.extend(r.as_ref().and_then(line_key));
            }
            ShownRow::Hunk(_) | ShownRow::Note(_) => {}
        }
    }
    out
}

fn stream_memo(state: &State, key: MemoKey) -> Memo {
    let (old, new) = (stream(state, PaneSide::Old), stream(state, PaneSide::New));
    let last = |s: &[(u32, &str)]| s.last().map_or(0, |(n, _)| *n);
    Memo { key, hashes: [hash_stream(&old), hash_stream(&new)], big: last(&old).max(last(&new)) > MAX_LINES }
}

/// Highlight requests near the viewport, or `None` when nothing changed since the last plan.
pub(super) fn plan_files(state: &State) -> Option<FilePlan> {
    if !matches!(state.load, LoadState::Ready) || state.rows.rows.is_empty() {
        return None;
    }
    let path = current_path(state)?;
    let lang = language_for_path(path)?;
    let split = state.rows.key.as_ref().is_some_and(|k| k.split);
    let cache = &state.hl;
    let key = PlanKey {
        files_gen: state.files_gen,
        file_index: state.nav.file_index,
        top: state.nav.top,
        rows_high: state.size.rows,
        row_count: state.rows.rows.len(),
        split,
        epoch: cache.epoch,
    };
    if cache.planner.last_plan == Some(key) {
        return None;
    }
    let mk = MemoKey {
        files_gen: state.files_gen,
        file_index: state.nav.file_index,
        browse: state.browse.is_some(),
    };
    let memo = match cache.planner.memo {
        Some(m) if m.key == mk => m,
        _ => stream_memo(state, mk),
    };
    let mut plan = FilePlan {
        key,
        memo,
        path: path.to_string(),
        lang: lang.to_string(),
        touch: false,
        seen: Vec::new(),
        requests: Vec::new(),
    };
    if memo.big {
        return Some(plan);
    }
    plan.touch = true;

    let rows = &state.rows.rows;
    let (t0, t1) = zone(state, TRIGGER);
    let mut trigger = false;
    for (side, no) in lines_in(rows, t0, t1) {
        let hash = memo.hashes[side_ix(side)];
        if !cache.file(path, side, hash).is_some_and(|f| f.covered(no)) {
            trigger = true;
        }
        plan.see(side, hash);
    }
    if !trigger {
        return Some(plan);
    }

    let (f0, f1) = zone(state, WINDOW);
    let mut need: [Option<(u32, u32)>; 2] = [None; 2];
    for (side, no) in lines_in(rows, f0, f1) {
        let e = &mut need[side_ix(side)];
        *e = Some(match *e {
            None => (no, no),
            Some((a, b)) => (a.min(no), b.max(no)),
        });
    }
    for side in SIDES {
        let Some((lo, hi)) = need[side_ix(side)] else { continue };
        let hash = memo.hashes[side_ix(side)];
        plan.see(side, hash);
        let file = cache.file(path, side, hash);
        for seg in segments(&stream(state, side)) {
            let seg_end = seg.first.saturating_add(seg.lines.len() as u32);
            let (a, b) = (lo.max(seg.first), hi.saturating_add(1).min(seg_end));
            if a >= b {
                continue;
            }
            let gaps = file.map_or_else(|| vec![(a, b)], |f| f.gaps(a, b));
            for (g0, g1) in gaps {
                let from = (g0 - seg.first) as usize;
                let to = (g1 - seg.first) as usize;
                let Some(slice) = seg.lines.get(from..to) else { continue };
                let carry = if g0 > seg.first { file.and_then(|f| f.carry_ending_at(g0)) } else { None };
                plan.requests.push(Request {
                    side,
                    hash,
                    start: g0,
                    lines: slice.iter().map(|s| (*s).to_string()).collect(),
                    carry,
                });
            }
        }
    }
    Some(plan)
}

fn has_fence(text: &str) -> bool {
    text.contains("```") || text.contains("~~~")
}

/// Fenced code lines of the threads to highlight, or `None` when threads and width are unchanged.
pub(super) fn plan_code(comments: &[Comment], bw: usize, last_sig: Option<u64>) -> Option<CodePlan> {
    let mut h = DefaultHasher::new();
    bw.hash(&mut h);
    for c in comments {
        c.id.hash(&mut h);
        crate::thread::turns_sig(c).hash(&mut h);
    }
    let sig = h.finish();
    if last_sig == Some(sig) {
        return None;
    }
    let mut lines = Vec::new();
    for c in comments {
        let fenced = has_fence(&c.message)
            || c.turns
                .iter()
                .any(|t| has_fence(&t.message) || t.answer.as_ref().is_some_and(|a| has_fence(&a.text)));
        if !fenced {
            continue;
        }
        for l in thread_body(c, bw) {
            if l.kind != BodyKind::Code || l.text.trim().is_empty() {
                continue;
            }
            if let Some(lang) = l.lang.as_deref().and_then(language_for_fence) {
                lines.push((lang, l.text));
            }
        }
    }
    Some(CodePlan { sig, lines })
}

/// Width thread code lines wrap at.
pub(super) fn code_width(state: &State) -> usize {
    box_width(state).saturating_sub(3).max(1)
}
