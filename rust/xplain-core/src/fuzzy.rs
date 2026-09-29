//! fzf-style path matcher for file search.
//!
//! Spec: F-SEARCH-01 (matching, smart case, ordering, highlight indices). Oracle: `src/match.ts`.
//! Owner: component `parse` (A). Pure, no state.
//! Must not: know about UI state or the search modal.

/// One hit: the path and matched char (code point) positions in it, ascending.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathHit {
    pub path: String,
    pub idx: Vec<usize>,
}

/// Empty query returns all paths in input order with empty `idx`. Otherwise smart-case subsequence match,
/// ranked exact > score > shorter path > input order (same algorithm as `matchPaths` in match.ts).
pub fn match_paths(_query: &str, _paths: &[String]) -> Vec<PathHit> {
    todo!("F-SEARCH-01")
}
