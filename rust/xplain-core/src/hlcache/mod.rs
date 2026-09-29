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
//!
//! Layout. `ranges` (per-side computed ranges), `code_lru` (thread code lines), `plan` (pure planner over
//! `&State`; [`HlCache`] applies its plans). This file holds the cache, the shell hook and result intake.

mod code_lru;
mod plan;
mod ranges;

use crate::comments::PaneSide;
use crate::diff::LineKind;
use crate::effect::{Effect, Fx};
use crate::highlight::{Carry, ClassRun, LineRuns};
use crate::rows::RowLine;
use crate::state::State;

use code_lru::CodeLru;
use plan::{CodePlan, FilePlan, Planner};
use ranges::{FileHl, Range};

/// Rows around the viewport that get highlighted.
const WINDOW: usize = 100;
/// Rows around the viewport that trigger a request when a line in them is not covered.
const TRIGGER: usize = 50;
/// Files with more lines than this stay plain.
const MAX_LINES: u32 = 50_000;
/// Most recently used files kept (current one included).
const RECENT_FILES: usize = 4;

/// Identifies one file side's content; results of older content (other hash) are dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HlKey {
    pub path: String,
    pub side: PaneSide,
    pub hash: u64,
}

/// Highlight cache in [`State`].
#[derive(Debug, Default)]
pub struct HlCache {
    files: Vec<FileHl>,
    /// Paths by recency, most recent last.
    recent: Vec<String>,
    /// Bumped when a result lands, so the planner looks again.
    epoch: u64,
    planner: Planner,
    code: CodeLru,
}

fn side_ix(s: PaneSide) -> usize {
    match s {
        PaneSide::Old => 0,
        PaneSide::New => 1,
    }
}

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
        self.code.get(lang, text)
    }

    /// The entry for (path, side) when it holds content with `hash`.
    fn file(&self, path: &str, side: PaneSide, hash: u64) -> Option<&FileHl> {
        self.files.iter().find(|f| f.side == side && f.path == path && f.hash == hash)
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

    fn apply_files(&mut self, plan: FilePlan, fx: &mut Fx) {
        self.planner.last_plan = Some(plan.key);
        self.planner.memo = Some(plan.memo);
        if plan.touch {
            self.touch(&plan.path);
        }
        for (side, hash) in &plan.seen {
            self.ensure(&plan.path, *side, *hash);
        }
        for r in plan.requests {
            let i = self.ensure(&plan.path, r.side, r.hash);
            if let Some(f) = self.files.get_mut(i) {
                f.pending.push((r.start, r.start.saturating_add(r.lines.len() as u32)));
            }
            fx.push(Effect::Highlight {
                key: HlKey { path: plan.path.clone(), side: r.side, hash: r.hash },
                lang: plan.lang.clone(),
                start: r.start,
                lines: r.lines,
                carry: r.carry,
            });
        }
    }

    fn apply_code(&mut self, plan: CodePlan) {
        self.planner.code_sig = Some(plan.sig);
        for (lang, text) in &plan.lines {
            self.code.insert(lang, text);
        }
    }

    /// Test fixture: a computed range for (path, side).
    #[cfg(test)]
    pub(crate) fn seed(&mut self, path: &str, side: PaneSide, start: u32, lines: Vec<LineRuns>) {
        let i = self.ensure(path, side, 0);
        self.files[i].insert(Range { start, lines, end: None });
    }

    /// Number of file entries (path + side).
    #[cfg(test)]
    fn file_entries(&self) -> usize {
        self.files.len()
    }

    /// Whether the file side has a cached, current line.
    #[cfg(test)]
    fn has_line(&self, path: &str, side: PaneSide, no: u32) -> bool {
        self.runs(path, side, no).is_some()
    }
}

/// Hook after every event (`update`): request highlights near the viewport, prepare thread code blocks.
pub fn sync(state: &mut State, fx: &mut Fx) {
    if let Some(plan) = plan::plan_files(state) {
        state.hl.apply_files(plan, fx);
    }
    let bw = plan::code_width(state);
    if let Some(plan) = plan::plan_code(&state.comments, bw, state.hl.planner.code_sig) {
        state.hl.apply_code(plan);
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
    use crate::state::LoadState;
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
        let req = s.loader.diff_req.unwrap_or(ReqId(1));
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
    fn huge_files_stay_plain() {
        let mut s = fake_state();
        let n = MAX_LINES as usize + 1;
        let raw = RawDiff { tracked: rust_file(n, "a"), untracked: vec![] };
        let req = s.loader.diff_req.unwrap_or(ReqId(1));
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
        let req = s.loader.diff_req.unwrap_or(ReqId(1));
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
        let req = s.loader.diff_req.unwrap_or(ReqId(1));
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
