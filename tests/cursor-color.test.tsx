process.env.FORCE_COLOR = '3';
const {render} = await import('ink-testing-library');
const {DiffView, CUR_MARK} = await import('../src/components/DiffView.js');
const {ThemeContext, THEMES} = await import('../src/theme.js');
type ThemeName = keyof typeof THEMES;
let fail = 0;
const ok = (n: string, c: boolean) => {
	console.log(c ? 'PASS' : 'FAIL', n);
	if (!c) fail++;
};
const bgEsc = (hex: string) => {
	const n = parseInt(hex.slice(1), 16);
	return `\x1b[48;2;${n >> 16};${(n >> 8) & 255};${n & 255}m`;
};
const rows: any[] = [
	{kind: 'line', type: 'normal', oldNo: 1, newNo: 1, text: 'const a = "x";'},
	{kind: 'line', type: 'add', newNo: 2, text: 'const b = 2;'},
	{kind: 'line', type: 'del', oldNo: 2, text: 'const c = 3;'},
];
const file: any = {path: 'a.ts', hunks: []};
const frame = (name: ThemeName, r: any[], cur: number, single = false, extra: any = {}) =>
	(
		render(
			<ThemeContext.Provider value={THEMES[name]}>
				<DiffView
					file={file}
					rows={r}
					offset={0}
					height={r.length}
					cols={60}
					name={name}
					cur={cur}
					single={single}
					{...extra}
				/>
			</ThemeContext.Provider>,
		).lastFrame() ?? ''
	).split('\n');
