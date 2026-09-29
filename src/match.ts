// fzf-style path matching for file search: hit = query chars in order (gaps allowed), all chars literal.
// Smart case: all-lowercase query matches case-insensitively, any uppercase char makes it case-sensitive.
export type PathHit = {path: string; idx: number[]}; // idx: matched char positions (code points)

const lower = (c: string) => c.toLowerCase();
const SEP = '/';
const DELIM = '_-. ';

// bonus for a match at i: path/word boundaries and camelCase humps score higher
const boundary = (t: string[], i: number) => {
	if (i === 0) return 10;
	const p = t[i - 1]!;
	if (p === SEP) return 10;
	if (DELIM.includes(p)) return 8;
	if (p !== lower(p) || lower(t[i]!) === t[i]) return 0;
	return 7; // lower -> Upper
};

// one placement per hit: greedy forward to the end of the first match, then back to its latest start
// (shortest window ending there), fzf v1 style
const place = (t: string[], q: string[], eq: (a: string, b: string) => boolean): number[] | undefined => {
	let j = 0;
	let end = -1;
	for (let i = 0; i < t.length && j < q.length; i++) if (eq(t[i]!, q[j]!) && ++j === q.length) end = i;
	if (end < 0) return undefined;
	const idx: number[] = [];
	j = q.length - 1;
	for (let i = end; j >= 0; i--)
		if (eq(t[i]!, q[j]!)) {
			idx.push(i);
			j--;
		}
	return idx.reverse();
};

const score = (t: string[], idx: number[]) => {
	let s = 0;
	idx.forEach((i, k) => {
		const p = k ? idx[k - 1]! : -1;
		s += 16 + boundary(t, i);
		if (k && i === p + 1) s += 8;
		else if (k) s -= 3 + (i - p - 2);
	});
	return s;
};

export function matchPaths(query: string, paths: readonly string[]): PathHit[] {
	if (!query) return paths.map((path) => ({path, idx: []}));
	const cs = query !== lower(query);
	const eq = cs ? (a: string, b: string) => a === b : (a: string, b: string) => lower(a) === lower(b);
	const q = [...query];
	const scored: (PathHit & {exact: boolean; score: number; n: number})[] = [];
	paths.forEach((path, n) => {
		const t = [...path];
		const idx = place(t, q, eq);
		if (!idx) return;
		const exact = t.length === q.length && t.every((c, i) => eq(c, q[i]!));
		scored.push({path, idx, exact, score: score(t, idx), n});
	});
	scored.sort(
		(a, b) => Number(b.exact) - Number(a.exact) || b.score - a.score || a.path.length - b.path.length || a.n - b.n,
	);
	return scored.map(({path, idx}) => ({path, idx}));
}
