import {render} from 'ink-testing-library';
import App from '../src/app.js';
import {join} from 'node:path';
import {existsSync, readFileSync} from 'node:fs';
import {cwd, ok, keyPress, tmpDir, finish, until, booted} from './keysHelpers.js';
const {stdin, lastFrame} = render(<App args={[]} cwd={cwd} />);
await booted({lastFrame});
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
ok('help opens', f().includes('Help \u00b7 Diff view') && f().includes('[2/'));
const hl = f().split('\n');
ok('help fits terminal', hl.length <= 24);
ok('help has no cursor mode toggle', !f().includes('cursor mode') && !/\bi\s+(enter|exit)/.test(f()));
ok('footer: hjkl move  enter ask  J/K comments  ? help', f().includes('hjkl move  enter ask  J/K comments  ? help'));
ok(
	'help core keys, no motions',
	f().includes(']/[') &&
		f().includes('export comments') &&
		!f().includes('Move') &&
		!f().includes('w/b/e') &&
		!/g\/G\s+first/.test(f()),
);
ok(
	'help lists former diff view keys',
	['/ n/N', 'tab/S-tab', 'f/F', 's/c/m', 't/r', 'M/C', ')/(', 'v/V'].every((k) => f().includes(k)) &&
		/q\s+quit/.test(f()),
);
{
	const hi = hl.findIndex((l) => l.includes('Help · Diff view'));
	const top = hl[hi - 1] ?? '';
	const bw = top.lastIndexOf('╮') - top.indexOf('╭') + 1;
	ok(
		'help single column, fits content at 100 cols',
		hl.some((l) => /│ Find\s+│/.test(l)) && hl.some((l) => /│ General\s+│/.test(l)) && top.includes('╭') && bw < 50,
	);
}
ok('help fits without overflow at 24 rows', !/\u2026 \d+ more/.test(f()));
ok('help bottom row has move keys hint', hl.some((l) => l.includes('? move keys')) && !f().includes('? close'));
await pr('?');
ok(
	'2nd ? shows motions, minus footer ones',
	f().includes('Help \u00b7 Diff view') &&
		f().includes('Move') &&
		!/h\/j\/k\/l\s+char/.test(f()) &&
		/d\/u\s+half page down/.test(f()) &&
		/g\/G\s+first/.test(f()) &&
		f().includes('? close') &&
		!f().includes('? move keys'),
);
await pr('?');
ok('3rd ? closes help', !f().includes('Help \u00b7'));
await pr('?');
const pos = () => /\[cursor ([^\]]+)\]/.exec(f())?.[1];
await pr('g');
const p0 = pos();
await pr('j');
ok('j moves cursor while help open', !!p0 && !!pos() && pos() !== p0 && f().includes('Help \u00b7 Diff view'));
const p1 = pos();
await pr('\x1b');
ok('esc does not close help, no-op', f().includes('Help \u00b7 Diff view') && pos() === p1);
await pr('i');
ok('i is a no-op', f().includes('Help \u00b7 Diff view') && pos() === p1);
ok(
	'cursor core: full v/V, no w/b/e',
	/v\/V\s+select chars\/lines/.test(f()) && !f().includes('w/b/e') && f().includes('? move keys'),
);
await pr('?');
ok('cursor 2nd ?: w/b/e', f().includes('Help \u00b7 Diff view') && f().includes('w/b/e') && f().includes('? close'));
await pr('?');
await pr('?');
await pr('\x1b');
ok('esc keeps cursor, help stays', f().includes('Help \u00b7 Diff view') && pos() === p1);
await pr('f');
ok('f opens picker while help open', f().includes('Files (') && f().includes('Help \u00b7 File picker'));
await pr('\x1b');
await pr('M');
ok('help label in MCP modal', f().includes('Help \u00b7 MCP'));
await pr('?');
ok('? shows motions in MCP modal', f().includes('Help \u00b7 MCP') && f().includes('Move') && f().includes('? close'));
await pr('?');
ok('? closes help in MCP modal', !f().includes('Help \u00b7') && f().includes('MCP'));
await pr('?');
ok('? reopens help in MCP modal', f().includes('Help \u00b7 MCP'));
await pr('\x1b');
await pr('C');
ok('help label in config modal', f().includes('Help \u00b7 Config') && f().includes('Config'));
await pr('?');
await pr('?');
ok('? closes help in config modal', !f().includes('Help \u00b7') && f().includes('Config'));
await pr('?');
await pr('\x1b');
await pr('/');
ok('help label in find', f().includes('Help \u00b7 Find in file'));
ok('help no close hint in find', !f().includes('? close'));
await pr('?');
ok('? typed in / input, help stays', f().includes('/?\u2588') && f().includes('Help \u00b7 Find in file'));
await pr('\x1b');
ok('esc cancels find, help stays', !f().includes('/?\u2588') && f().includes('Help \u00b7 Diff view'));
await pr('?');
await pr('?');
ok('? closes help', !f().includes('Help \u00b7'));
await pr('q');
await pr('?');
ok(
	'dialog help: no motions, close hint',
	f().includes('Help \u00b7 Confirm') && f().includes('? close') && !f().includes('? move keys'),
);
await pr('?');
ok('dialog 2nd ? closes help', !f().includes('Help \u00b7') && f().includes('Quit xplain?'));
await pr('n');
ok('n closes quit dialog', !f().includes('Quit xplain?'));
console.log('unified header', f().includes('[unified]'));
await pr('s');
ok('s -> split', f().includes('[split]') && f().includes('│'));
const splitFrame = f();
await pr('?');
const helpFrame = f();
await pr('?');
await pr('?');
await pr('s');
ok('s -> unified', f().includes('[unified]') && !f().includes('│'));
{
	const r = render(<App args={[]} cwd={cwd} />);
	Object.defineProperty(r.stdout, 'rows', {value: 60});
	await booted(r);
	const g = () => r.lastFrame() ?? '';
	await keyPress(r)('?');
	ok(
		'tall help: no overflow, credit',
		g().includes('Help · Diff view') && !/… \d+ more/.test(g()) && g().includes('Made by Wouter de Wild - 2026'),
	);
	ok('tall help: lists global keys', g().includes('M/C') && g().includes('t/r'));
	ok('tall help: shows full v/V entry', /v\/V\s+select chars\/lines/.test(g()));
	ok('tall help: footer lacks v select', g().includes('hjkl move  enter ask') && !g().includes('v select'));
	ok(
		'tall help: )/( in help, J/K only in footer',
		/\)\/\(\s+numbered, any file/.test(g()) && !/J\/K\s+next\/prev in file/.test(g()) && g().includes('J/K comments'),
	);
	r.unmount();
}
const nar = render(<App args={[]} cwd={cwd} split />);
Object.defineProperty(nar.stdout, 'columns', {value: 80});
await booted(nar);
nar.stdin.write('s');
await keyPress(nar)('s');
ok('narrow fallback', (nar.lastFrame() ?? '').includes('too narrow for split'));
{
	const r = render(<App args={[]} cwd={cwd} />);
	await booted(r);
	await keyPress(r)('\t');
	const g = () => r.lastFrame() ?? '';
	ok('full: big.ts', g().includes('big.ts') && g().includes('[full]'));
	const first = () => Number(/\((\d+)-/.exec(g())?.[1]);
	ok('initial scroll on first change', g().includes('v30 = 2') && first() > 1 && g().includes('v27 = 1'));
	await keyPress(r)('g');
	ok('g top, far line shown', g().includes('v1 = 1') && !g().includes('v30 = 2'));
	const total = Number(/\/(\d+)\)/.exec(g())?.[1]);
	await keyPress(r)(']');
	ok('] next change', g().includes('v30 = 2') && g().includes('[cursor L30:C1]'));
	await keyPress(r)('[');
	ok('[ no earlier change, stays', g().includes('[cursor L30:C1]') && g().includes('v30 = 2'));
	await keyPress(r)('c');
	await until(r, (f) => f.includes('[changes]') && Number(/\/(\d+)\)/.exec(f)?.[1]) < total); // rows load async
	const t2 = Number(/\/(\d+)\)/.exec(g())?.[1]);
	ok(
		'c -> changes-only, fewer rows',
		g().includes('[changes]') && g().includes('big.ts') && t2 < total && !g().includes('v1 = 1'),
	);
	await keyPress(r)('c');
	await until(r, (f) => f.includes('[full]') && Number(/\/(\d+)\)/.exec(f)?.[1]) === total);
	ok('c -> full again, same file', g().includes('[full]') && g().includes('big.ts') && g().includes('v27 = 1'));
	console.log(g());
}
{
	const d = render(<App args={[]} cwd={cwd} />);
	await booted(d);
	ok('theme default solarized', (d.lastFrame() ?? '').includes('[solarized]'));
	const r = render(<App args={[]} cwd={cwd} theme="vibrant" />);
	await booted(r);
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
	ok('t cycles theme while help open', g().includes('Help · Diff view') && g().includes('[dull]'));
	await keyPress(r)('?');
}
{
	const dir = tmpDir('keys-cfg');
	const cp = join(dir, `c${Date.now()}.json`);
	const r = render(<App args={[]} cwd={cwd} theme="vibrant" configPath={cp} />);
	await booted(r);
	await keyPress(r)('t');
	ok('t does not write config', !existsSync(cp));
}
{
	const dir = tmpDir('keys-cfg');
	const cp = join(dir, `cm${Date.now()}.json`);
	const r = render(<App args={[]} cwd={cwd} theme="vibrant" configPath={cp} />);
	await booted(r);
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
	await booted(b);
	await keyPress(b)('F');
	ok('F opens search', bf().includes('Search ('));
	b.stdin.write('j');
	await keyPress(b)('q');
	ok('j/q are text', bf().includes('> jq') && bf().includes('Search ('));
	b.stdin.write('\x7f');
	b.stdin.write('\x7f');
	await keyPress(b)('big');
	await until(b, 'Search (1/'); // file list loads async
	ok('backspace + fuzzy', bf().includes('> big') && bf().includes('Search (1/1)'));
	await keyPress(b)('\x1b');
	ok('esc closes search', !bf().includes('Search (') && bf().includes('[1/'));
	await keyPress(b)('F');
	await until(b, 'x.bin'); // reopening reloads the list
	ok('empty query lists all', bf().includes('README.md') && bf().includes('x.bin'));
	await keyPress(b)('\x0e');
	ok('ctrl-n moves', bf().includes('Search (2/'));
	await keyPress(b)('\x10');
	ok('ctrl-p moves', bf().includes('Search (1/'));
	await keyPress(b)('a.ts');
	await keyPress(b)('\r');
	await until(b, '[browse]');
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
	await booted(n);
	ok('no changes shown', nf().includes('No changes'));
	await keyPress(n)('F');
	ok('no-changes F opens search', nf().includes('Search ('));
	await keyPress(n)('hello');
	await until(n, 'Search (1/');
	await keyPress(n)('\r');
	await until(n, '[browse]');
	ok('no-changes browse shows content', nf().includes('[browse]') && nf().includes('hello world'));
	await keyPress(n)('\x1b');
	await keyPress(n)('?');
	await keyPress(n)('?');
	await keyPress(n)('?');
	await keyPress(n)('F');
	await keyPress(n)('bin');
	await until(n, 'Search (1/');
	await keyPress(n)('\r');
	await until(n, 'binary file, not shown');
	ok('binary file not shown', nf().includes('binary file, not shown'));
	n.unmount();
}

console.log(splitFrame);
console.log(helpFrame);
finish();