// no bg-cut: after first cursor bg escape, only a final 49m (no 0m, no 49m mid-row unless followed by reopen of same bg)
const persists = (line: string, esc: string) => {
	const at = line.indexOf(esc);
	if (at < 0 || line.includes('\x1b[0m')) return false;
	const rest = line.slice(at);
	const cuts = [...rest.matchAll(/\x1b\[49m/g)].map((m) => m.index!);
	return cuts.every((i) => i + 5 === rest.length || rest.startsWith(esc, i + 5) || rest.slice(i + 5).startsWith(esc));
};
for (const name of ['contrast', 'light', 'solarized', 'vibrant', 'dull', 'colorblind'] as ThemeName[]) {
	const esc = bgEsc(THEMES[name].curBg);
	for (const [k, label] of [
		[0, 'plain'],
		[1, 'added'],
		[2, 'deleted'],
	] as const) {
		const lines = frame(name, rows, k);
		ok(`${name} ${label}: cursor row has bg+marker`, lines[k]!.includes(esc) && lines[k]!.includes(CUR_MARK));
		// full-width persistence + no leak to other rows: same code path per theme, check on 2 themes
		if (name === 'contrast' || name === 'light') {
			ok(`${name} ${label}: bg persists`, persists(lines[k]!, esc));
			ok(
				`${name} ${label}: others no bg/marker`,
				lines.every((l, i) => i === k || (!l.includes(esc) && !l.includes(CUR_MARK))),
			);
		}
	}
}
// split whole row
const pair: any[] = [
	{kind: 'pair', l: rows[2], r: rows[1]},
	{kind: 'pair', l: rows[0], r: rows[0]},
];
for (const name of ['contrast'] as ThemeName[]) {
	const esc = bgEsc(THEMES[name].curBg);
	const lines = frame(name, pair, 0);
	ok(`${name} split: cursor row bg+marker`, lines[0]!.includes(esc) && lines[0]!.includes(CUR_MARK));
	ok(`${name} split: other row clean`, !lines[1]!.includes(esc) && !lines[1]!.includes(CUR_MARK));
}
// hunk row + browse (single)
const hunk: any[] = [{kind: 'hunk', text: '@@ -1 +1 @@'}, rows[0]];
ok('hunk cursor bg', frame('contrast', hunk, 0)[0]!.includes(bgEsc(THEMES.contrast.curBg)));
ok('browse cursor', frame('light', rows, 1, true)[1]!.includes(CUR_MARK));

// char cursor block + selection
const strip = (x: string) => x.replace(/\x1b\[[0-9;]*m/g, '');
const INV = '\x1b[7m';
for (const name of ['contrast', 'light', 'solarized', 'vibrant', 'dull', 'colorblind'] as ThemeName[]) {
	const T = THEMES[name];
	const l0 = frame(name, rows, 0, false, {col: 6})[0]!;
	const at = l0.indexOf(INV);
	ok(
		`${name} block: inverse at col`,
		at > 0 && strip(l0.slice(0, at)).length === 12 + 6 && strip(l0.slice(at)).startsWith('a'),
	);
	if (name === 'contrast') {
		ok('block: row bg kept', l0.includes(bgEsc(T.curBg)) && l0.includes(CUR_MARK));
		ok('block: only cursor row', frame(name, rows, 0, false, {col: 6})[1]!.indexOf(INV) < 0);
	}
	const s = {sr: 0, sc: 2, er: 0, ec: 4, line: false};
	const l1 = frame(name, rows, 0, false, {col: 4, sel: s})[0]!;
	const m = new RegExp(bgEsc(T.visBg).replace(/[\[\]]/g, '\\$&') + '([^]*?)\x1b\\[49m').exec(l1);
	ok(`${name} sel: bg over selected chars only`, !!m && strip(m[1]!).startsWith('ns'));
	ok(`${name} sel: distinct from cursor bg`, T.visBg !== T.curBg && l1.includes(bgEsc(T.curBg)));
	if (name === 'contrast') {
		ok('sel: not on other rows', !frame(name, rows, 0, false, {col: 4, sel: s})[1]!.includes(bgEsc(T.visBg)));
	}
}
{
	const e = frame('light', [{kind: 'line', type: 'normal', oldNo: 1, newNo: 1, text: ''}], 0, false, {col: 0})[0]!;
	ok('empty line: block on a space', strip(e.slice(e.indexOf(INV))).startsWith(' ') && e.includes(INV));
	const h = frame('contrast', rows, 0, false, {col: 6, hoff: 3})[0]!;
	const at = h.indexOf(INV);
	ok(
		'hoff: gutter fixed, block shifted',
		at > 0 && strip(h.slice(0, at)).length === 12 + 3 && strip(h.slice(at)).startsWith('a'),
	);
	const two = [rows[0], {kind: 'line', type: 'normal', oldNo: 2, newNo: 2, text: 'second row'}];
	const ml = frame('solarized', two, 0, false, {col: 3, sel: {sr: 0, sc: 8, er: 1, ec: 2, line: false}});
	const bg = bgEsc(THEMES.solarized.visBg);
	ok('multi-row sel on both rows', ml[0]!.includes(bg) && ml[1]!.includes(bg));
	const ll = frame('contrast', two, 1, false, {col: 0, sel: {sr: 0, sc: 0, er: 1, ec: 0, line: true}});
	ok(
		'linewise sel whole rows',
		ll.every((x) => x.includes(bgEsc(THEMES.contrast.visBg))),
	);
	const sp = frame('light', pair, 0, false, {col: 2});
	ok(
		'split: block on new side only',
		sp[0]!.split(INV).length === 2 && strip(sp[0]!.slice(sp[0]!.indexOf(INV))).startsWith('n'),
	);
	// pane side: block only in the active pane cell
	const so = frame('light', pair, 0, false, {col: 6, side: 'old'})[0]!;
	const sn = frame('light', pair, 0, false, {col: 6, side: 'new'})[0]!;
	const bar = (x: string) => strip(x).indexOf('│');
	ok(
		'split side old: block in left pane',
		so.split(INV).length === 2 &&
			strip(so.slice(0, so.indexOf(INV))).length < bar(so) &&
			strip(so.slice(so.indexOf(INV))).startsWith('c'),
	);
	ok(
		'split side new: block in right pane',
		sn.split(INV).length === 2 &&
			strip(sn.slice(0, sn.indexOf(INV))).length > bar(sn) &&
			strip(sn.slice(sn.indexOf(INV))).startsWith('b'),
	);
	const add: any[] = [{kind: 'pair', l: undefined, r: rows[1]}];
	const eo = frame('light', add, 0, false, {col: 0, side: 'old'})[0]!;
	ok(
		'split empty left cell: block on a space in left pane',
		eo.split(INV).length === 2 &&
			strip(eo.slice(0, eo.indexOf(INV))).length < bar(eo) &&
			strip(eo.slice(eo.indexOf(INV))).startsWith(' '),
	);
	const en = frame('light', add, 0, false, {col: 0, side: 'new'})[0]!;
	ok('split added row side new: block in right pane', strip(en.slice(0, en.indexOf(INV))).length > bar(en));
	const del: any[] = [{kind: 'pair', l: rows[2], r: undefined}];
	const dn = frame('light', del, 0, false, {col: 0, side: 'new'})[0]!;
	ok('split deleted row side new falls back to left pane', strip(dn.slice(0, dn.indexOf(INV))).length < bar(dn));
}
{
	// focused comment: accent border + marker differ from unfocused
	const sentMap = (focused: boolean) =>
		new Map<number, any[]>([[0, [{head: 'line L1', lines: [], body: [{t: 'hi', k: 'msg'}], focused}]]]);
	for (const name of ['contrast', 'light', 'solarized', 'vibrant', 'dull', 'colorblind'] as ThemeName[]) {
		const a = frame(name, rows, -1, false, {sent: sentMap(true), height: 6}).join('\n');
		const b = frame(name, rows, -1, false, {sent: sentMap(false), height: 6}).join('\n');
		if (name === 'contrast') ok('focused comment: marker differs', a.includes('▸') && !b.includes('▸'));
		const esc = (x: string) => x.match(/\x1b\[[0-9;]*m/g)?.join('') ?? '';
		ok(`${name} focused comment: border color differs`, esc(a) !== esc(b));
	}
}
console.log(fail ? `${fail} FAILED` : 'ALL PASS');
process.exit(fail ? 1 : 0);
