import {render} from 'ink-testing-library';
import App from '../src/app.js';
import {join} from 'node:path';
import {settle, waitFor} from './helpers.js';
import {cwd, tick, ok, keyPress, tmpDir, finish} from './keysHelpers.js';
{
	type Cur = {index: number; row?: {kind: string; text?: string}} | undefined;
	const mk = (props: Record<string, unknown> = {}) => {
		let c: Cur;
		const r = render(<App args={[]} cwd={cwd} onCursor={(x) => (c = x as Cur)} {...props} />);
		const g = () => r.lastFrame() ?? '';
		const w = keyPress(r);
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
		await w('j');
		ok('j moves', cur()?.index === 1);
		await w('k');
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
		await w('i');
		await w('j');
		await w('j');
		ok('browse cursor moves, indicator L', cur()?.index === 2 && /\[cursor L3:C\d+\]/.test(g()));
		await w('\x1b');
		ok('esc exits cursor only, stays browse', g().includes('[browse]') && !g().includes('[cursor'));
		await w('\x1b');
		await w('i');
		await w('t');
		await w('m');
		ok('t/m work in cursor mode', (g().includes('[cursor') && g().includes('[staged]')) || g().includes('No changes'));
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
	const boot = async (g: () => string) => {
		await waitFor(() => /\[\d+\/\d+\]/.test(g()), {timeout: 3000});
		await settle(20);
	};
	{
		const {r, g, w, ws, hd} = mk(fx);
		await boot(g);
		await ws('ij');
		ok('char: start col 1', hd() === '[cursor L1:C1]');
		await w('h');
		ok('h clamps at 0', hd() === '[cursor L1:C1]');
		await w('l');
		ok('l moves', hd() === '[cursor L1:C2]');
		await ws('3l');
		ok('3l count', hd() === '[cursor L1:C5]');
		await w('h');
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
		await ws('ww');
		ok('w: baz -> ,', hd() === '[cursor L1:C12]');
		await w('w');
		ok('w skips blank to 12', hd() === '[cursor L1:C14]');
		await w('w');
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
		r.unmount();
	}
	{
		const {r, g, w, ws, hd, qs} = mk(fx);
		await boot(g);
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
		await boot(g);
		await w('\x1b[C');
		ok('right arrow outside cursor mode switches file', g().includes('[2/'));
		await w('\x1b[D');
		await ws('ilv');
		await w('l');
		await w('\t');
		ok('file switch clears selection, col 0', /\[cursor [Lr]\d+:C1\]/.test(hd()));
		await ws('lllv');
		await w('c');
		ok('c clears selection, col 0', /\[cursor [Lr]\d+:C1\]/.test(hd()));
		await ws('llv');
		await w('m');
		r.unmount();
	}
	{
		// horizontal scroll
		const {r, g, w, ws, hd} = mk(fx);
		await boot(g);
		await ws('ijjjj');
		ok('hoff 0 at start', g().includes('word0 word1') && hd() === '[cursor L4:C1]');
		await w('$');
		ok('$ scrolls to end of long line', g().includes('END') && !g().includes('word0 word1') && /^\s+4 /m.test(g()));
		ok('gutter fixed while scrolled', /4 \+ ?.*END/m.test(g()) || /\+▶?.*END/.test(g()));
		await w('0');
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
		await boot(g);
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
		await boot(g);
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
		await w('j');
		ok('side: j keeps side, empty left cell col 0', hd() === '[cursor old r4:C1]' || hd().startsWith('[cursor old'));
		await w('$');
		await w('l');
		await w('k');
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
		// added row: old pane empty; deleted row: new pane empty; p outside cursor mode is prev file
		const df = repo('keep\ngone\nend\n', 'keep\nend\n');
		const {r, g, w, ws, qs} = mk(df, {split: true}, 120);
		await boot(g);
		await ws('i');
		await w('p');
		await w('p');
		await w('p');
		await w('\r');
		await ws('d');
		await w('\r');
		ok(
			'side: deleted row ask',
			qs.length === 1 && (qs[0]!.text === 'gone' || qs[0]!.text === '') && qs[0]!.side === 'old',
		);
		await w('\x1b');
		await w('p');
		ok(
			'side: p outside cursor mode no-op',
			!g().includes('[cursor') && g().includes('[1/1]') && g().includes('[split]'),
		);
		await w('n');
		r.unmount();
	}
	{
		// browse ignores side, p no-op
		const {r, g, w, ws, hd} = mk(fx, {split: true}, 120);
		await boot(g);
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
		await boot(g);
		await w('F');
		await w('w.ts');
		await w('\r');
		await tick();
		await ws('ij');
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
		r.unmount();
	}
	{
		// sent questions stay inline under their anchor
		const {r, g, w, ws, hd, qs} = mk(fx);
		await boot(g);
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
		await w('c');
		await w('s');
		ok('sent: persists in split', g().includes('[split]') && g().includes('why') && g().includes('note1'));
		await w('s');
		r.unmount();
	}
}

finish();
