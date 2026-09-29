//! Syntax highlight cache: what is highlighted, what to ask for next, and where the view reads it.
//!
//! Spec: UNSPEC-31 (syntax colors are free, plain text until spans arrive is fine), F-ASK-06 (code blocks).
//! Owner: component `viewrows` (F2) with the shell hook in `update`.
//! Must not: parse on the render path (`view` only reads [`HlCache::runs`] / [`HlCache::code_runs`]), store
//! colors (spans hold token classes, the view maps them per theme), do IO.
//!
//! Files. Per (path, side) an entry holds computed line ranges (token classes per line). Sides: `New` =
//! context + added lines (browse: the whole file), `Old` = deleted lines. Line numbers are the side's own.
//! Key = path + side + content hash of that side's lines; entries with a changed hash are dropped when the
//! file is next planned (reload, `m`, `c`, MCP `files_changed`, browse open / re-read keep unchanged ones).
//! Only the current file plus the last three ([`RECENT_FILES`]) stay cached.
//!
//! Windowing. After every event [`sync`] looks at the rows near the viewport. When a line within
//! [`TRIGGER`] rows is not covered (cached or requested), it asks for every uncovered line within [`WINDOW`]
//! rows as `Effect::Highlight` chunks (one per contiguous gap of one segment: a hunk side, or the whole
//! browse file). A chunk that continues a cached range gets its saved parser state (`Carry`), everything
//! else starts fresh (jumps, upward extension, hunk starts). Results arrive as `Event::Highlighted`.
//! Files above [`MAX_LINES`] lines are never highlighted.
//!
//! Thread code blocks. Each code line of a fenced block is keyed by (language, text) in a small LRU
//! ([`CODE_CAP`] lines) computed once when the thread text changes (never per frame).

use std::collections::{HashMap, VecDeque};
use std::hash::{DefaultHasher, Hash, Hasher};

use crate::comments::PaneSide;
use crate::diff::LineKind;
use crate::effect::{Effect, Fx};
use crate::highlight::{Carry, ClassRun, LineRuns, highlight_one, language_for_fence, language_for_path};
use crate::rows::{RowLine, ShownRow};
use crate::state::{LoadState, State};
use crate::thread_layout::{BodyKind, box_width, thread_body};

/// Rows around the viewport that get highlighted.
pub const WINDOW: usize = 100;
/// Rows around the viewport that trigger a request when a line in them is not covered.
pub const TRIGGER: usize = 50;
/// Files with more lines than this stay plain.
pub const MAX_LINES: u32 = 50_000;
/// Most recently used files kept (current one included).
pub const RECENT_FILES: usize = 4;
/// Thread code lines kept.
pub const CODE_CAP: usize = 500;

/// Identifies one file side's content; results of older content (other hash) are dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HlKey {
    pub path: String,
    pub side: PaneSide,
    pub hash: u64,
}

#[derive(Debug, Clone)]
struct Range {
    /// First line number.
    start: u32,
    lines: Vec<LineRuns>,
    /// Parser state after the last line (continues a following range).
    end: Option<Carry>,
}

impl Range {
    fn end_excl(&self) -> u32 {
        self.start.saturating_add(self.lines.len() as u32)
    }
}

#[derive(Debug, Clone)]
struct FileHl {
    path: String,
    side: PaneSide,
    hash: u64,
    /// Sorted, non-overlapping, non-adjacent (adjacent ranges merge).
    ranges: Vec<Range>,
    /// Requested `[from, to)` line ranges not yet delivered.
    pending: Vec<(u32, u32)>,
}

impl FileHl {
    fn new(path: &str, side: PaneSide, hash: u64) -> FileHl {
        FileHl { path: path.to_string(), side, hash, ranges: Vec::new(), pending: Vec::new() }
    }

    fn line(&self, no: u32) -> Option<&[ClassRun]> {
        let i = self.ranges.partition_point(|r| r.end_excl() <= no);
        let r = self.ranges.get(i)?;
        if no < r.start {
            return None;
        }
        r.lines.get((no - r.start) as usize).map(Vec::as_slice)
    }

