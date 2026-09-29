import {matchPaths} from '../src/match.js';

let fail = 0;
const ok = (n: string, c: boolean) => {
	console.log(c ? 'PASS' : 'FAIL', n);
	if (!c) fail++;
};
const paths = (q: string, ps: string[]) => matchPaths(q, ps).map((h) => h.path);
const eq = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);

const P = ['README.md', 'big', 'notes/readme.md', 'package.json', 'src/a.ts', 'src/a.tsx', 'src/big.ts'];
ok('empty query: all, input order', eq(paths('', P), P));
ok(
	'empty query: no matched chars',
	matchPaths('', P).every((h) => h.idx.length === 0),
);
ok('subsequence with gaps', eq(paths('pkg', P), ['package.json']));
ok('scattered subsequence', eq(paths('rdm', P).sort(), ['README.md', 'notes/readme.md']));
ok('out of order: no hit', eq(paths('tbs', P), []));
ok('lowercase query: case-insensitive', eq(paths('rm', P).sort(), ['README.md', 'notes/readme.md']));
ok('uppercase query: case-sensitive', eq(paths('RM', P), ['README.md']));
ok('mixed case r then M: no hit', eq(paths('rM', P), []));
ok('uppercase query misses lowercase path', eq(paths('BIG', P), []));
ok('exact path first', paths('big', P)[0] === 'big' && paths('big', P).length === 2);
ok('exact path first, case-insensitive rule', paths('readme.md', P)[0] === 'README.md');
ok('exact path, case-sensitive rule', eq(paths('README.md', P), ['README.md']));
ok('exact beats prefix match', eq(paths('src/a.ts', P), ['src/a.ts', 'src/a.tsx']));
ok('space literal', eq(paths('a b', ['doc/a b.md', 'a/b', 'ab']), ['doc/a b.md']));
ok("specials literal ^ $ ! | '", eq(paths('^y$', ['x^y$z', 'yy', 'y$']), ['x^y$z']));
ok('^ is no anchor', eq(paths('^s', ['src/a.ts']), []));
ok('$ is no anchor', eq(paths('.md$', ['a.md']), []));
ok('! is no negation', eq(paths('!x', ['a.ts']), []));
ok('| is no or', eq(paths('a|b', ['a', 'b', 'a|b']), ['a|b']));
ok("' is no exact prefix", eq(paths("'ab", ['ab', "x'ab"]), ["x'ab"]));
const h = matchPaths('big', ['src/big.ts'])[0]!;
ok('matched char positions', eq(h.idx, [4, 5, 6]));
const h2 = matchPaths('st', ['s-x-st'])[0]!;
ok('shortest window ending at first match end', eq(h2.idx, [4, 5]));
ok('consecutive / word start ranks higher', paths('ab', ['xaxb', 'x/ab'])[0] === 'x/ab');
ok('shorter path wins ties', eq(paths('ab', ['q/ab.ts', 'q/ab']), ['q/ab', 'q/ab.ts']));
ok('code points: index by char', eq(matchPaths('b', ['é/b'])[0]!.idx, [2]));
process.exit(fail ? 1 : 0);
