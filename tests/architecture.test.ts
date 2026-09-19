import {readdirSync, readFileSync} from 'node:fs';
import {join, relative, sep} from 'node:path';

let fail = 0;
const ok = (n: string, c: boolean) => {
	console.log(c ? 'PASS' : 'FAIL', n);
	if (!c) fail++;
};

const src = join(import.meta.dirname, '..', 'src');
const walk = (d: string): string[] =>
	readdirSync(d, {withFileTypes: true}).flatMap((e) => (e.isDirectory() ? walk(join(d, e.name)) : [join(d, e.name)]));
const rel = (f: string) => relative(src, f).split(sep).join('/');
const all = walk(src);
const scanned = all.filter((f) => !rel(f).startsWith('integrations/'));
const lines = (f: string) => readFileSync(f, 'utf8').split('\n');

// (a) agent names only in src/integrations/**
const NAMES = /claude|copilot|codex|opencode/i;
const hits: string[] = [];
for (const f of scanned) lines(f).forEach((l, i) => NAMES.test(l) && hits.push(`${rel(f)}:${i + 1}: ${l.trim()}`));
for (const h of hits) console.log('offender', h);
ok('scanned some files', scanned.length > 5);
ok('no agent names outside src/integrations', hits.length === 0);

// (b) src/ask has no react/ink imports
const RI = /(?:from|import)\s*\(?\s*['"](?:react|ink)(?:[/'"])/;
const ask = all.filter((f) => rel(f).startsWith('ask/'));
const bad2 = ask.flatMap((f) => lines(f).flatMap((l, i) => (RI.test(l) ? [`${rel(f)}:${i + 1}: ${l.trim()}`] : [])));
for (const h of bad2) console.log('offender', h);
ok('found ask files', ask.length > 0);
ok('src/ask does not import react/ink', bad2.length === 0);

process.exit(fail ? 1 : 0);