    fn covered(&self, no: u32) -> bool {
        self.line(no).is_some() || self.pending.iter().any(|(a, b)| no >= *a && no < *b)
    }

    /// Uncovered `[from, to)` pieces of `[a, b)` (not cached, not requested).
    fn gaps(&self, a: u32, b: u32) -> Vec<(u32, u32)> {
        let mut blocks: Vec<(u32, u32)> =
            self.ranges.iter().map(|r| (r.start, r.end_excl())).chain(self.pending.iter().copied()).collect();
        blocks.sort_unstable();
        let mut out = Vec::new();
        let mut cur = a;
        for (s, e) in blocks {
            if e <= cur {
                continue;
            }
            if s >= b {
                break;
            }
            if s > cur {
                out.push((cur, s));
            }
            cur = cur.max(e);
        }
        if cur < b {
            out.push((cur, b));
        }
        out
    }

    fn carry_ending_at(&self, no: u32) -> Option<Carry> {
        self.ranges.iter().find(|r| r.end_excl() == no).and_then(|r| r.end.clone())
    }

    /// Add a delivered range; overlapping deliveries (races) are dropped, adjacent ranges merge.
    fn insert(&mut self, r: Range) {
        if r.lines.is_empty() {
            return;
        }
        let end = r.end_excl();
        if self.ranges.iter().any(|o| o.start < end && r.start < o.end_excl()) {
            return;
        }
        let at = self.ranges.partition_point(|o| o.start < r.start);
        self.ranges.insert(at, r);
        if at + 1 < self.ranges.len() && self.ranges[at].end_excl() == self.ranges[at + 1].start {
            let next = self.ranges.remove(at + 1);
            self.ranges[at].lines.extend(next.lines);
            self.ranges[at].end = next.end;
        }
        if at > 0 && self.ranges[at - 1].end_excl() == self.ranges[at].start {
            let cur = self.ranges.remove(at);
            self.ranges[at - 1].lines.extend(cur.lines);
            self.ranges[at - 1].end = cur.end;
        }
    }
}

#[derive(Debug, Clone)]
struct CodeEntry {
    lang: &'static str,
    text: String,
    runs: LineRuns,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Memo {
    /// (files_gen, file_index, browse open).
    key: (u64, usize, bool),
    hashes: [u64; 2],
    big: bool,
}

/// Highlight cache in [`State`].
#[derive(Debug, Default)]
pub struct HlCache {
    files: Vec<FileHl>,
    /// Paths by recency, most recent last.
    recent: Vec<String>,
    memo: Option<Memo>,
    /// Bumped when a result lands, so the planner looks again.
    epoch: u64,
    last_plan: Option<PlanKey>,
    code: HashMap<u64, CodeEntry>,
    code_order: VecDeque<u64>,
    code_sig: Option<u64>,
}

type PlanKey = (u64, usize, usize, u16, usize, bool, u64);

fn side_ix(s: PaneSide) -> usize {
    match s {
        PaneSide::Old => 0,
        PaneSide::New => 1,
    }
}

const SIDES: [PaneSide; 2] = [PaneSide::Old, PaneSide::New];

/// Side and line number the row line takes its colors from: deleted lines from the old side, everything else
/// (context, added, browse) from the new side. `None` for the synthetic no-newline marker.
pub fn line_key(l: &RowLine) -> Option<(PaneSide, u32)> {
    if l.no_newline_marker {
        return None;
    }
    match l.kind {
        LineKind::Del => l.old_no.map(|n| (PaneSide::Old, n)),
        _ => l.new_no.map(|n| (PaneSide::New, n)).or_else(|| l.old_no.map(|n| (PaneSide::Old, n))),
    }
}

impl HlCache {
    /// Token classes of one line of the file side, `None` while not computed (draw plain).
    pub fn runs(&self, path: &str, side: PaneSide, no: u32) -> Option<&[ClassRun]> {
        self.files.iter().find(|f| f.side == side && f.path == path)?.line(no)
    }

