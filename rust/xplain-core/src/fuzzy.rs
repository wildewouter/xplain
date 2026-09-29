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

/// `c` is its own lowercase form (JS `c.toLowerCase() === c`).
fn is_lower_stable(c: char) -> bool {
    if c.is_ascii() { !c.is_ascii_uppercase() } else { c.to_lowercase().eq(std::iter::once(c)) }
}

/// Case-insensitive equality by full lowercase mapping (JS `a.toLowerCase() === b.toLowerCase()`), no allocation.
fn eq_ignore_case(a: char, b: char) -> bool {
    if a.is_ascii() && b.is_ascii() {
        a.eq_ignore_ascii_case(&b)
    } else {
        a.to_lowercase().eq(b.to_lowercase())
    }
}

const DELIM: &str = "_-. ";

/// Bonus for a match at `i`: path/word boundaries and camelCase humps score higher.
fn boundary(t: &[char], i: usize) -> i64 {
    if i == 0 {
        return 10;
    }
    let p = t[i - 1];
    if p == '/' {
        return 10;
    }
    if DELIM.contains(p) {
        return 8;
    }
    let c = t[i];
    if !is_lower_stable(p) || is_lower_stable(c) {
        return 0;
    }
    7
}

/// One placement per hit: greedy forward to the end of the first match, then back to its latest start.
fn place(t: &[char], q: &[char], eq: &dyn Fn(char, char) -> bool) -> Option<Vec<usize>> {
    if q.is_empty() {
        return Some(Vec::new());
    }
    let mut j = 0;
    let mut end = None;
    for (i, &c) in t.iter().enumerate() {
        if j >= q.len() {
            break;
        }
        if eq(c, q[j]) {
            j += 1;
            if j == q.len() {
                end = Some(i);
            }
        }
    }
    let end = end?;
    let mut idx = Vec::with_capacity(q.len());
    let mut j = q.len();
    let mut i = end as i64;
    while j > 0 && i >= 0 {
        if eq(t[i as usize], q[j - 1]) {
            idx.push(i as usize);
            j -= 1;
        }
        i -= 1;
    }
    idx.reverse();
    Some(idx)
}

fn score(t: &[char], idx: &[usize]) -> i64 {
    let mut s = 0i64;
    for (k, &i) in idx.iter().enumerate() {
        s += 16 + boundary(t, i);
        if k > 0 {
            let p = idx[k - 1];
            if i == p + 1 {
                s += 8;
            } else {
                s -= 3 + (i as i64 - p as i64 - 2);
            }
        }
    }
    s
}

/// Empty query returns all paths in input order with empty `idx`. Otherwise smart-case subsequence match,
/// ranked exact > score > shorter path > input order (same algorithm as `matchPaths` in match.ts).
pub fn match_paths(query: &str, paths: &[String]) -> Vec<PathHit> {
    if query.is_empty() {
        return paths.iter().map(|p| PathHit { path: p.clone(), idx: Vec::new() }).collect();
    }
    let case_sensitive = query != query.to_lowercase();
    let eq = move |a: char, b: char| if case_sensitive { a == b } else { eq_ignore_case(a, b) };
    let q: Vec<char> = query.chars().collect();
    struct Scored {
        path: String,
        idx: Vec<usize>,
        exact: bool,
        score: i64,
        len16: usize,
        n: usize,
    }
    let mut scored: Vec<Scored> = Vec::new();
    for (n, path) in paths.iter().enumerate() {
        let t: Vec<char> = path.chars().collect();
        let Some(idx) = place(&t, &q, &eq) else { continue };
        let exact = t.len() == q.len() && t.iter().zip(&q).all(|(&a, &b)| eq(a, b));
        let score = score(&t, &idx);
        scored.push(Scored { path: path.clone(), idx, exact, score, len16: path.encode_utf16().count(), n });
    }
    scored.sort_by(|a, b| {
        b.exact.cmp(&a.exact).then(b.score.cmp(&a.score)).then(a.len16.cmp(&b.len16)).then(a.n.cmp(&b.n))
    });
    scored.into_iter().map(|s| PathHit { path: s.path, idx: s.idx }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }
    fn paths(q: &str, a: &[&str]) -> Vec<String> {
        match_paths(q, &v(a)).into_iter().map(|h| h.path).collect()
    }

    #[test]
    fn f_search_01_empty_query_keeps_order() {
        let r = match_paths("", &v(&["b", "a"]));
        assert_eq!(r.iter().map(|h| h.path.as_str()).collect::<Vec<_>>(), ["b", "a"]);
        assert!(r.iter().all(|h| h.idx.is_empty()));
    }

    #[test]
    fn f_search_01_subsequence_and_idx() {
        let r = match_paths("sra", &v(&["src/a.rs", "xyz"]));
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].idx, vec![0, 1, 4]);
    }

    #[test]
    fn f_search_01_window_is_shortest_ending_at_first_match() {
        // greedy end at 'b' index 3; back-walk picks latest start
        let r = match_paths("ab", &v(&["a_a_b"]));
        assert_eq!(r[0].idx, vec![2, 4]);
    }

    #[test]
    fn f_search_01_smart_case() {
        assert_eq!(paths("abc", &["ABC", "abc"]), ["ABC", "abc"]);
        assert_eq!(paths("Abc", &["abc", "Abc"]), ["Abc"]);
        assert!(paths("aBc", &["abc"]).is_empty());
    }

    #[test]
    fn f_search_01_exact_first_then_score() {
        assert_eq!(paths("a.ts", &["x/a.ts.bak", "a.ts", "b/a.ts"]), ["a.ts", "b/a.ts", "x/a.ts.bak"]);
    }

    #[test]
    fn f_search_01_boundary_and_camel() {
        assert_eq!(boundary(&"foo/bar".chars().collect::<Vec<_>>(), 4), 10);
        assert_eq!(boundary(&"foo_bar".chars().collect::<Vec<_>>(), 4), 8);
        assert_eq!(boundary(&"fooBar".chars().collect::<Vec<_>>(), 3), 7);
        assert_eq!(boundary(&"foobar".chars().collect::<Vec<_>>(), 3), 0);
        assert_eq!(boundary(&"foobar".chars().collect::<Vec<_>>(), 0), 10);
    }

    #[test]
    fn f_search_01_ties_shorter_then_input_order() {
        assert_eq!(paths("a", &["bba", "ba", "ca"]), ["ba", "ca", "bba"]);
    }

    #[test]
    fn f_search_01_code_point_idx() {
        let r = match_paths("b", &v(&["\u{e9}\u{1F600}b"]));
        assert_eq!(r[0].idx, vec![2]);
    }

    #[test]
    fn f_search_01_non_ascii_case_folding() {
        assert!(eq_ignore_case('\u{c9}', '\u{e9}'));
        assert!(eq_ignore_case('K', '\u{212a}'));
        assert!(!eq_ignore_case('a', 'b'));
        assert_eq!(paths("\u{e9}", &["\u{c9}a", "b"]), ["\u{c9}a"]);
        assert!(is_lower_stable('a') && is_lower_stable('_') && is_lower_stable('\u{e9}'));
        assert!(!is_lower_stable('A') && !is_lower_stable('\u{c9}'));
    }

    #[test]
    fn f_search_01_no_match() {
        assert!(paths("zz", &["a", "b"]).is_empty());
    }
}
