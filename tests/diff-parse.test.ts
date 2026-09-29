import {execFileSync} from 'node:child_process';
import {mkdirSync, mkdtempSync, rmSync, unlinkSync, writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {loadDiff, parse, type DiffFile} from '../src/diff/load.js';
import {status} from '../src/components/FileModal.js';

let fail = 0;
const ok = (n: string, c: boolean, extra?: unknown) => {
	console.log(c ? 'PASS' : 'FAIL', n, c || extra === undefined ? '' : JSON.stringify(extra));
	if (!c) fail++;
};

// ---- parse: synthetic git output
{
	const raw = [
		'diff --git a/b/a/x.txt b/b/a/x.txt',
		'index 1..2 100644',
		'--- a/b/a/x.txt',
		'+++ b/b/a/x.txt',
		'@@ -1 +1 @@',
		'-old',
		'+new',
		'diff --git a/a/y.txt b/a/y.txt',
		'new file mode 100644',
		'index 0..1',
		'--- /dev/null',
		'+++ b/a/y.txt',
		'@@ -0,0 +1 @@',
		'+y',
		'diff --git a/b/a/w.txt b/b/a/w.txt',
		'deleted file mode 100644',
		'index 1..0',
		'--- a/b/a/w.txt',
		'+++ /dev/null',
		'@@ -1 +0,0 @@',
		'-w',
		'diff --git a/a/old.txt b/b/new.txt',
		'similarity index 100%',
		'rename from a/old.txt',
		'rename to b/new.txt',
		'diff --git "a/q \\"x\\"\\t\\303\\251.txt" "b/q \\"x\\"\\t\\303\\251.txt"',
		'index 1..2 100644',
		'--- "a/q \\"x\\"\\t\\303\\251.txt"',
		'+++ "b/q \\"x\\"\\t\\303\\251.txt"',
		'@@ -1 +1 @@',
		'-1',
		'+2',
		'diff --git a/sp ace.txt b/sp ace.txt',
		'index 1..2 100644',
		'--- a/sp ace.txt\t',
		'+++ b/sp ace.txt\t',
		'@@ -1 +1 @@',
		'-1',
		'+2',
		'diff --git a/b b/b',
		'old mode 100644',
		'new mode 100755',
		'',
	].join('\n');
	const fs = parse(raw);
	const p = fs.map((f) => f.path);
	ok('parse: count', fs.length === 7, p);
	ok('parse: b/a/x.txt kept', p[0] === 'b/a/x.txt', p[0]);
	ok('parse: added a/y.txt kept', p[1] === 'a/y.txt' && status(fs[1]!) === 'A', p[1]);
	ok('parse: deleted shows old path b/a/w.txt', p[2] === 'b/a/w.txt' && status(fs[2]!) === 'D', p[2]);
	ok(
		'parse: rename old/new keep own a/ b/',
		p[3] === 'b/new.txt' && fs[3]!.from === 'a/old.txt' && fs[3]!.note === 'Renamed, no content changes',
		fs[3],
	);
	ok('parse: rename status R', status(fs[3]!) === 'R');
	ok('parse: quoted path decoded', p[4] === 'q "x"\té.txt' && fs[4]!.from === undefined, p[4]);
	ok('parse: space path, trailing tab dropped', p[5] === 'sp ace.txt', p[5]);
	ok('parse: mode-only file named b', p[6] === 'b' && fs[6]!.note === 'No textual changes', fs[6]);
	ok('parse: hunk lines kept', fs[0]!.hunks[0]!.lines.map((l) => l.type + l.text).join() === 'delold,addnew');
}

// ---- parse: binary detection per entry (a text deletion before the binary deletion must not matter)
{
	const raw = [
		'diff --git a/gone.txt b/gone.txt',
		'deleted file mode 100644',
		'--- a/gone.txt',
		'+++ /dev/null',
		'@@ -1 +0,0 @@',
		'-Binary files x and y differ',
		'diff --git a/bdel.bin b/bdel.bin',
		'deleted file mode 100644',
		'index 1..0',
		'Binary files a/bdel.bin and /dev/null differ',
		'diff --git a/badd.bin b/badd.bin',
		'new file mode 100644',
		'index 0..1',
		'Binary files /dev/null and b/badd.bin differ',
		'diff --git a/bmod.bin b/bmod.bin',
		'index 1..2 100644',
		'GIT binary patch',
		'literal 3',
		'KcmZ?wW&i*H0RRI4',
		'',
		'diff --git a/empty.txt b/empty.txt',
		'new file mode 100644',
		'index 0..e69de29',
		'',
	].join('\n');
	const fs = parse(raw);
	const by = (n: string) => fs.find((f) => f.path === n) as DiffFile;
	ok('binary: text file with Binary-looking content not binary', !by('gone.txt').binary && !by('gone.txt').note);
	ok('binary: deleted tracked binary', by('bdel.bin').binary && by('bdel.bin').note === 'Binary file');
	ok('binary: added tracked binary', by('badd.bin').binary && by('badd.bin').note === 'Binary file');
	ok('binary: GIT binary patch', by('bmod.bin').binary && by('bmod.bin').note === 'Binary file');
	ok(
		'binary: status M',
		['bdel.bin', 'badd.bin', 'bmod.bin'].every((n) => status(by(n)) === 'M'),
	);
	ok(
		'binary: +0 -0',
		['bdel.bin', 'badd.bin'].every((n) => by(n).adds === 0 && by(n).dels === 0),
	);
	ok('empty new file: No textual changes', !by('empty.txt').binary && by('empty.txt').note === 'No textual changes');
}

// ---- real git: prefix dirs, deleted / added binaries, untracked binary
const d = mkdtempSync(join(tmpdir(), 'xplain-parse-'));
const g = (...a: string[]) =>
	execFileSync('git', ['-c', 'user.email=a@b.c', '-c', 'user.name=x', ...a], {cwd: d, stdio: 'pipe', timeout: 20000});
try {
	g('init', '-q');
	mkdirSync(join(d, 'b/a'), {recursive: true});
	mkdirSync(join(d, 'a'), {recursive: true});
	writeFileSync(join(d, 'b/a/x.txt'), 'x1\n');
	writeFileSync(join(d, 'b/a/w.txt'), 'w1\n');
	writeFileSync(join(d, 'gone.txt'), 'g\n');
	writeFileSync(join(d, 'bdel.bin'), 'a\0b');
	g('add', '-A');
	g('commit', '-qm', 'base');
	writeFileSync(join(d, 'b/a/x.txt'), 'x2\n');
	unlinkSync(join(d, 'b/a/w.txt'));
	unlinkSync(join(d, 'gone.txt'));
	unlinkSync(join(d, 'bdel.bin'));
	writeFileSync(join(d, 'a/y.txt'), 'y\n');
	writeFileSync(join(d, 'badd.bin'), 'x\0y');
	g('add', 'a/y.txt', 'badd.bin');
	writeFileSync(join(d, 'ubin.bin'), 'u\0v');
	const fs = await loadDiff('all', [], d);
	const by = (n: string) => fs.find((f) => f.path === n);
	ok(
		'git: paths as git prints them',
		['b/a/x.txt', 'b/a/w.txt', 'a/y.txt'].every((n) => by(n)),
		fs.map((f) => f.path),
	);
	ok('git: deleted b/a/w.txt is D', by('b/a/w.txt') !== undefined && status(by('b/a/w.txt')!) === 'D');
	ok('git: deleted tracked binary note', by('bdel.bin')?.note === 'Binary file');
	ok('git: added tracked binary note', by('badd.bin')?.note === 'Binary file');
	ok('git: untracked binary note', by('ubin.bin')?.note === 'Binary file');
	ok('git: deleted text not binary', by('gone.txt')?.binary === false);
} finally {
	rmSync(d, {recursive: true, force: true});
}

// ---- F-MODE-04: outside a repo the error is always git diff's own stderr (never ls-files'), every time
{
	const nd = mkdtempSync(join(tmpdir(), 'xplain-nogit-'));
	try {
		const env = {...process.env, GIT_CEILING_DIRECTORIES: tmpdir()};
		const want = (() => {
			try {
				execFileSync('git', ['diff', '--no-color', '--no-ext-diff', '-U1000000', 'HEAD'], {
					cwd: nd,
					stdio: 'pipe',
					env,
				});
				return '';
			} catch (e) {
				return String((e as {stderr?: Buffer}).stderr ?? '');
			}
		})();
		const prev = process.env.GIT_CEILING_DIRECTORIES;
		process.env.GIT_CEILING_DIRECTORIES = tmpdir();
		const msgs = await Promise.all(
			Array.from({length: 12}, () =>
				loadDiff('all', [], nd).then(
					() => 'ok',
					(e: Error) => e.message,
				),
			),
		);
		if (prev === undefined) delete process.env.GIT_CEILING_DIRECTORIES;
		else process.env.GIT_CEILING_DIRECTORIES = prev;
		ok('no repo: git diff usage text', want.includes('usage: git diff --no-index'), want.slice(0, 80));
		ok(
			'no repo: always git diff stderr',
			msgs.every((m) => m === want),
			[...new Set(msgs)].map((m) => m.slice(0, 60)),
		);
	} finally {
		rmSync(nd, {recursive: true, force: true});
	}
}
process.exit(fail ? 1 : 0);