    /// Token classes of one thread code line, `None` while not computed.
    pub fn code_runs(&self, lang: &str, text: &str) -> Option<&[ClassRun]> {
        let e = self.code.get(&code_hash(lang, text))?;
        (e.text == text && e.lang == lang).then_some(e.runs.as_slice())
    }

    /// Index of the entry for (path, side) with `hash`; an entry with other content is replaced.
    fn ensure(&mut self, path: &str, side: PaneSide, hash: u64) -> usize {
        match self.files.iter().position(|f| f.side == side && f.path == path) {
            Some(i) if self.files[i].hash == hash => i,
            Some(i) => {
                self.files[i] = FileHl::new(path, side, hash);
                i
            }
            None => {
                self.files.push(FileHl::new(path, side, hash));
                self.files.len() - 1
            }
        }
    }

    /// Make `path` the most recent file; drop entries of files beyond [`RECENT_FILES`].
    fn touch(&mut self, path: &str) {
        if self.recent.last().map(String::as_str) == Some(path) {
            return;
        }
        self.recent.retain(|p| p != path);
        self.recent.push(path.to_string());
        while self.recent.len() > RECENT_FILES {
            let old = self.recent.remove(0);
            self.files.retain(|f| f.path != old);
        }
    }

    fn code_insert(&mut self, lang: &'static str, text: &str) {
        let h = code_hash(lang, text);
        if let Some(e) = self.code.get(&h) {
            if e.text == text && e.lang == lang {
                self.code_order.retain(|k| *k != h);
                self.code_order.push_back(h);
                return;
            }
        }
        let runs = highlight_one(lang, text);
        self.code.insert(h, CodeEntry { lang, text: text.to_string(), runs });
        self.code_order.retain(|k| *k != h);
        self.code_order.push_back(h);
        while self.code_order.len() > CODE_CAP {
            if let Some(old) = self.code_order.pop_front() {
                self.code.remove(&old);
            }
        }
    }

    /// Test fixture: a computed range for (path, side).
    #[cfg(test)]
    pub(crate) fn seed(&mut self, path: &str, side: PaneSide, start: u32, lines: Vec<LineRuns>) {
        let i = self.ensure(path, side, 0);
        self.files[i].insert(Range { start, lines, end: None });
    }

    /// Number of cached thread code lines.
    pub fn code_len(&self) -> usize {
        self.code.len()
    }

    /// Number of file entries (path + side).
    pub fn file_entries(&self) -> usize {
        self.files.len()
    }

