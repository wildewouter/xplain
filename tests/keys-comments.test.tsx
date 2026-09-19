import {render} from 'ink-testing-library';
import App from '../src/app.js';
import {join} from 'node:path';
import {readFileSync} from 'node:fs';
import {settle, waitFor} from './helpers.js';
import {cwd, tick, ok, keyPress, tmpDir, finish} from './keysHelpers.js';
{
	// prompt window + quit confirm
	const dir = tmpDir('keys-ask');
	type Q = {file: string; index: number; line?: number; text: string; message: string};
	const mk = (props: Record<string, unknown> = {}) => {
		const qs: Q[] = [];
		const r = render(<App args={[]} cwd={cwd} onQuestion={(q) => qs.push(q as Q)} {...props} />);
		const g = () => r.lastFrame() ?? '';
		const w = keyPress(r);
		return {r, g, w, qs};
	};
	{
		const {r, g, w, qs} = mk();
		await tick();
		await w('\r');
		ok('enter outside cursor mode: no box', !g().includes('enter send'));
		await w('a');
		await w('i');
		await w('j');
		await w('\r');
		ok('enter opens box', g().includes('enter send  tab save/ask  esc cancel') && g().includes('[cursor'));
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
		const {r, w, qs} = mk({split: true});
		await tick();
		await w('\t');
		await w('i');
		await w('j');
		await w('a');
		await w('hi');
		await w('\r');
		ok('split: submit', qs.length === 1 && qs[0]!.message === 'hi' && typeof qs[0]!.text === 'string');
		r.unmount();
	}
	{
		const {r, w, qs} = mk();
		await tick();
		await w('F');
		await w('a.ts');
		await w('\r');
		await tick();
		await w('i');
		await w('j');
		await w('j');
		await w('\r');
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
	const {writeFileSync} = await import('node:fs');
	const repo = (oldT: string, newT: string) => {
		const d = tmpDir('vis-');
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
		const w = keyPress(r);
		const ws = async (ks: string) => {
			for (const k of ks) await w(k);
		};
		const hd = () => /\[(?:cursor|visual)(?: old| new)? [Lr]\d+:C\d+\]/.exec(g())?.[0] ?? '';
		return {r, g, w, ws, hd, qs};
	};
	const qs_len = (t: string) => t.split('sent').length - 1;
	const boot = async (g: () => string) => {
		await waitFor(() => /\[\d+\/\d+\]/.test(g()), {timeout: 3000});
		await settle(20);
	};
	{
		const {r, g, w, ws} = mk(fx, {split: true}, 140);
		await boot(g);
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
		await boot(g);
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
		await boot(g);
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
			['cursor', cwd, undefined, 'i'],
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
			await boot(g);
			await ws(keys);
			await w('s');
			const want = cols && cols < 100 ? 'too narrow' : '[split]';
			ok(`s alive: ${n}`, g().includes(want) && g().includes('[split]') && !g().includes('Loading'));
			await w('s');
			await w('s');
			await ws('jkl');
			ok(`s then keys alive: ${n}`, g().includes('[split]') && g().length > 100);
			r.unmount();
		}
		{
			const {r, g, w, ws} = mk(cwd, {}, 120);
			await boot(g);
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
			await boot(g);
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
			await boot(m.g);
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
			ok('hint: focused shows full hint', g().includes('e edit  D delete  a ask  esc back'));
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
			await w('n');
			ok('delete: n cancels', !g().includes('Delete comment?') && dls.length === 0 && g().includes('one!'));
			await w('D');
			await w('\x1b');
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
			ok('hint: viewport height unchanged on focus', g().split('\n').length === before);
			r.unmount();
		}
		{
			const {r, g, w, ws} = await fresh({split: true}, 140);
			await ws('ia');
			ok('footer: ask open', g().includes(') enter send  tab save/ask  esc cancel'));
			await ws('sp');
			await w('\r');
			ok('footer: split cursor p pane', g().includes('p pane  esc exit'));
			await w('J');
			ok('split: focus', g().includes('[split]') && g().includes('▸'));
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
			await w('D');
			await w('y');
			ok('browse: delete', !g().includes('bm') && dls.length === 1);
			r.unmount();
		}
	}
}

{
	// export (E): temp git repo so the fixture stays clean
	const {execSync} = await import('node:child_process');
	const {writeFileSync, readdirSync} = await import('node:fs');
	const dir = tmpDir('keys-export');
	const sh = (c: string) => execSync(c, {cwd: dir, stdio: 'ignore'});
	sh('git init -q');
	writeFileSync(join(dir, 'hello.txt'), 'hello world\nsecond line\n');
	sh('git add -A');
	sh('git -c user.email=a@b -c user.name=n commit -qm init');
	const r = render(<App args={[]} cwd={dir} />);
	const g = () => r.lastFrame() ?? '';
	const w = keyPress(r);
	await new Promise((x) => setTimeout(x, 400));
	const files = () => readdirSync(dir).filter((f) => f.startsWith('xplain-review-'));
	await w('E');
	ok('export: no comments noted, no file', g().includes('no comments to export') && files().length === 0);
	await w('F');
	await w('hello');
	await w('\r');
	await tick();
	await w('i');
	await w('\r');
	await w('my `note`');
	await w('\r');
	await w('E');
	await waitFor(() => files().length === 1);
	const f = files()[0]!;
	await tick();
	ok('export: file name', /^xplain-review-\d{8}-\d{6}\.md$/.test(f));
	ok('export: note shown', g().includes('exported 1 comment'));
	const md = readFileSync(join(dir, f), 'utf8');
	ok('export: content', md.includes('hello.txt') && md.includes('my `note`') && md.includes('state: saved'));
	await w('a');
	await w('x');
	await w('E');
	ok('export: E is text while typing', g().includes('xE') && files().length === 1);
	r.unmount();
}

finish();
