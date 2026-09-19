import {render} from 'ink-testing-library';
import App from '../src/app.js';
import {mkdirSync, existsSync, readFileSync, rmSync} from 'node:fs';
const cleanup: string[] = [];
import {join} from 'node:path';
import type {ThemeName} from '../src/theme.js';
const cwd = process.argv[2];
const tick = () => new Promise((r) => setTimeout(r, 80));
const {stdin, lastFrame} = render(<App args={[]} cwd={cwd} />);
let fail = 0;
const ok = (n: string, c: boolean) => {
	console.log(c ? 'PASS' : 'FAIL', n);
	if (!c) fail++;
};
await tick();
const f = () => lastFrame() ?? '';
ok('loaded', f().includes('[1/'));
stdin.write('d');
await tick();
ok('d half page ok', f().includes('[1/'));
stdin.write('\x04');
await tick();
ok('ctrl-d ignored, no quit', f().includes('[1/'));
stdin.write('f');
await tick();
ok('modal open', f().includes('Files ('));
stdin.write('j');
await tick();
ok('sel moved', f().includes('Files (2/'));
stdin.write('d');
await tick();
ok('d in modal', /Files \(\d+\//.test(f()));
stdin.write('\t');
await tick();
ok('main keys ignored in modal', f().includes('Files ('));
stdin.write('q');
await tick();
ok('q closes modal', f().includes('[1/'));
stdin.write('f');
await tick();
stdin.write('j');
await tick();
stdin.write('\r');
await tick();
ok('enter jumps', f().includes('[2/'));
stdin.write('f');
await tick();
stdin.write('\x1b');
await tick();
ok('esc closes', f().includes('[2/') && !f().includes('Files ('));
stdin.write('?');
await tick();
ok('help opens', f().includes('Help') && f().includes('d/u'));
stdin.write('f');
await tick();
ok('main keys ignored in help', !f().includes('Files ('));
stdin.write('q');
await tick();
ok('q closes help, app alive', !f().includes('Help') && f().includes('[2/'));
stdin.write('?');
await tick();
stdin.write('?');
await tick();
ok('? closes help', !f().includes('Help'));
stdin.write('?');
await tick();
stdin.write('\x1b');
await tick();
ok('esc closes help', !f().includes('Help'));
stdin.write('A');
await tick();
ok('A opens agents modal', f().includes('Agents ('));
stdin.write('f');
await tick();
ok('main keys ignored in agents', !f().includes('Files ('));
stdin.write('j');
stdin.write('k');
await tick();
ok('j/k ok in agents', f().includes('Agents ('));
stdin.write('q');
await tick();
ok('q closes agents', !f().includes('Agents ('));
stdin.write('A');
await tick();
stdin.write('A');
await tick();
ok('A closes agents', !f().includes('Agents ('));
stdin.write('A');
await tick();
stdin.write('\x1b');
await tick();
ok('esc closes agents', !f().includes('Agents ('));
stdin.write('?');
await tick();
ok('help lists A', /\bA\s+running agents/.test(f()));
stdin.write('?');
await tick();
console.log('unified header', f().includes('[unified]'));
stdin.write('s');
await tick();
ok('s -> split', f().includes('[split]') && f().includes('│'));
const splitFrame = f();
stdin.write('?');
await tick();
const helpFrame = f();
stdin.write('?');
await tick();
stdin.write('s');
await tick();
ok('s -> unified', f().includes('[unified]') && !f().includes('│'));
const nar = render(<App args={[]} cwd={cwd} split />);
Object.defineProperty(nar.stdout, 'columns', {value: 80});
await tick();
nar.stdin.write('s');
nar.stdin.write('s');
await tick();
ok('narrow fallback', (nar.lastFrame() ?? '').includes('too narrow for split'));
{
	const r = render(<App args={[]} cwd={cwd} />);
	await tick();
	r.stdin.write('\t');
	await tick();
	const g = () => r.lastFrame() ?? '';
	ok('full: big.ts', g().includes('big.ts') && g().includes('[full]'));
	const first = () => Number(/\((\d+)-/.exec(g())?.[1]);
	ok('initial scroll on first change', g().includes('v30 = 2') && first() > 1 && g().includes('v27 = 1'));
	r.stdin.write('g');
	await tick();
	ok('g top, far line shown', g().includes('v1 = 1') && !g().includes('v30 = 2'));
	const total = Number(/\/(\d+)\)/.exec(g())?.[1]);
	r.stdin.write(']');
	await tick();
	ok('] next change', g().includes('v30 = 2'));
	r.stdin.write('[');
	await tick();
	ok('[ no earlier change, stays', first() === 28 && g().includes('v30 = 2'));
	r.stdin.write('c');
	await tick();
	const t2 = Number(/\/(\d+)\)/.exec(g())?.[1]);
	ok(
		'c -> changes-only, fewer rows',
		g().includes('[changes]') && g().includes('big.ts') && t2 < total && !g().includes('v1 = 1'),
	);
	r.stdin.write('c');
	await tick();
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
		r.stdin.write('t');
		await tick();
		ok('t -> ' + n, g().includes(`[${n}]`));
	}
	r.stdin.write('f');
	await tick();
	r.stdin.write('t');
	await tick();
	ok('t ignored in file modal', g().includes('Files (') && g().includes('[vibrant]'));
	r.stdin.write('q');
	await tick();
	r.stdin.write('?');
	await tick();
	ok('help lists t', /\bt\s+cycle theme/.test(g()));
	r.stdin.write('t');
	await tick();
	r.stdin.write('q');
	await tick();
	ok('t ignored in help', g().includes('[vibrant]'));
	const h0 = g();
	r.stdin.write('T');
	await tick();
	ok('T does nothing in main view', !g().includes('Themes') && g() === h0);
	const c = render(<App args={[]} cwd={cwd} theme="dull" />);
	await tick();
	ok('theme prop initial', (c.lastFrame() ?? '').includes('[dull]'));
	for (const n of ['light', 'colorblind', 'solarized']) {
		const x = render(<App args={[]} cwd={cwd} theme={n as ThemeName} />);
		await tick();
		ok('theme prop ' + n, (x.lastFrame() ?? '').includes(`[${n}]`));
	}
}
{
	const dir = join(process.env.CLAUDE_JOB_DIR ?? '/tmp', 'tmp', 'keys-cfg');
	mkdirSync(dir, {recursive: true});
	cleanup.push(dir);
	const cp = join(dir, `c${Date.now()}.json`);
	const r = render(<App args={[]} cwd={cwd} theme="vibrant" configPath={cp} />);
	await tick();
	r.stdin.write('t');
	await tick();
	ok('t does not write config', !existsSync(cp));
}
{
	const dir = join(process.env.CLAUDE_JOB_DIR ?? '/tmp', 'tmp', 'keys-cfg');
	mkdirSync(dir, {recursive: true});
	cleanup.push(dir);
	const cp = join(dir, `cm${Date.now()}.json`);
	const r = render(<App args={[]} cwd={cwd} theme="vibrant" configPath={cp} />);
	await tick();
	const g = () => r.lastFrame() ?? '';
	const cfg = () => JSON.parse(readFileSync(cp, 'utf8'));
	r.stdin.write('C');
	await tick();
	ok(
		'C opens config, lists 4',
		g().includes('Config') && /theme/.test(g()) && /mode/.test(g()) && /split/.test(g()) && /view\s+\[full\]/.test(g()),
	);
	r.stdin.write('\t');
	await tick();
	r.stdin.write('s');
	await tick();
	ok('other keys ignored in config', g().includes('Config') && g().includes('[unified]'));
	r.stdin.write('l');
	await tick();
	ok('l previews theme, not saved', g().includes('Config') && g().includes('[dull]') && !existsSync(cp));
	r.stdin.write('\x1b');
	await tick();
	ok('esc reverts theme preview', !g().includes('Config') && g().includes('[vibrant]') && !existsSync(cp));
	r.stdin.write('C');
	await tick();
	r.stdin.write('l');
	await tick();
	r.stdin.write('\r');
	await tick();
	ok('enter selects + saves theme', g().includes('[dull]') && cfg().theme === 'dull');
	r.stdin.write('j');
	await tick();
	r.stdin.write('l');
	await tick();
	ok('l on mode only moves cursor', !cfg().view?.mode && !g().includes('[staged]'));
	r.stdin.write('\r');
	await tick();
	ok('enter selects mode', g().includes('[staged]') && cfg().view.mode === 'staged');
	r.stdin.write('h');
	await tick();
	r.stdin.write('\r');
	await tick();
	ok('h + enter mode back', g().includes('[all]') && cfg().view.mode === 'all');
	r.stdin.write('j');
	await tick();
	r.stdin.write('l');
	await tick();
	r.stdin.write('\r');
	await tick();
	ok('split on', g().includes('[split]') && cfg().view.split === true);
	r.stdin.write('j');
	await tick();
	r.stdin.write('\r');
	await tick();
	ok('enter on current does not cycle', g().includes('[full]') && cfg().view?.full !== false);
	r.stdin.write('l');
	await tick();
	r.stdin.write('\r');
	await tick();
	ok('full off', g().includes('[changes]') && cfg().view.full === false && cfg().theme === 'dull');
	r.stdin.write('\x1b');
	await tick();
	ok('esc closes config', !g().includes('Config'));
	r.stdin.write('c');
	await tick();
	ok('c still toggles', g().includes('[full]'));
	r.stdin.write('C');
	await tick();
	r.stdin.write('C');
	await tick();
	ok('C closes config', !g().includes('Config'));
	r.stdin.write('?');
	await tick();
	ok('help lists C', /\bC\s+config/.test(g()));
}
{
	const b = render(<App args={[]} cwd={cwd} />);
	const bf = () => b.lastFrame() ?? '';
	await tick();
	b.stdin.write('F');
	await tick();
	ok('F opens search', bf().includes('Search ('));
	b.stdin.write('j');
	b.stdin.write('q');
	await tick();
	ok('j/q are text', bf().includes('> jq') && bf().includes('Search ('));
	b.stdin.write('\x7f');
	b.stdin.write('\x7f');
	b.stdin.write('big');
	await tick();
	ok('backspace + fuzzy', bf().includes('> big') && bf().includes('Search (1/1)'));
	b.stdin.write('\x1b');
	await tick();
	ok('esc closes search', !bf().includes('Search (') && bf().includes('[1/'));
	b.stdin.write('F');
	await tick();
	ok('empty query lists all', bf().includes('README.md') && bf().includes('x.bin'));
	b.stdin.write('\x0e');
	await tick();
	ok('ctrl-n moves', bf().includes('Search (2/'));
	b.stdin.write('\x10');
	await tick();
	ok('ctrl-p moves', bf().includes('Search (1/'));
	b.stdin.write('a.ts');
	await tick();
	b.stdin.write('\r');
	await tick();
	await tick();
	ok('enter -> browse', bf().includes('[browse]') && bf().includes('src/a.ts') && !bf().includes('Search ('));
	ok('browse no diff header', !bf().includes('[all]') && !bf().includes('[1/'));
	ok('browse one gutter', !/^\s*1\s+1\s/m.test(bf()) && /^\s*1\s/m.test(bf()));
	b.stdin.write('\t');
	await tick();
	ok('n ignored in browse', bf().includes('src/a.ts'));
	b.stdin.write('j');
	b.stdin.write('G');
	b.stdin.write('g');
	await tick();
	ok('scroll keys ok', bf().includes('[browse]'));
	b.stdin.write('t');
	await tick();
	ok('theme key in browse', bf().includes('[browse]'));
	b.stdin.write('F');
	await tick();
	ok('F in browse opens search', bf().includes('Search ('));
	b.stdin.write('\x1b');
	await tick();
	b.stdin.write('\x1b');
	await tick();
	ok('esc leaves browse', bf().includes('[1/') && !bf().includes('[browse]'));
	b.stdin.write('?');
	await tick();
	ok('help lists F', /\bF\s+search/.test(bf()));
	b.unmount();
}
{
	const {execSync} = await import('node:child_process');
	const {mkdtempSync, writeFileSync} = await import('node:fs');
	const dir = mkdtempSync(join(process.env.CLAUDE_JOB_DIR ?? '.', 'tmp', 'nochg-'));
	cleanup.push(dir);
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
	n.stdin.write('F');
	await tick();
	ok('no-changes F opens search', nf().includes('Search ('));
	n.stdin.write('hello');
	await tick();
	n.stdin.write('\r');
	await tick();
	ok('no-changes browse shows content', nf().includes('[browse]') && nf().includes('hello world'));
	n.stdin.write('\x1b');
	await tick();
	n.stdin.write('?');
	await tick();
	ok('no-changes help opens', nf().includes('Help'));
	n.stdin.write('?');
	await tick();
	n.stdin.write('A');
	await tick();
	ok('no-changes A opens agents', nf().includes('Agents ('));
	n.stdin.write('q');
	await tick();
	ok('no-changes q closes agents', !nf().includes('Agents ('));
	n.stdin.write('F');
	await tick();
	n.stdin.write('bin');
	await tick();
	n.stdin.write('\r');
	await tick();
	ok('binary file not shown', nf().includes('binary file, not shown'));
	n.unmount();
}
{
	type Cur = {index: number; row?: {kind: string; text?: string}} | undefined;
	const mk = (props: Record<string, unknown> = {}) => {
		let c: Cur;
		const r = render(<App args={[]} cwd={cwd} onCursor={(x) => (c = x as Cur)} {...props} />);
		const g = () => r.lastFrame() ?? '';
		const w = async (k: string) => {
			r.stdin.write(k);
			await tick();
		};
		return {r, g, w, cur: () => c, ln: () => Number(/\[cursor(?: old| new)? (?:L|r)(\d+):C\d+\]/.exec(g())?.[1])};
	};
	const first = (g: () => string) => Number(/\((\d+)-/.exec(g())?.[1]);
	{
		const {r, g, w, cur, ln} = mk();
		await tick();
		await w('\t');
		await w('j');
		ok('no cursor outside mode', !g().includes('[cursor') && cur() === undefined);
		await w('i');
		ok('i enters, indicator', /\[cursor (L|r)\d+:C\d+\]/.test(g()) && cur() !== undefined);
		await w('g');
		ok('cursor row exposed', cur()?.row !== undefined);
		await w('j');
		ok('j moves', cur()?.index === 1);
		await w('k');
		ok('k moves back', cur()?.index === 0);
		await w('k');
		ok('k clamps at 0', cur()?.index === 0);
		await w('5');
		await w('j');
		ok('5j count', cur()?.index === 5);
		await w('3');
		await w('x');
		await w('j');
		ok('count cleared by other key', cur()?.index === 6);
		await w('G');
		const lastIdx = cur()!.index;
		ok('G last', lastIdx > 6);
		await w('g');
		ok('g first', cur()?.index === 0);
		await w('4');
		await w('G');
		ok('4G goes row 4', cur()?.index === 3);
		await w('d');
		ok('d moves cursor half page', cur()!.index > 3);
		await w('u');
		ok('u moves cursor back', cur()?.index === 3);
		await w('\t');
		ok('n switches file, cursor reset, mode on', /\[3\//.test(g()) && cur() !== undefined && g().includes('[cursor'));
		await w('\x1b');
		ok('esc exits, still running', !g().includes('[cursor') && cur() === undefined && /\[3\//.test(g()));
		await w('j');
		await w('i');
		await w('i');
		ok('i again exits', !g().includes('[cursor'));
		await w('\x1b[Z');
		await w('i');
		await w('s');
		ok('split keeps cursor mode', g().includes('[split]') && g().includes('[cursor') && ln() >= 0);
		const s0 = cur()!.index;
		await w('j');
		await w('j');
		ok('split j moves', cur()?.index === s0 + 2 && cur()?.row?.kind !== undefined);
		await w('s');
		await w('?');
		ok('help lists i', /\bi\s+cursor mode/.test(g()) && g().includes('5j'));
		const top = g().split('\n')[0]!;
		ok('help does not cover header (24 rows)', top.includes('[cursor'));
		await w('?');
		await w('q');
		r.unmount();
	}
	{
		const {r, g, w, cur} = mk();
		await tick();
		await w('\t');
		ok('cursor test: big.ts full', g().includes('big.ts') && g().includes('[full]'));
		await w('i');
		const st = cur()!.index;
		ok('full: cursor at first change', st > 20 && g().includes('v30 = 2'));
		await w('g');
		ok('g top scrolls to it', cur()?.index === 0 && g().includes('v1 = 1') && first(g) === 1);
		await w(']');
		ok('] cursor next change', cur()?.index === st);
		await w('[');
		ok('[ no earlier change stays', cur()?.index === st);
		await w('G');
		ok('G scrolls to bottom', cur()!.index > st && !g().includes('v1 = 1'));
		await w('g');
		for (let k = 0; k < 30; k++) r.stdin.write('j');
		await tick();
		const fl = first(g);
		const to = Number(/-(\d+)\//.exec(g())?.[1]);
		ok('scroll follows cursor', cur()?.index === 30 && fl <= 31 && to >= 31 && fl > 1);
		await w('c');
		ok('c resets cursor, mode stays', g().includes('[changes]') && g().includes('[cursor'));
		await w('c');
		ok('c back full, cursor at first change', g().includes('[full]') && cur()?.index === st);
		await w('q');
		r.unmount();
	}
	{
		const {r, g, w} = mk();
		await tick();
		await w('\t');
		await w('g');
		const a = first(g);
		await w('j');
		ok('outside cursor mode j scrolls', first(g) === a + 1 && !g().includes('[cursor'));
		r.unmount();
	}
	{
		const {r, g, w, cur} = mk();
		await tick();
		await w('F');
		await w('a.ts');
		await w('\r');
		await tick();
		ok('cursor browse: in browse', g().includes('[browse]'));
		await w('i');
		await w('j');
		await w('j');
		ok('browse cursor moves, indicator L', cur()?.index === 2 && /\[cursor L3:C\d+\]/.test(g()));
		await w('\x1b');
		ok('esc exits cursor only, stays browse', g().includes('[browse]') && !g().includes('[cursor'));
		await w('\x1b');
		ok('esc leaves browse', !g().includes('[browse]') && g().includes('[1/'));
		await w('i');
		await w('t');
		await w('m');
		ok('t/m work in cursor mode', (g().includes('[cursor') && g().includes('[staged]')) || g().includes('No changes'));
		r.unmount();
	}
}
{
	// prompt window + quit confirm
	const dir = join(process.env.CLAUDE_JOB_DIR ?? '/tmp', 'tmp', 'keys-ask');
	mkdirSync(dir, {recursive: true});
	cleanup.push(dir);
	type Q = {file: string; index: number; line?: number; text: string; message: string};
	const mk = (props: Record<string, unknown> = {}) => {
		const qs: Q[] = [];
		const r = render(<App args={[]} cwd={cwd} onQuestion={(q) => qs.push(q as Q)} {...props} />);
		const g = () => r.lastFrame() ?? '';
		const w = async (k: string) => {
			r.stdin.write(k);
			await tick();
		};
		return {r, g, w, qs};
	};
	{
		const {r, g, w, qs} = mk();
		await tick();
		await w('\r');
		ok('enter outside cursor mode: no box', !g().includes('enter send'));
		await w('a');
		ok('a outside cursor mode: no box', !g().includes('enter send'));
		await w('i');
		await w('j');
		await w('\r');
		ok('enter opens box', g().includes('enter send  esc cancel') && g().includes('[cursor'));
		await w('j');
		await w('q');
		await w('i');
		ok('keys are text while open', g().includes('jqi') && g().includes('enter send') && !g().includes('Quit xplain'));
		await w('\x7f');
		ok('backspace deletes', g().includes('jq') && !g().includes('jqi'));
		await w('\x1b[D');
		await w('X');
		ok('left moves caret', g().includes('jXq'));
		await w('\x1b[C');
		await w('\x1b[C');
		await w('!');
		ok('right moves caret', g().includes('jXq!'));
		await w('\x1b');
		ok('esc cancels', !g().includes('enter send') && qs.length === 0 && g().includes('[cursor'));
		await w('a');
		ok('a reopens with empty text', g().includes('enter send') && !g().includes('jXq'));
		await w('\r');
		ok('empty submit stays open', g().includes('enter send') && qs.length === 0);
		await w('why?');
		await w('\r');
		ok('submit closes, note', !g().includes('enter send') && g().includes('question saved (1)'));
		ok(
			'payload',
			qs.length === 1 &&
				qs[0]!.message === 'why?' &&
				qs[0]!.index === 3 &&
				qs[0]!.line === 2 &&
				qs[0]!.text === 'hello world' &&
				typeof qs[0]!.file === 'string' &&
				qs[0]!.file.length > 0 &&
				typeof qs[0]!.text === 'string' &&
				typeof qs[0]!.line === 'number',
		);
		await w('a');
		await w('again');
		await w('\r');
		ok('second question numbered', g().includes('question saved (2)') && qs.length === 2);
		await w('G');
		await w('a');
		await w('bottom');
		ok(
			'box at last row keeps cursor visible',
			g().includes('enter send') && g().includes('bottom') && g().includes('[cursor'),
		);
		await w('\r');
		r.unmount();
	}
	{
		const {r, g, w, qs} = mk({split: true});
		await tick();
		await w('\t');
		await w('i');
		await w('j');
		await w('a');
		ok('split: box opens', g().includes('[split]') && g().includes('enter send'));
		await w('hi');
		await w('\r');
		ok('split: submit', qs.length === 1 && qs[0]!.message === 'hi' && typeof qs[0]!.text === 'string');
		r.unmount();
	}
	{
		const {r, g, w, qs} = mk();
		await tick();
		await w('F');
		await w('a.ts');
		await w('\r');
		await tick();
		await w('i');
		await w('j');
		await w('j');
		await w('\r');
		ok('browse: box opens', g().includes('[browse]') && g().includes('enter send'));
		await w('note');
		await w('\r');
		ok(
			'browse: payload',
			qs.length === 1 &&
				/a\.ts$/.test(qs[0]!.file) &&
				qs[0]!.index === 2 &&
				qs[0]!.line === 3 &&
				qs[0]!.message === 'note',
		);
		r.unmount();
	}
	{
		const {r, g, w} = mk();
		await tick();
		await w('?');
		ok('help lists ask', /Enter\/a\s+ask about line/.test(g()) && g().split('\n')[0]!.includes('[1/'));
		await w('?');
		await w('q');
		ok('q in main shows quit modal', g().includes('Quit xplain? (y/n)'));
		await w('j');
		await w('f');
		ok('other keys ignored in quit modal', g().includes('Quit xplain?') && !g().includes('Files ('));
		await w('n');
		ok('n cancels', !g().includes('Quit xplain?') && g().includes('[1/'));
		await w('q');
		await w('\x1b');
		ok('esc cancels', !g().includes('Quit xplain?'));
		await w('q');
		await w('q');
		ok('q cancels', !g().includes('Quit xplain?') && g().includes('[1/'));
		await w('q');
		await w('y');
		await w('\t');
		await w('\t');
		ok('y quits (input dead after)', !g().includes('[2/'));
		await w('q');
		r.unmount();
	}
	{
		const {r, g, w} = mk({confirmQuit: false});
		await tick();
		await w('q');
		ok('confirm off: no modal', !g().includes('Quit xplain?'));
		await w('\t');
		ok('confirm off: q quit, input dead', !g().includes('[2/'));
		r.unmount();
	}
	{
		const cp = join(dir, `c${Date.now()}.json`);
		const {r, g, w} = mk({configPath: cp});
		await tick();
		await w('C');
		ok('config lists confirm quit', /confirm quit\s+off\s+\[on\]/.test(g()) && g().includes('Config'));
		for (let k = 0; k < 4; k++) await w('j');
		await w('h');
		await w('\r');
		ok(
			'enter selects off, saved',
			/confirm quit\s+\[off\]/.test(g()) && JSON.parse(readFileSync(cp, 'utf8')).app.confirmQuit === false,
		);
		await w('q');
		await w('q');
		ok('setting applies live', !g().includes('Quit xplain?'));
		r.unmount();
	}
}
{
	// char cursor, visual selection, hscroll, ask preview
	const {execSync} = await import('node:child_process');
	const {mkdtempSync, writeFileSync} = await import('node:fs');
	const repo = (oldT: string, newT: string) => {
		const d = mkdtempSync(join(process.env.CLAUDE_JOB_DIR ?? '.', 'tmp', 'vis-'));
		cleanup.push(d);
		const sh = (c: string) => execSync(c, {cwd: d, stdio: 'ignore'});
		sh('git init -q');
		writeFileSync(join(d, 'w.ts'), oldT);
		sh('git add -A');
		sh('git -c user.email=a@b -c user.name=n commit -qm init');
		writeFileSync(join(d, 'w.ts'), newT);
		return d;
	};
	const long = Array.from({length: 40}, (_, i) => 'word' + i).join(' ') + ' END';
	// full view rows: 0 hunk, 1 del old, 2 foo.bar(baz, 12), 3 empty, 4 "\tindented word", 5 long, 6 last
	const fx = repo('old\n', ['foo.bar(baz, 12)', '', '\tindented word', long, 'last'].join('\n') + '\n');
	type Q = {file: string; index: number; line?: number; text: string; message: string} & Record<string, unknown>;
	const mk = (dir: string, props: Record<string, unknown> = {}, cols?: number) => {
		const qs: Q[] = [];
		const r = render(<App args={[]} cwd={dir} onQuestion={(q) => qs.push(q as Q)} {...props} />);
		if (cols) Object.defineProperty(r.stdout, 'columns', {value: cols});
		const g = () => r.lastFrame() ?? '';
		const w = async (k: string) => {
			r.stdin.write(k);
			await tick();
		};
		const ws = async (ks: string) => {
			for (const k of ks) await w(k);
		};
		const hd = () => /\[(?:cursor|visual)(?: old| new)? [Lr]\d+:C\d+\]/.exec(g())?.[0] ?? '';
		return {r, g, w, ws, hd, qs};
	};
	const qs_len = (t: string) => t.split('sent').length - 1;
	const boot = () => new Promise((r) => setTimeout(r, 350));
	{
		const {r, g, w, ws, hd} = mk(fx);
		await boot();
		await ws('ij');
		ok('char: start col 1', hd() === '[cursor L1:C1]');
		await w('h');
		ok('h clamps at 0', hd() === '[cursor L1:C1]');
		await w('l');
		ok('l moves', hd() === '[cursor L1:C2]');
		await ws('3l');
		ok('3l count', hd() === '[cursor L1:C5]');
		await w('h');
		ok('h moves', hd() === '[cursor L1:C4]');
		await w('0');
		ok('0 line start', hd() === '[cursor L1:C1]');
		await w('$');
		ok('$ last char', hd() === '[cursor L1:C16]');
		await w('l');
		ok('l clamps at end', hd() === '[cursor L1:C16]');
		await ws('0');
		await ws('10l');
		ok('10l: 0 extends count', hd() === '[cursor L1:C11]');
		await ws('0w');
		ok('w: foo -> .', hd() === '[cursor L1:C4]');
		await w('w');
		ok('w: . -> bar', hd() === '[cursor L1:C5]');
		await w('w');
		ok('w: bar -> (', hd() === '[cursor L1:C8]');
		await ws('ww');
		ok('w: baz -> ,', hd() === '[cursor L1:C12]');
		await w('w');
		ok('w skips blank to 12', hd() === '[cursor L1:C14]');
		await w('w');
		ok('w: 12 -> )', hd() === '[cursor L1:C16]');
		await w('w');
		ok('w crosses to empty line', hd() === '[cursor L2:C1]');
		await w('b');
		ok('b crosses back', hd() === '[cursor L1:C16]');
		await ws('0e');
		ok('e end of foo', hd() === '[cursor L1:C3]');
		await ws('2e');
		ok('2e', hd() === '[cursor L1:C7]');
		await ws('$b');
		ok('b from end -> 12', hd() === '[cursor L1:C14]');
		await ws('03l');
		await w('j');
		ok('j on empty line clamps', hd() === '[cursor L2:C1]');
		await w('j');
		ok('j keeps desired col (tab = 2 cells)', hd() === '[cursor L3:C4]');
		await w('k');
		await w('k');
		ok('k back keeps col', hd() === '[cursor L1:C4]');
		await ws('$j');
		await w('j');
		ok('$ sticks to end on j', hd() === '[cursor L3:C15]');
		await w('0');
		await w('^');
		ok('^ first non-blank (tab expanded)', hd() === '[cursor L3:C3]');
		await w('\x1b[C');
		ok('right arrow moves char in cursor mode', hd() === '[cursor L3:C4]' && g().includes('[1/1]'));
		await w('\x1b[D');
		ok('left arrow moves char', hd() === '[cursor L3:C3]');
		ok('help lists h/l w/b/e v', /h\/l 0\^\$/.test((await w('?'), g())) && /w\/b\/e/.test(g()) && /v\/V/.test(g()));
		ok('help keeps header (24 rows)', g().split('\n')[0]!.includes('[cursor'));
		await w('?');
		r.unmount();
	}
	{
		const {r, g, w, ws, hd, qs} = mk(fx);
		await boot();
		await ws('ij');
		await ws('llvll');
		ok('v: visual header', hd() === '[visual L1:C5]');
		await w('\r');
		ok(
			'ask shows range + text',
			g().includes('selection L1:C3-C5') && g().includes('> o.b') && g().includes('enter send'),
		);
		await w('\x1b');
		ok('cancel keeps selection', !g().includes('enter send') && hd() === '[visual L1:C5]');
		await w('a');
		await w('why');
		await w('\r');
		ok('submit clears selection', hd() === '[cursor L1:C5]' && !g().includes('enter send'));
		ok(
			'payload selection',
			qs.length === 1 &&
				qs[0]!.text === 'o.b' &&
				qs[0]!.startLine === 1 &&
				qs[0]!.endLine === 1 &&
				qs[0]!.startCol === 3 &&
				qs[0]!.endCol === 5 &&
				qs[0]!.line === 1 &&
				qs[0]!.message === 'why',
		);
		await ws('0llll');
		await w('v');
		await ws('jj');
		ok('multi-row visual', hd() === '[visual L3:C5]');
		await w('\r');
		ok(
			'ask multi-line preview',
			g().includes('selection L1:C5-L3:C5') && g().includes('> bar(baz, 12)') && g().includes('>   ind'),
		);
		await w('m');
		await w('\r');
		ok(
			'payload multi',
			qs[1]!.text === 'bar(baz, 12)\n\n  ind' && qs[1]!.startLine === 1 && qs[1]!.endLine === 3 && qs[1]!.endCol === 5,
		);
		await ws('kk');
		await w('a');
		await w('x');
		await w('\r');
		ok('no selection: old payload', qs[2]!.startLine === undefined && qs[2]!.text === 'foo.bar(baz, 12)');
		await ws('vl\x1b');
		ok('esc ends visual first', hd().startsWith('[cursor'));
		await w('\x1b');
		ok('second esc exits cursor', !g().includes('[cursor'));
		await ws('iv');
		await w('v');
		ok('v again ends', hd().startsWith('[cursor'));
		await ws('V');
		ok('V linewise', hd().startsWith('[visual'));
		await w('j');
		await w('a');
		ok('V ask preview lines', g().includes('selection L1-2') || g().includes('selection r'));
		await w('\x1b');
		await w('\x1b');
		await w('g');
		await w('V');
		await w('G');
		await w('\r');
		ok('preview cap +N more', g().includes('… +2 more') && g().includes('selection r1-5') && g().includes('[visual'));
		await w('\x1b');
		r.unmount();
	}
	{
		// reset on file switch / mode toggles, arrows outside cursor mode
		const {r, g, w, ws, hd} = mk(cwd);
		await boot();
		await w('\x1b[C');
		ok('right arrow outside cursor mode switches file', g().includes('[2/'));
		await w('\x1b[D');
		await ws('ilv');
		await w('l');
		ok('demo visual moving', hd().startsWith('[visual') && !g().includes('[2/'));
		await w('\t');
		ok('file switch clears selection, col 0', /\[cursor [Lr]\d+:C1\]/.test(hd()));
		await ws('lllv');
		await w('c');
		ok('c clears selection, col 0', /\[cursor [Lr]\d+:C1\]/.test(hd()));
		await ws('llv');
		await w('m');
		ok('m clears selection', hd().startsWith('[cursor') || g().includes('No changes'));
		r.unmount();
	}
	{
		// horizontal scroll
		const {r, g, w, ws, hd} = mk(fx);
		await boot();
		await ws('ijjjj');
		ok('hoff 0 at start', g().includes('word0 word1') && hd() === '[cursor L4:C1]');
		await w('$');
		ok('$ scrolls to end of long line', g().includes('END') && !g().includes('word0 word1') && /^\s+4 /m.test(g()));
		ok('gutter fixed while scrolled', /4 \+ ?.*END/m.test(g()) || /\+▶?.*END/.test(g()));
		await w('0');
		ok('0 scrolls back', g().includes('word0 word1'));
		await ws('120l');
		ok('mid scroll keeps cursor visible', hd() === '[cursor L4:C121]' && !g().includes('word0 word1'));
		await w('j');
		await w('\x1b');
		await w('\x1b');
		ok('hoff reset outside cursor mode', !g().includes('[cursor') && g().includes('word0 word1'));
		r.unmount();
	}
	{
		// split: cursor + selection on new side; hoff both sides
		const {r, g, w, ws, hd, qs} = mk(fx, {split: true}, 140);
		await boot();
		await ws('i');
		ok('split cursor on', g().includes('[split]') && hd() === '[cursor new L1:C1]');
		await ws('llvll');
		await w('\r');
		ok('split ask preview', g().includes('selection L1:C3-C5') && g().includes('> o.b'));
		await w('s');
		await w('\r');
		ok('split payload new side', qs[0]?.text === 'o.b' && qs[0]?.startLine === 1 && qs[0]?.message === 's');
		await ws('jjj');
		await w('$');
		ok('split hoff', g().includes('END') && !g().includes('word0 word1'));
		r.unmount();
	}
	{
		// split pane lock: p toggles pane, h/l never cross
		const sf = repo('a1\nold1\nz\n', 'a1\nnew line\nextra\nz\n');
		const {r, g, w, ws, hd, qs} = mk(sf, {split: true}, 120);
		await boot();
		await ws('i');
		ok('side: new default', hd() === '[cursor new L2:C1]');
		await w('h');
		ok('side: h at col 0 stays', hd() === '[cursor new L2:C1]');
		await w('$');
		await w('l');
		ok('side: l at end stays', hd() === '[cursor new L2:C8]');
		await w('p');
		ok('side: p -> old, no file switch', hd().startsWith('[cursor old L2:C') && g().includes('[1/1]'));
		ok('side: col clamped to old text', hd() === '[cursor old L2:C4]');
		await w('$');
		await w('l');
		ok('side: l at old end stays', hd() === '[cursor old L2:C4]');
		await w('j');
		ok('side: j keeps side, empty left cell col 0', hd() === '[cursor old r4:C1]' || hd().startsWith('[cursor old'));
		await w('$');
		ok('side: empty cell $ stays col 0', hd().endsWith(':C1]'));
		await w('l');
		ok('side: empty cell l stays', hd().endsWith(':C1]'));
		await w('k');
		ok('side: k keeps side', hd().startsWith('[cursor old L2'));
		await w('p');
		ok('side: p -> new', hd().startsWith('[cursor new L2'));
		await ws('pvl');
		ok('side: visual old', hd().startsWith('[visual old'));
		await w('p');
		ok('side: p ends visual', hd().startsWith('[cursor new'));
		await ws('p0vll');
		await w('\r');
		await w('q');
		await w('\r');
		ok(
			'side: old payload',
			qs.length === 1 && qs[0]!.side === 'old' && qs[0]!.text === 'old' && qs[0]!.startLine === 2 && qs[0]!.line === 2,
		);
		await w('s');
		ok('side: s to unified resets', g().includes('[unified]') && /^\[cursor (?:L|r)\d+:C\d+\]$/.test(hd()));
		await w('p');
		ok(
			'side: p in unified no-op',
			g().includes('[cursor') && /^\[cursor (?:L|r)\d+:C\d+\]$/.test(hd()) && g().includes('[1/1]'),
		);
		await w('s');
		ok('side: s back to split resets to new', g().includes('[split]') && /\[cursor new /.test(g()));
		r.unmount();
	}
	{
		// n and p never switch files
		const {r, g, ws} = mk(cwd);
		await boot();
		await ws('np');
		ok('n/p do nothing', g().includes('[1/3]') && g().includes('[unified]'));
		await ws('inp');
		ok('n/p in cursor mode do nothing', g().includes('[1/3]') && g().includes('[cursor'));
		r.unmount();
	}
	{
		// added row: old pane empty; deleted row: new pane empty; p outside cursor mode is prev file
		const df = repo('keep\ngone\nend\n', 'keep\nend\n');
		const {r, g, w, ws, hd, qs} = mk(df, {split: true}, 120);
		await boot();
		await ws('i');
		ok('side: deleted row cursor', /\[cursor new L2/.test(hd()) || hd().startsWith('[cursor'));
		await w('p');
		await w('p');
		await w('p');
		ok('side: deleted row toggles safely', g().includes('[cursor') && g().includes('[split]'));
		await w('\r');
		await ws('d');
		await w('\r');
		ok(
			'side: deleted row ask',
			qs.length === 1 && (qs[0]!.text === 'gone' || qs[0]!.text === '') && qs[0]!.side === 'old',
		);
		await w('\x1b');
		ok('side: cursor off', !g().includes('[cursor'));
		await w('p');
		ok(
			'side: p outside cursor mode no-op',
			!g().includes('[cursor') && g().includes('[1/1]') && g().includes('[split]'),
		);
		await w('n');
		ok('side: n unbound', g().includes('[1/1]') && g().includes('[split]'));
		r.unmount();
	}
	{
		// browse ignores side, p no-op
		const {r, g, w, ws, hd} = mk(fx, {split: true}, 120);
		await boot();
		await w('F');
		await w('w.ts');
		await w('\r');
		await tick();
		await ws('ijp');
		ok('side: browse no side suffix', g().includes('[browse]') && hd() === '[cursor L2:C1]');
		r.unmount();
	}
	{
		// browse
		const {r, g, w, ws, hd, qs} = mk(fx);
		await boot();
		await w('F');
		await w('w.ts');
		await w('\r');
		await tick();
		await ws('ij');
		ok('browse cursor', g().includes('[browse]') && hd() === '[cursor L2:C1]');
		await ws('jj');
		await w('$');
		ok('browse hoff', hd().startsWith('[cursor L4') && g().includes('END') && !g().includes('word0 word1'));
		await ws('0v$');
		await w('\r');
		ok('browse ask', g().includes('selection L4:C1-C') && g().includes('enter send'));
		await w('q');
		await w('\r');
		ok(
			'browse payload',
			qs[0]?.text === long && qs[0]?.startLine === 4 && qs[0]?.startCol === 1 && qs[0]?.endCol === long.length,
		);
		await w('\x1b');
		await w('\x1b');
		ok('esc order in browse', g().includes('[1/'));
		r.unmount();
	}
	{
		// sent questions stay inline under their anchor
		const {r, g, w, ws, hd, qs} = mk(fx);
		await boot();
		await ws('ijllvll');
		await w('\r');
		await w('why');
		await w('\r');
		ok(
			'sent: box stays with message + range',
			g().includes('sent') && g().includes('why') && g().includes('selection L1:C3-C5') && g().includes('> o.b'),
		);
		ok('sent: input closed, cursor mode', !g().includes('enter send') && hd().startsWith('[cursor') && qs.length === 1);
		await w('a');
		await w('note1');
		await w('\r');
		ok(
			'sent: two entries render',
			g().includes('why') && g().includes('note1') && g().includes('line L1') && qs.length === 2,
		);
		await ws('G');
		ok(
			'sent: cursor last row visible with boxes above',
			hd() === '[cursor L5:C4]' && g().includes('last') && g().includes('note1'),
		);
		await ws('g');
		await ws('jj');
		ok(
			'sent: persists after cursor away/back',
			g().includes('why') && g().includes('note1') && hd().startsWith('[cursor L1'),
		);
		await w('j');
		ok('sent: j skips over boxes to next diff row', hd() === '[cursor L2:C1]');
		await w('\x1b');
		ok('sent: persists outside cursor mode', g().includes('why') && !g().includes('[cursor'));
		await w('c');
		ok('sent: persists after c', g().includes('why') && g().includes('[changes]'));
		await w('c');
		await w('s');
		ok('sent: persists in split', g().includes('[split]') && g().includes('why') && g().includes('note1'));
		await w('s');
		ok('sent: persists after s back', g().includes('[unified]') && g().includes('why'));
		r.unmount();
	}
	{
		const {r, g, w, ws} = mk(fx, {split: true}, 140);
		await boot();
		await ws('ia');
		await ws('sp');
		await w('\r');
		ok(
			'sent: split',
			g().includes('[split]') && g().includes('sent') && g().includes('sp') && !g().includes('enter send'),
		);
		r.unmount();
	}
	{
		const {r, g, w, ws} = mk(fx);
		await boot();
		await w('F');
		await ws('w.ts');
		await w('\r');
		await tick();
		await ws('ivl');
		await w('a');
		await w('bm');
		await w('\r');
		ok(
			'sent: browse',
			g().includes('[browse]') && g().includes('sent') && g().includes('bm') && g().includes('selection L1:C1-C2'),
		);
		r.unmount();
	}
	{
		// anchor row missing (changes-only hides context), no crash, back again
		const {r, g, w, ws} = mk(cwd);
		await boot();
		await ws('\tigjj');
		await w('a');
		await ws('ctx');
		await w('\r');
		ok('sent: demo entry', g().includes('sent') && g().includes('ctx'));
		await w('c');
		ok(
			'sent: anchor gone in changes-only, no crash',
			g().includes('[changes]') && !g().includes('sent') && g().includes('big.ts'),
		);
		await w('c');
		await w('g');
		ok('sent: back in full', g().includes('[full]') && g().includes('ctx'));
		await w('\t');
		ok('sent: other file hides it', !g().includes('ctx') && g().includes('[3/'));
		r.unmount();
	}
	{
		// regression: s never closes the app
		const del = repo('x1\nx2\n', 'y1\n');
		const cases: [string, string, number | undefined, string][] = [
			['off', cwd, undefined, ''],
			['off wide', cwd, 120, ''],
			['cursor', cwd, undefined, 'i'],
			['cursor wide', cwd, 120, 'i'],
			['cursor G', cwd, 120, 'iG'],
			['visual', cwd, 120, 'ilv$'],
			['visual narrow', cwd, 80, 'ilv$'],
			['changes', cwd, 120, 'ci'],
			['fx cursor long', fx, 120, 'ijjjj$'],
			['fx visual V', fx, 120, 'ijVj'],
			['deleted row', del, 120, 'ij'],
			['deleted row narrow', del, 80, 'ijj'],
		];
		for (const [n, dir, cols, keys] of cases) {
			const {r, g, ws, w} = mk(dir, {}, cols);
			await boot();
			await ws(keys);
			await w('s');
			const want = cols && cols < 100 ? 'too narrow' : '[split]';
			ok(`s alive: ${n}`, g().includes(want) && g().includes('[split]') && !g().includes('Loading'));
			await w('s');
			ok(`s back: ${n}`, g().includes('[unified]'));
			await w('s');
			await ws('jkl');
			ok(`s then keys alive: ${n}`, g().includes('[split]') && g().length > 100);
			r.unmount();
		}
		{
			const {r, g, w, ws} = mk(cwd, {}, 120);
			await boot();
			await ws('iv');
			await w('\r');
			await w('s');
			ok('s is text while ask open', g().includes('enter send') && g().includes('[unified]'));
			await w('\x1b');
			await w('\x1b');
			r.unmount();
		}
		{
			const {r, g, w, ws} = mk(cwd, {}, 120);
			await boot();
			await w('F');
			await ws('a.ts');
			await w('\r');
			await tick();
			await ws('i');
			await w('s');
			ok('s in browse keeps app alive', g().includes('[browse]') && g().includes('[cursor'));
			r.unmount();
		}
	}
	{
		// comment focus / edit / delete
		const ups: Q[] = [];
		const dls: Q[] = [];
		const hooks = {
			onQuestionUpdate: (q: unknown) => ups.push(q as Q),
			onQuestionDelete: (q: unknown) => dls.push(q as Q),
		};
		const fresh = async (props: Record<string, unknown> = {}, cols?: number, dir = fx) => {
			ups.length = 0;
			dls.length = 0;
			const m = mk(dir, {...hooks, ...props}, cols);
			await boot();
			return m;
		};
		{
			const {r, g, w, ws, hd} = await fresh();
			await ws('ij');
			await ws('a');
			await ws('one');
			await w('\r');
			await ws('ja');
			await ws('two');
			await w('\r');
			await ws('ja');
			await ws('three');
			await w('\r');
			await w('g');
			await w('K');
			ok('focus: K from none goes to last, marker', g().includes('▸') && hd().includes('L3'));
			ok('hint: focused shows full hint', g().includes('e/enter edit  D delete  J/K prev/next  esc back'));
			ok('hint: only focused box has it', g().split('D delete').length === 3);
			await w('K');
			ok('focus: K prev', hd().includes('L2'));
			await w('K');
			await w('K');
			ok('focus: K stops at first', hd().includes('L1') && g().split('▸').length === 2);
			await w('J');
			await w('J');
			await w('J');
			ok('focus: J stops at last', hd().includes('L3'));
			await w('\x1b');
			ok('focus: esc unfocuses, stays cursor', !g().includes('▸') && g().includes('[cursor'));
			ok('hint: gone after esc', !g().includes('D delete'));
			ok('footer: cursor', g().includes('hjkl move  v select  enter ask  J/K comments') && !g().includes('i cursor'));
			await w('v');
			ok('footer: visual', g().includes('v/esc end  enter ask'));
			await w('\x1b');
			await w('K');
			ok('footer: comment focused', g().includes('esc back  ? help'));
			await w('\x1b');
			await w('J');
			await w('j');
			ok('focus: motion unfocuses then moves', !g().includes('▸') && g().includes('[cursor'));
			await w('J');
			await w('e');
			ok('edit: prefill + edit header', g().includes('enter send') && g().includes('edit line') && g().includes('one'));
			await ws('X');
			await w('\x1b');
			ok(
				'edit: esc cancels keeps old',
				!g().includes('enter send') && g().includes('one') && !g().includes('oneX') && ups.length === 0,
			);
			await w('e');
			await w('!');
			await w('\r');
			ok(
				'edit: save in place',
				g().includes('one!') &&
					g().includes('comment updated') &&
					g().includes('▸') &&
					ups.length === 1 &&
					ups[0]!.message === 'one!' &&
					qs_len(g()) === 3,
			);
			await w('\r');
			await ws('\x7f'.repeat(10));
			await w('\r');
			ok('edit: empty save stays open', g().includes('enter send') && ups.length === 1);
			await w('\x1b');
			ok('esc order: ask then focus', !g().includes('enter send') && g().includes('▸'));
			await w('D');
			ok('delete: modal', g().includes('Delete comment? (y/n)'));
			await w('x');
			ok('delete: other key ignored', g().includes('Delete comment?'));
			await w('n');
			ok('delete: n cancels', !g().includes('Delete comment?') && dls.length === 0 && g().includes('one!'));
			await w('D');
			await w('\x1b');
			ok('delete: esc cancels', !g().includes('Delete comment?') && dls.length === 0);
			await w('D');
			await w('y');
			ok(
				'delete: y deletes, focus moves next',
				!g().includes('one!') &&
					dls.length === 1 &&
					dls[0]!.message === 'one!' &&
					g().includes('comment deleted') &&
					g().includes('two') &&
					g().includes('▸'),
			);
			await w('J');
			await w('D');
			await w('\r');
			ok('delete: enter deletes, last unfocuses', !g().includes('three') && !g().includes('▸') && dls.length === 2);
			r.unmount();
		}
		{
			const {r, g, w, ws} = await fresh();
			await ws('ij');
			await w('J');
			ok('no comments note', g().includes('no comments') && !g().includes('▸'));
			await ws('a');
			await ws('aa');
			await w('\r');
			await ws('a');
			await ws('bb');
			await w('\r');
			await w('J');
			await w('J');
			ok('two on same row: second focused', g().split('▸').length === 2);
			await w('e');
			await w('\r');
			ok('two on same row: edit no dup', g().includes('aa') && g().includes('bb') && g().split('sent').length === 3);
			r.unmount();
		}
		{
			const {r, g, w, ws} = await fresh({}, 80);
			await ws('ij');
			await ws('a');
			await ws('nar');
			await w('\r');
			const before = g().split('\n').length;
			await w('K');
			ok(
				'footer: narrow no overflow',
				g()
					.split('\n')
					.every((l) => l.length <= 80),
			);
			ok('hint: narrow 80 shorter form fits', g().includes('D delete') && !g().includes('▸ sent  x'));
			ok(
				'hint: no line exceeds width',
				g()
					.split('\n')
					.every((l) => l.length <= 80),
			);
			ok('hint: viewport height unchanged on focus', g().split('\n').length === before);
			r.unmount();
		}
		{
			const {r, g, w, ws} = await fresh({split: true}, 140);
			await ws('ia');
			ok('footer: ask open', g().includes(') enter send  esc cancel'));
			await ws('sp');
			await w('\r');
			ok('footer: split cursor p pane', g().includes('p pane  esc exit'));
			await w('J');
			ok('split: focus', g().includes('[split]') && g().includes('▸'));
			ok('footer: focused split', g().includes('esc back  ? help'));
			ok('hint: split', g().includes('D delete') && g().includes('esc back'));
			await w('e');
			await w('Z');
			await w('\r');
			ok('split: edit', g().includes('spZ') && ups.length === 1);
			await w('D');
			await w('y');
			ok('split: delete', !g().includes('spZ') && dls.length === 1);
			r.unmount();
		}
		{
			const {r, g, w, ws} = await fresh();
			await w('F');
			await ws('w.ts');
			await w('\r');
			await tick();
			await ws('ia');
			await ws('bm');
			await w('\r');
			await w('J');
			ok('browse: focus', g().includes('[browse]') && g().includes('▸'));
			ok('hint: browse', g().includes('D delete') && g().includes('esc back'));
			await w('D');
			await w('y');
			ok('browse: delete', !g().includes('bm') && dls.length === 1);
			r.unmount();
		}
	}
}
console.log(splitFrame);
console.log(helpFrame);
for (const d of cleanup) rmSync(d, {recursive: true, force: true});
process.exit(fail ? 1 : 0);