    /// Whether the file side has a cached, current line (tests and diagnostics).
    pub fn has_line(&self, path: &str, side: PaneSide, no: u32) -> bool {
        self.runs(path, side, no).is_some()
    }
}

fn code_hash(lang: &str, text: &str) -> u64 {
    let mut h = DefaultHasher::new();
    lang.hash(&mut h);
    text.hash(&mut h);
    h.finish()
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

/// Hook after every event (`update`): request highlights near the viewport, prepare thread code blocks.
pub fn sync(state: &mut State, fx: &mut Fx) {
    let mut cache = std::mem::take(&mut state.hl);
    plan_files(state, &mut cache, fx);
    plan_code(state, &mut cache);
    state.hl = cache;
}

fn plan_files(state: &State, cache: &mut HlCache, fx: &mut Fx) {
    if !matches!(state.load, LoadState::Ready) || state.rows.rows.is_empty() {
        return;
    }
    let Some(path) = current_path(state) else { return };
    let Some(lang) = language_for_path(path) else { return };
    let split = state.rows.key.as_ref().is_some_and(|k| k.split);
    let plan_key: PlanKey = (
        state.files_gen,
        state.nav.file_index,
        state.nav.top,
        state.size.rows,
        state.rows.rows.len(),
        split,
        cache.epoch,
    );
    if cache.last_plan == Some(plan_key) {
        return;
    }
    cache.last_plan = Some(plan_key);

    let mk = (state.files_gen, state.nav.file_index, state.browse.is_some());
    let memo = match cache.memo {
        Some(m) if m.key == mk => m,
        _ => {
            let (old, new) = (stream(state, PaneSide::Old), stream(state, PaneSide::New));
            let last = |s: &[(u32, &str)]| s.last().map_or(0, |(n, _)| *n);
            let m = Memo {
                key: mk,
                hashes: [hash_stream(&old), hash_stream(&new)],
                big: last(&old).max(last(&new)) > MAX_LINES,
            };
            cache.memo = Some(m);
            m
        }
    };
    if memo.big {
        return;
    }
    cache.touch(path);

    let rows = &state.rows.rows;
    let (t0, t1) = zone(state, TRIGGER);
    let mut trigger = false;
    for (side, no) in lines_in(rows, t0, t1) {
        let i = cache.ensure(path, side, memo.hashes[side_ix(side)]);
        if !cache.files[i].covered(no) {
            trigger = true;
        }
    }
    if !trigger {
        return;
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
        let stream = stream(state, side);
        let i = cache.ensure(path, side, hash);
        for seg in segments(&stream) {
            let seg_end = seg.first.saturating_add(seg.lines.len() as u32);
            let (a, b) = (lo.max(seg.first), hi.saturating_add(1).min(seg_end));
            if a >= b {
                continue;
            }
            for (g0, g1) in cache.files[i].gaps(a, b) {
                let from = (g0 - seg.first) as usize;
                let to = (g1 - seg.first) as usize;
                let Some(slice) = seg.lines.get(from..to) else { continue };
                let carry = if g0 > seg.first { cache.files[i].carry_ending_at(g0) } else { None };
                cache.files[i].pending.push((g0, g1));
                fx.push(Effect::Highlight {
                    key: HlKey { path: path.to_string(), side, hash },
                    lang: lang.to_string(),
                    start: g0,
                    lines: slice.iter().map(|s| (*s).to_string()).collect(),
                    carry,
                });
            }
        }
    }
}

fn has_fence(text: &str) -> bool {
    text.contains("```") || text.contains("~~~")
}

fn plan_code(state: &State, cache: &mut HlCache) {
    let bw = box_width(state).saturating_sub(3).max(1);
    let mut h = DefaultHasher::new();
    bw.hash(&mut h);
    for c in &state.comments {
        c.id.hash(&mut h);
        crate::thread::turns_sig(c).hash(&mut h);
    }
    let sig = h.finish();
    if cache.code_sig == Some(sig) {
        return;
    }
    cache.code_sig = Some(sig);
    for c in &state.comments {
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
                cache.code_insert(lang, &l.text);
            }
        }
    }
}

