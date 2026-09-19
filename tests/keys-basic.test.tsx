import {render} from 'ink-testing-library';
import App from '../src/app.js';
import {join} from 'node:path';
import {existsSync, readFileSync} from 'node:fs';
import {KEYS} from '../src/keys.js';
import {cwd, tick, ok, keyPress, tmpDir, finish} from './keysHelpers.js';
const {stdin, lastFrame} = render(<App args={[]} cwd={cwd} />);
await tick();
const pr = keyPress({stdin, lastFrame});
const f = () => lastFrame() ?? '';
ok('loaded', f().includes('[1/'));
await pr('d');
await pr('\x04');
await pr('f');
ok('modal open', f().includes('Files ('));
await pr('j');
ok('sel moved', f().includes('Files (2/'));
await pr('d');
await pr('\t');
ok('main keys ignored in modal', f().includes('Files ('));
await pr('q');
ok('q closes modal', f().includes('[1/'));
await pr('f');
await pr('j');
await pr('\r');
ok('enter jumps', f().includes('[2/'));
await pr('f');
await pr('\x1b');
ok('esc closes', f().includes('[2/') && !f().includes('Files ('));
await pr('?');
ok('help opens', f().includes('Help') && f().includes('d/u'));
ok(
	'help lists every documented key',
	KEYS.every((k) => f().includes(k.k)),
);
const hl = f().split('\n');
ok(
	'help lists a ask / follow up, scroll',
	f().includes('a ask / follow up') && f().includes('scroll the comment thread') && hl.length <= 24,
);
ok(
	'help bottom row has close hint and credit',
	hl.some((l) => l.includes('?/esc/q close') && l.includes('Made by Wouter de Wild - 2026')),
);
await pr('f');
ok('main keys ignored in help', !f().includes('Files ('));
await pr('q');
ok('q closes help, app alive', !f().includes('Help') && f().includes('[2/'));
await pr('?');
await pr('?');
ok('? closes help', !f().includes('Help'));
await pr('?');
await pr('\x1b');
ok('esc closes help', !f().includes('Help'));
await pr('?');
await pr('?');
console.log('unified header', f().includes('[unified]'));
await pr('s');
ok('s -> split', f().includes('[split]') && f().includes('│'));
const splitFrame = f();
await pr('?');
const helpFrame = f();
await pr('?');
await pr('s');
ok('s -> unified', f().includes('[unified]') && !f().includes('│'));
const nar = render(<App args={[]} cwd={cwd} split />);
Object.defineProperty(nar.stdout, 'columns', {value: 80});
await tick();
nar.stdin.write('s');
await keyPress(nar)('s');
ok('narrow fallback', (nar.lastFrame() ?? '').includes('too narrow for split'));
{
	const r = render(<App args={[]} cwd={cwd} />);
	await tick();
	await keyPress(r)('\t');
	const g = () => r.lastFrame() ?? '';
	ok('full: big.ts', g().includes('big.ts') && g().includes('[full]'));
	const first = () => Number(/\((\d+)-/.exec(g())?.[1]);
	ok('initial scroll on first change', g().includes('v30 = 2') && first() > 1 && g().includes('v27 = 1'));
	await keyPress(r)('g');
	ok('g top, far line shown', g().includes('v1 = 1') && !g().includes('v30 = 2'));
	const total = Number(/\/(\d+)\)/.exec(g())?.[1]);
	await keyPress(r)(']');
	ok('] next change', g().includes('v30 = 2'));
	await keyPress(r)('[');
	ok('[ no earlier change, stays', first() === 28 && g().includes('v30 = 2'));
	await keyPress(r)('c');
	const t2 = Number(/\/(\d+)\)/.exec(g())?.[1]);
	ok(
		'c -> changes-only, fewer rows',
		g().includes('[changes]') && g().includes('big.ts') && t2 < total && !g().includes('v1 = 1'),
	);
	await keyPress(r)('c');
	ok('c -> full again, same file', g().includes('[full]') && g().includes('big.ts') && g().includes('v27 = 1'));
	console.log(g());
}
{
	const d = render(<App args={[]} cwd={cwd} />);
	await tick();
	ok('theme default solarized', (d.lastFrame() ?? '').includes('[solarized]'));
	const r = render(<App args={[]} cwd={cwd} theme="vibrant" />);
	await tick();
	const g = () => r.lastFrame() ?? '';
	ok('theme prop vibrant', g().includes('[vibrant]'));
	for (const n of ['dull', 'contrast', 'colorblind', 'light', 'solarized', 'vibrant']) {
		await keyPress(r)('t');
		ok('t -> ' + n, g().includes(`[${n}]`));
	}
	await keyPress(r)('f');
	await keyPress(r)('t');
	ok('t ignored in file modal', g().includes('Files (') && g().includes('[vibrant]'));
	await keyPress(r)('q');
	await keyPress(r)('?');
	await keyPress(r)('t');
	await keyPress(r)('q');
	ok('t ignored in help', g().includes('[vibrant]'));
}
{
	const dir = tmpDir('keys-cfg');
	const cp = join(dir, `c${Date.now()}.json`);
	const r = render(<App args={[]} cwd={cwd} theme="vibrant" configPath={cp} />);
	await tick();
	await keyPress(r)('t');
	ok('t does not write config', !existsSync(cp));
}
{
	const dir = tmpDir('keys-cfg');
	const cp = join(dir, `cm${Date.now()}.json`);
	const r = render(<App args={[]} cwd={cwd} theme="vibrant" configPath={cp} />);
	await tick();
	const g = () => r.lastFrame() ?? '';
	const cfg = () => JSON.parse(readFileSync(cp, 'utf8'));
	await keyPress(r)('C');
	ok(
		'C opens config, lists 4',
		g().includes('Config') && /theme/.test(g()) && /mode/.test(g()) && /split/.test(g()) && /view\s+\[full\]/.test(g()),
	);
	await keyPress(r)('\t');
	await keyPress(r)('s');
	ok('other keys ignored in config', g().includes('Config') && g().includes('[unified]'));
	await keyPress(r)('l');
	ok('l previews theme, not saved', g().includes('Config') && g().includes('[dull]') && !existsSync(cp));
	await keyPress(r)('\x1b');
	ok('esc reverts theme preview', !g().includes('Config') && g().includes('[vibrant]') && !existsSync(cp));
	await keyPress(r)('C');
	await keyPress(r)('l');
	await keyPress(r)('\r');
	ok('enter selects + saves theme', g().includes('[dull]') && cfg().theme === 'dull');
	await keyPress(r)('j');
	await keyPress(r)('l');
	ok('l on mode only moves cursor', !cfg().view?.mode && !g().includes('[staged]'));
	await keyPress(r)('\r');
	ok('enter selects mode', g().includes('[staged]') && cfg().view.mode === 'staged');
	await keyPress(r)('h');
	await keyPress(r)('\r');
	ok('h + enter mode back', g().includes('[all]') && cfg().view.mode === 'all');
	await keyPress(r)('j');
	await keyPress(r)('l');
	await keyPress(r)('\r');
	ok('split on', g().includes('[split]') && cfg().view.split === true);
	await keyPress(r)('j');
	await keyPress(r)('\r');
	ok('enter on current does not cycle', g().includes('[full]') && cfg().view?.full !== false);
	await keyPress(r)('l');
	await keyPress(r)('\r');
	ok('full off', g().includes('[changes]') && cfg().view.full === false && cfg().theme === 'dull');
	await keyPress(r)('\x1b');
	await keyPress(r)('c');
	ok('c still toggles', g().includes('[full]'));
	await keyPress(r)('C');
	await keyPress(r)('C');
	ok('C closes config', !g().includes('Config'));
	await keyPress(r)('?');
}
{
	const b = render(<App args={[]} cwd={cwd} />);
	const bf = () => b.lastFrame() ?? '';
	await tick();
	await keyPress(b)('F');
	ok('F opens search', bf().includes('Search ('));
	b.stdin.write('j');
	await keyPress(b)('q');
	ok('j/q are text', bf().includes('> jq') && bf().includes('Search ('));
	b.stdin.write('\x7f');
	b.stdin.write('\x7f');
	await keyPress(b)('big');
	ok('backspace + fuzzy', bf().includes('> big') && bf().includes('Search (1/1)'));
	await keyPress(b)('\x1b');
	ok('esc closes search', !bf().includes('Search (') && bf().includes('[1/'));
	await keyPress(b)('F');
	ok('empty query lists all', bf().includes('README.md') && bf().includes('x.bin'));
	await keyPress(b)('\x0e');
	ok('ctrl-n moves', bf().includes('Search (2/'));
	await keyPress(b)('\x10');
	ok('ctrl-p moves', bf().includes('Search (1/'));
	await keyPress(b)('a.ts');
	await keyPress(b)('\r');
	await tick();
	ok('enter -> browse', bf().includes('[browse]') && bf().includes('src/a.ts') && !bf().includes('Search ('));
	ok('browse no diff header', !bf().includes('[all]') && !bf().includes('[1/'));
	ok('browse one gutter', !/^\s*1\s+1\s/m.test(bf()) && /^\s*1\s/m.test(bf()));
	await keyPress(b)('\t');
	ok('n ignored in browse', bf().includes('src/a.ts'));
	b.stdin.write('j');
	b.stdin.write('G');
	await keyPress(b)('g');
	await keyPress(b)('t');
	await keyPress(b)('F');
	ok('F in browse opens search', bf().includes('Search ('));
	await keyPress(b)('\x1b');
	await keyPress(b)('\x1b');
	ok('esc leaves browse', bf().includes('[1/') && !bf().includes('[browse]'));
	await keyPress(b)('?');
	b.unmount();
}
{
	const {execSync} = await import('node:child_process');
	const {writeFileSync} = await import('node:fs');
	const dir = tmpDir('nochg-');
	const sh = (c: string) => execSync(c, {cwd: dir, stdio: 'ignore'});
	sh('git init -q');
	writeFileSync(join(dir, 'hello.txt'), 'hello world\n');
	writeFileSync(join(dir, 'bin.dat'), Buffer.from([65, 0, 66]));
	sh('git add -A');
	sh('git -c user.email=a@b -c user.name=n commit -qm init');
	const n = render(<App args={[]} cwd={dir} />);
	const nf = () => n.lastFrame() ?? '';
	await new Promise((r) => setTimeout(r, 400));
	ok('no changes shown', nf().includes('No changes'));
	await keyPress(n)('F');
	ok('no-changes F opens search', nf().includes('Search ('));
	await keyPress(n)('hello');
	await keyPress(n)('\r');
	ok('no-changes browse shows content', nf().includes('[browse]') && nf().includes('hello world'));
	await keyPress(n)('\x1b');
	await keyPress(n)('?');
	await keyPress(n)('?');
	await keyPress(n)('F');
	await keyPress(n)('bin');
	await keyPress(n)('\r');
	ok('binary file not shown', nf().includes('binary file, not shown'));
	n.unmount();
}

console.log(splitFrame);
console.log(helpFrame);
finish();
