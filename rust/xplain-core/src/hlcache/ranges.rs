//! Computed line ranges of one file side: lookup, gap search, merging inserts.

use crate::comments::PaneSide;
use crate::highlight::{Carry, ClassRun, LineRuns};

#[derive(Debug, Clone)]
pub(super) struct Range {
    /// First line number.
    pub start: u32,
    pub lines: Vec<LineRuns>,
    /// Parser state after the last line (continues a following range).
    pub end: Option<Carry>,
}

impl Range {
    pub fn end_excl(&self) -> u32 {
        self.start.saturating_add(self.lines.len() as u32)
    }
}

#[derive(Debug, Clone)]
pub(super) struct FileHl {
    pub path: String,
    pub side: PaneSide,
    pub hash: u64,
    /// Sorted, non-overlapping, non-adjacent (adjacent ranges merge).
    pub ranges: Vec<Range>,
    /// Requested `[from, to)` line ranges not yet delivered.
    pub pending: Vec<(u32, u32)>,
}

impl FileHl {
    pub fn new(path: &str, side: PaneSide, hash: u64) -> FileHl {
        FileHl { path: path.to_string(), side, hash, ranges: Vec::new(), pending: Vec::new() }
    }

    pub fn line(&self, no: u32) -> Option<&[ClassRun]> {
        let i = self.ranges.partition_point(|r| r.end_excl() <= no);
        let r = self.ranges.get(i)?;
        if no < r.start {
            return None;
        }
        r.lines.get((no - r.start) as usize).map(Vec::as_slice)
    }

    pub fn covered(&self, no: u32) -> bool {
        self.line(no).is_some() || self.pending.iter().any(|(a, b)| no >= *a && no < *b)
    }

    /// Uncovered `[from, to)` pieces of `[a, b)` (not cached, not requested).
    pub fn gaps(&self, a: u32, b: u32) -> Vec<(u32, u32)> {
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

    pub fn carry_ending_at(&self, no: u32) -> Option<Carry> {
        self.ranges.iter().find(|r| r.end_excl() == no).and_then(|r| r.end.clone())
    }

    /// Add a delivered range; overlapping deliveries (races) are dropped, adjacent ranges merge.
    pub fn insert(&mut self, r: Range) {
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

#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::*;

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
}