/// `Event::Highlighted`: store the range when the content still matches; a result for replaced content is dropped.
pub fn on_highlighted(state: &mut State, key: &HlKey, start: u32, runs: Vec<LineRuns>, end: Option<Carry>) {
    let c = &mut state.hl;
    c.epoch += 1;
    let Some(f) = c.files.iter_mut().find(|f| f.side == key.side && f.path == key.path && f.hash == key.hash)
    else {
        return;
    };
    f.pending.retain(|(a, _)| *a != start);
    f.insert(Range { start, lines: runs, end });
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::diff::RawDiff;
    use crate::event::{Event, ReqId};
    use crate::highlight::highlight_lines;
    use crate::keys::KeyEvent;
    use crate::screen::Size;
    use crate::state::testutil::{fake_state, k};
    use crate::theme::SyntaxClass;
    use crate::update::update;

    fn rust_file(n: usize, tag: &str) -> String {
        let body: String = (1..=n).map(|i| format!("+fn f{i}() {{ let x = \"{tag}\"; }}\n")).collect();
        format!(
            "diff --git a/a.rs b/a.rs\nnew file mode 100644\n--- /dev/null\n+++ b/a.rs\n@@ -0,0 +1,{n} @@\n{body}"
        )
    }

    fn loaded(n: usize, tag: &str) -> State {
        let mut s = fake_state();
        s.size = Size { cols: 80, rows: 24 };
        let raw = RawDiff { tracked: rust_file(n, tag), untracked: vec![] };
        let req = s.diff_req.unwrap_or(ReqId(1));
        let fx = update(&mut s, Event::DiffLoaded { req, result: Ok(raw) });
        run_fx(&mut s, fx);
        s
    }

    /// Answer every Highlight effect as the executor would, until the planner asks for nothing more.
    fn run_fx(s: &mut State, fx: Vec<Effect>) -> usize {
        let mut total = 0;
        let mut queue = fx;
        while !queue.is_empty() {
            let mut next = Vec::new();
            for e in queue {
                if let Effect::Highlight { key, lang, start, lines, carry } = e {
                    total += 1;
                    let (runs, end) = highlight_lines(&lang, &lines, carry.as_ref());
                    next.extend(update(s, Event::Highlighted { key, start, runs, end }));
                }
            }
            queue = next;
        }
        total
    }

    fn key(s: &mut State, c: char) -> Vec<Effect> {
        update(s, Event::Key(k(c)))
    }

    fn highlights(fx: &[Effect]) -> Vec<(u32, usize, bool)> {
        fx.iter()
            .filter_map(|e| match e {
                Effect::Highlight { start, lines, carry, .. } => Some((*start, lines.len(), carry.is_some())),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn window_is_limited_around_viewport() {
        let s = loaded(1000, "a");
        assert!(s.hl.has_line("a.rs", PaneSide::New, 1));
        assert!(s.hl.has_line("a.rs", PaneSide::New, 100));
        assert!(!s.hl.has_line("a.rs", PaneSide::New, 400));
        assert!(!s.hl.has_line("a.rs", PaneSide::New, 1000));
    }

    #[test]
    fn scrolling_down_extends_range_with_carried_state() {
        let mut s = loaded(1000, "a");
        let mut all = Vec::new();
        for _ in 0..400 {
            all.extend(highlights(&key(&mut s, 'j')));
            // answer as we go
        }
        // nothing answered: pending covers, so requests are chunked, never overlapping
        let mut covered = vec![false; 2000];
        for (start, len, _) in &all {
            for n in *start..*start + *len as u32 {
                assert!(!covered[n as usize], "line {n} requested twice");
                covered[n as usize] = true;
            }
        }
    }

    #[test]
    fn extension_below_uses_carry_of_cached_range() {
        let mut s = loaded(1000, "a");
        let mut hs = Vec::new();
        for _ in 0..120 {
            let fx = key(&mut s, 'j');
            hs.extend(highlights(&fx));
            run_fx(&mut s, fx);
        }
        assert!(!hs.is_empty());
        assert!(hs.iter().all(|(_, _, carry)| *carry), "extending down continues cached range: {hs:?}");
        assert!(s.hl.has_line("a.rs", PaneSide::New, 200));
        assert!(s.hl.has_line("a.rs", PaneSide::New, 1), "one merged range");
    }

    #[test]
    fn jump_starts_fresh_near_target() {
        let mut s = loaded(1000, "a");
        let fx = key(&mut s, 'G');
        let hs = highlights(&fx);
        assert!(!hs.is_empty());
        assert!(hs.iter().all(|(start, _, carry)| !*carry && *start > 800), "{hs:?}");
        run_fx(&mut s, fx);
        assert!(s.hl.has_line("a.rs", PaneSide::New, 1000));
        // the top of the file is still cached (LRU is per file, not per range)
        assert!(s.hl.has_line("a.rs", PaneSide::New, 1));
    }

    #[test]
    fn covered_lines_request_nothing() {
        let mut s = loaded(300, "a");
        let fx = key(&mut s, 'j');
        assert!(highlights(&fx).is_empty());
    }

    #[test]
    fn theme_change_needs_no_new_request() {
        let mut s = loaded(50, "a");
        let before = s.hl.runs("a.rs", PaneSide::New, 1).map(<[ClassRun]>::to_vec);
        assert!(before.is_some_and(|r| r.iter().any(|x| x.class == Some(SyntaxClass::Keyword))));
        s.settings.theme = crate::theme::ThemeId::Vibrant;
        let mut fx = Vec::new();
        sync(&mut s, &mut fx);
        assert!(fx.is_empty());
        assert!(s.hl.has_line("a.rs", PaneSide::New, 1));
    }

    /// `r` reload answered with `tracked`.
    fn reload(s: &mut State, tracked: String) -> Vec<Effect> {
        let fx = key(s, 'r');
        let req = fx
            .iter()
            .find_map(|e| match e {
                Effect::LoadDiff { req, .. } => Some(*req),
                _ => None,
            })
            .expect("load request");
        update(s, Event::DiffLoaded { req, result: Ok(RawDiff { tracked, untracked: vec![] }) })
    }

    #[test]
    fn reload_keeps_unchanged_and_drops_changed() {
        let mut s = loaded(50, "same");
        assert!(s.hl.has_line("a.rs", PaneSide::New, 1));
        let fx = reload(&mut s, rust_file(50, "same"));
        assert!(highlights(&fx).is_empty(), "unchanged content keeps its entry");
        assert!(s.hl.has_line("a.rs", PaneSide::New, 1));
        let fx = reload(&mut s, rust_file(50, "other"));
        assert!(!s.hl.has_line("a.rs", PaneSide::New, 1), "changed content drops the entry");
        assert!(!highlights(&fx).is_empty());
    }

    #[test]
    fn reload_with_appended_line_rehighlights() {
        // an appended line changes the new-side hash
        let mut s = loaded(50, "same");
        let fx = reload(&mut s, rust_file(51, "same"));
        assert!(!highlights(&fx).is_empty());
    }

    #[test]
    fn stale_result_for_old_content_is_dropped() {
        let mut s = loaded(50, "a");
        let stale = HlKey { path: "a.rs".into(), side: PaneSide::New, hash: 1 };
        let n = s.hl.file_entries();
        update(&mut s, Event::Highlighted { key: stale, start: 1, runs: vec![vec![]; 3], end: None });
        assert_eq!(s.hl.file_entries(), n);
        assert!(s.hl.runs("a.rs", PaneSide::New, 1).is_some_and(|r| !r.is_empty()));
    }

    #[test]
    fn only_current_plus_three_recent_files_stay() {
        let mut c = HlCache::default();
        for p in ["a.rs", "b.rs", "c.rs", "d.rs"] {
            c.ensure(p, PaneSide::New, 1);
            c.touch(p);
        }
        assert_eq!(c.file_entries(), 4);
        c.ensure("e.rs", PaneSide::New, 1);
        c.touch("e.rs");
        assert_eq!(c.file_entries(), 4);
        assert!(c.runs("a.rs", PaneSide::New, 1).is_none());
        // touching an older one keeps it and drops the least recent instead
        c.touch("c.rs");
        c.ensure("f.rs", PaneSide::New, 1);
        c.touch("f.rs");
        assert!(c.files.iter().any(|f| f.path == "c.rs"));
        assert!(!c.files.iter().any(|f| f.path == "b.rs"));
    }

    #[test]
    fn ranges_merge_and_gaps_skip_pending() {
        let mut f = FileHl::new("x", PaneSide::New, 1);
        let r = |start: u32, n: usize| Range { start, lines: vec![vec![]; n], end: None };
        f.insert(r(10, 5));
        f.insert(r(20, 5));
        assert_eq!(f.ranges.len(), 2);
        f.insert(r(15, 5));
        assert_eq!(f.ranges.len(), 1);
        assert_eq!((f.ranges[0].start, f.ranges[0].end_excl()), (10, 25));
        f.insert(r(12, 2));
        assert_eq!(f.ranges.len(), 1, "overlap dropped");
        f.pending.push((30, 35));
        assert_eq!(f.gaps(5, 40), vec![(5, 10), (25, 30), (35, 40)]);
    }

    #[test]
    fn huge_files_stay_plain() {
        let mut s = fake_state();
        let n = MAX_LINES as usize + 1;
        let raw = RawDiff { tracked: rust_file(n, "a"), untracked: vec![] };
        let req = s.diff_req.unwrap_or(ReqId(1));
        let fx = update(&mut s, Event::DiffLoaded { req, result: Ok(raw) });
        assert!(highlights(&fx).is_empty());
        let fx = key(&mut s, 'G');
        assert!(highlights(&fx).is_empty());
    }

    #[test]
    fn unknown_language_asks_nothing() {
        let mut s = fake_state();
        let raw = RawDiff {
            tracked: "diff --git a/a.zzz b/a.zzz\n--- a/a.zzz\n+++ b/a.zzz\n@@ -1 +1 @@\n-y\n+z\n".into(),
            untracked: vec![],
        };
        let req = s.diff_req.unwrap_or(ReqId(1));
        let fx = update(&mut s, Event::DiffLoaded { req, result: Ok(raw) });
        assert!(highlights(&fx).is_empty());
    }

    #[test]
    fn changes_scope_hunks_are_separate_segments_without_carry() {
        let mut s = fake_state();
        let raw = RawDiff {
            tracked: "diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ -1,2 +1,2 @@\n /* c\n-x\n+y\n@@ -50,2 +50,2 @@\n z\n-q\n+w\n"
                .into(),
            untracked: vec![],
        };
        let req = s.diff_req.unwrap_or(ReqId(1));
        let fx = update(&mut s, Event::DiffLoaded { req, result: Ok(raw) });
        let hs = highlights(&fx);
        // new side: two hunks; old side: two delete runs
        let starts: Vec<u32> = hs.iter().map(|h| h.0).collect();
        assert!(starts.contains(&1) && starts.contains(&50), "{starts:?}");
        assert!(hs.iter().all(|h| !h.2));
    }

    #[test]
    fn browse_uses_whole_file_and_shares_key_with_full_scope() {
        let mut s = fake_state();
        s.size = Size { cols: 80, rows: 24 };
        let lines: Vec<String> = (1..=30).map(|i| format!("fn f{i}() {{}}")).collect();
        s.browse = Some(crate::state::Browse { path: "b.rs".into(), lines });
        s.load = LoadState::Ready;
        s.files_gen += 1;
        crate::rows::ensure(&mut s);
        let mut fx = Vec::new();
        sync(&mut s, &mut fx);
        let hs = highlights(&fx);
        assert_eq!(hs, vec![(1, 30, false)]);
    }

    #[test]
    fn thread_code_lines_cached_once_with_lru() {
        let mut c = HlCache::default();
        for i in 0..(CODE_CAP + 20) {
            c.code_insert("rust", &format!("let x{i} = 1;"));
        }
        assert_eq!(c.code_len(), CODE_CAP);
        assert!(c.code_runs("rust", "let x0 = 1;").is_none(), "oldest evicted");
        assert!(c.code_runs("rust", &format!("let x{} = 1;", CODE_CAP + 19)).is_some());
        // touching keeps an entry alive
        c.code_insert("rust", "let x25 = 1;");
        for i in 0..CODE_CAP - 2 {
            c.code_insert("rust", &format!("y{i}"));
        }
        assert!(c.code_runs("rust", "let x25 = 1;").is_some());
        // keyed by language and text
        assert!(c.code_runs("python", "let x25 = 1;").is_none());
    }

    #[test]
    fn thread_code_planned_on_arrival_not_per_frame() {
        let mut s = fake_state();
        s.size = Size { cols: 80, rows: 24 };
        s.comments.push(crate::comments::testutil::comment("c1", 1, 1, "look\n```rust\nlet a = 1;\n```"));
        let mut fx = Vec::new();
        sync(&mut s, &mut fx);
        assert!(fx.is_empty());
        assert!(s.hl.code_runs("rust", "let a = 1;").is_some_and(|r| !r.is_empty()));
        let _ = KeyEvent::ch('x');
    }
}
