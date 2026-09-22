import {execFileSync} from 'node:child_process';
import {mkdtempSync, writeFileSync, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {loadDiff} from '../src/diff/load.js';

let fail = 0;
const ok = (n: string, c: boolean) => {
	console.log(c ? 'PASS' : 'FAIL', n);
	if (!c) fail++;
};
const d = mkdtempSync(join(tmpdir(), 'xplain-untracked-'));
const g = (...a: string[]) =>
	execFileSync('git', ['-c', 'user.email=a@b.c', '-c', 'user.name=x', ...a], {cwd: d, stdio: 'pipe', timeout: 20000});
try {
	g('init', '-q');
	writeFileSync(join(d, 'a.txt'), 'one\n');
	writeFileSync(join(d, '.gitignore'), 'ign.txt\n');
	g('add', '-A');
	g('commit', '-qm', 'init');
	writeFileSync(join(d, 'new.txt'), 'x\ny\n');
	writeFileSync(join(d, 'ign.txt'), 'nope\n');
	writeFileSync(join(d, 'big.txt'), 'z'.repeat(2 * 1024 * 1024));
	const all = await loadDiff('all', [], d);
	const n = all.find((f) => f.path === 'new.txt');
	ok('untracked in all', !!n && n.adds === 2 && n.dels === 0);
	ok('ignored skipped', !all.some((f) => f.path === 'ign.txt'));
	ok('huge skipped', !all.some((f) => f.path === 'big.txt'));
	ok('not in unstaged', !(await loadDiff('unstaged', [], d)).some((f) => f.path === 'new.txt'));
	ok('not in staged', !(await loadDiff('staged', [], d)).some((f) => f.path === 'new.txt'));
	ok('index untouched', g('status', '--porcelain').toString().includes('?? new.txt'));
	writeFileSync(join(d, 'later.txt'), 'q\n');
	ok(
		'reload picks new',
		(await loadDiff('all', [], d)).some((f) => f.path === 'later.txt'),
	);
} finally {
	rmSync(d, {recursive: true, force: true});
}
process.exit(fail ? 1 : 0);
