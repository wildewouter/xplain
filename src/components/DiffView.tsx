import {Fragment, type ReactNode} from 'react';
import {Box, Text} from 'ink';
import {AskBox, askH, SentBox, sentH, type AskSel, type SentQ} from './AskBox.js';
import type {DiffFile} from '../diff/load.js';
import {hl} from '../highlight.js';
import {useTheme, type Theme, type ThemeName} from '../theme.js';

export type Row =
	| {kind: 'hunk'; text: string}
	| {kind: 'note'; text: string}
	| {kind: 'line'; type: 'add' | 'del' | 'normal'; oldNo?: number; newNo?: number; text: string};

export const toRows = (f: DiffFile): Row[] => [
	...(f.note ? [{kind: 'note', text: f.note} as Row] : []),
	...f.hunks.flatMap((h) => [
		{kind: 'hunk', text: h.header} as Row,
		...h.lines.map((l) => ({kind: 'line', ...l}) as Row),
	]),
];

type Ln = Extract<Row, {kind: 'line'}>;
export type SRow = Exclude<Row, {kind: 'line'}> | {kind: 'pair'; l?: Ln; r?: Ln};

export const toSplit = (rows: Row[]): SRow[] => {
	const out: SRow[] = [];
	let dels: Ln[] = [];
	let adds: Ln[] = [];
	const flush = () => {
		for (let i = 0; i < Math.max(dels.length, adds.length); i++) out.push({kind: 'pair', l: dels[i], r: adds[i]});
		dels = [];
		adds = [];
	};
	for (const r of rows) {
		if (r.kind !== 'line') (flush(), out.push(r));
		else if (r.type === 'del') dels.push(r);
		else if (r.type === 'add') adds.push(r);
		else (flush(), out.push({kind: 'pair', l: r, r}));
	}
	flush();
	return out;
};

const isChg = (r: Row | SRow) =>
	r.kind === 'line' ? r.type !== 'normal' : r.kind === 'pair' && (r.l?.type !== 'normal' || r.r?.type !== 'normal');
export const changeStarts = (rows: (Row | SRow)[]): number[] =>
	rows.flatMap((r, i) => (isChg(r) && !(i > 0 && isChg(rows[i - 1]!)) ? [i] : []));

export type PaneSide = 'old' | 'new';
// side 'old' (split only) = left pane's own line number; empty left cell has none
export const rowNo = (r?: Row | SRow, side: PaneSide = 'new') =>
	!r
		? undefined
		: r.kind === 'line'
			? (r.newNo ?? r.oldNo)
			: r.kind === 'pair'
				? side === 'old'
					? r.l?.oldNo
					: (r.r?.newNo ?? r.l?.oldNo)
				: undefined;
// pane that really carries the cursor: deleted rows always use the left pane
export const paneOf = (r: Row | SRow | undefined, side: PaneSide): PaneSide =>
	r?.kind === 'pair' && (side === 'old' || !r.r) ? 'old' : 'new';

// non-color cursor cue, survives any theme
export const CUR_MARK = '▶';
const pad = (n?: number) => (n === undefined ? '    ' : String(n).padStart(4));

export const expand = (s: string) => s.replace(/\t/g, '  ');
// visible text of a row for the char cursor: chosen pane of a pair (new falls back to old on deleted rows)
export const rowCode = (r?: Row | SRow, side: PaneSide = 'new') =>
	!r ? '' : r.kind === 'pair' ? expand((paneOf(r, side) === 'old' ? r.l : r.r)?.text ?? '') : expand(r.text);
// inclusive char selection, ordered; line = whole rows
export type Sel = {sr: number; sc: number; er: number; ec: number; line: boolean};
export type Cell = {col: number; sel?: [number, number]; hoff: number}; // sel = [from, to) in this row

// code text as segments: plain (syntax highlighted), selection (visBg), cursor block (inverse)
function code(text: string, path: string, name: ThemeName, t: Theme, cell?: Cell, on = false): ReactNode {
	const hoff = cell?.hoff ?? 0;
	if (!cell || (!on && !cell.sel)) return hl(hoff ? text.slice(hoff) : text, path, name);
	const cur = on ? cell.col : -1;
	const need = Math.max(cur + 1, cell.sel?.[1] ?? 0);
	const tx = text.length < need ? text.padEnd(need) : text;
	const v = tx.slice(hoff);
	const [a, b] = cell.sel ? [cell.sel[0] - hoff, cell.sel[1] - hoff] : [-1, -1];
	const c = cur - hoff;
	const cuts = [...new Set([0, a, b, c, c + 1, v.length])].filter((x) => x >= 0 && x <= v.length).sort((x, y) => x - y);
	const out: ReactNode[] = [];
	for (let k = 0; k + 1 < cuts.length; k++) {
		const [x, y] = [cuts[k]!, cuts[k + 1]!];
		const seg = v.slice(x, y);
		const inSel = x >= a && y <= b;
		if (x === c && on)
			out.push(
				<Text key={k} inverse backgroundColor={inSel ? t.visBg : undefined} color={inSel ? t.visFg : undefined}>
					{seg}
				</Text>,
			);
		else if (inSel)
			out.push(
				<Text key={k} backgroundColor={t.visBg} color={t.visFg}>
					{seg}
				</Text>,
			);
		else out.push(<Fragment key={k}>{hl(seg, path, name)}</Fragment>);
	}
	return out;
}

const look = (t: Theme, type: 'add' | 'del' | 'normal') =>
	type === 'add'
		? {bg: t.addBg, mark: '+', fg: t.addMark}
		: type === 'del'
			? {bg: t.delBg, mark: '-', fg: t.delMark}
			: {bg: undefined, mark: ' ', fg: t.gutter};

function Side({
	l,
	no,
	file,
	w,
	name,
	cur,
	cell,
	active,
}: {
	l?: Ln;
	no?: number;
	file: DiffFile;
	w: number;
	name: ThemeName;
	cur?: boolean;
	cell?: Cell;
	active?: boolean; // side carrying the char cursor / selection
}) {
	const t = useTheme();
	const cb = cur ? t.curBg : undefined;
	if (!l) {
		if (!(cur && active)) return <Box width={w} backgroundColor={cb} />;
		// empty pane cell still carries the cursor block
		return (
			<Box width={w} backgroundColor={cb}>
				<Text wrap="truncate">
					<Text backgroundColor={cb}>{'      ' + CUR_MARK}</Text>
					{code('', file.path, name, t, cell ?? {col: 0, hoff: 0}, true)}
					{' '.repeat(w)}
				</Text>
			</Box>
		);
	}
	const {bg, mark, fg} = look(t, l.type);
	const bgc = cb ?? bg;
	return (
		<Box width={w} backgroundColor={cb}>
			<Text wrap="truncate">
				<Text backgroundColor={bgc} color={t.gutter}>
					{pad(no)}{' '}
				</Text>
				<Text backgroundColor={bgc} color={fg} bold>
					{mark}
					{cur ? CUR_MARK : ' '}
				</Text>
				{code(expand(l.text), file.path, name, t, cell, !!cur && !!active)}
				{cur ? ' '.repeat(w) : ''}
			</Text>
		</Box>
	);
}

export function DiffView({
	file,
	rows,
	offset,
	height,
	cols,
	name,
	single,
	cur = -1,
	ask,
	col = 0,
	sel,
	hoff = 0,
	askSel,
	sent,
	side = 'new',
}: {
	file: DiffFile;
	rows: (Row | SRow)[];
	offset: number;
	height: number;
	cols: number;
	name: ThemeName;
	single?: boolean;
	cur?: number; // absolute row index of cursor, -1 none
	ask?: {text: string; pos: number}; // input box below cursor row
	col?: number; // char cursor column (0-based, clamped by caller)
	sel?: Sel; // visual selection
	hoff?: number; // horizontal scroll (code text only)
	askSel?: AskSel;
	sent?: Map<number, SentQ[]>; // submitted questions by anchor row
	side?: PaneSide; // split: pane carrying the char cursor
}) {
	const t = useTheme();
	const w = Math.floor((cols - 1) / 2);
	// rows that fit, counting inline boxes (sent questions, ask box under the cursor row)
	const vis: {r: Row | SRow; ri: number}[] = [];
	let used = 0;
	for (let ri = offset; ri < rows.length; ri++) {
		const h = 1 + (sent?.get(ri) ?? []).reduce((n, q) => n + sentH(q), 0) + (ask && ri === cur ? askH(askSel) : 0);
		if (used + h > height && used > 0) break;
		used += h;
		vis.push({r: rows[ri]!, ri});
	}
	return (
		<Box flexDirection="column" height={height}>
			{vis.flatMap(({r, ri}, i) => {
				const c = ri === cur;
				const cb = c ? t.curBg : undefined;

				const cellFor = (txt: string): Cell | undefined => {
					let s2: [number, number] | undefined;
					if (sel && ri >= sel.sr && ri <= sel.er) {
						const n = txt.length;
						const a = ri === sel.sr && !sel.line ? sel.sc : 0;
						const b = ri === sel.er && !sel.line ? sel.ec + 1 : n;
						s2 = n === 0 ? [0, 1] : [Math.min(a, n), Math.min(Math.max(b, a + 1), n)];
					}
					return c || s2 ? {col, sel: s2, hoff} : hoff ? {col, hoff} : undefined;
				};
				const el = (() => {
					if (r.kind === 'hunk')
						return (
							<Text key={i} color={t.hunk} wrap="truncate" backgroundColor={cb}>
								{r.text}
								{c ? ' '.repeat(cols) : ''}
							</Text>
						);
					if (r.kind === 'pair')
						return (
							<Box key={i} backgroundColor={cb}>
								<Side
									l={r.l}
									no={r.l?.oldNo}
									file={file}
									w={w}
									name={name}
									cur={c}
									active={paneOf(r, side) === 'old'}
									cell={paneOf(r, side) === 'old' ? cellFor(expand(r.l?.text ?? '')) : hoff ? {col, hoff} : undefined}
								/>
								<Text color={t.dim} backgroundColor={cb}>
									│
								</Text>
								<Side
									l={r.r}
									no={r.r?.newNo}
									file={file}
									w={w}
									name={name}
									cur={c}
									active={paneOf(r, side) === 'new'}
									cell={r.r && paneOf(r, side) === 'new' ? cellFor(expand(r.r.text)) : hoff ? {col, hoff} : undefined}
								/>
							</Box>
						);
					if (r.kind === 'note')
						return (
							<Text key={i} color={t.dim} wrap="truncate" backgroundColor={cb}>
								{r.text}
								{c ? ' '.repeat(cols) : ''}
							</Text>
						);
					const {bg, mark, fg} = look(t, r.type);
					const bgc = cb ?? bg;
					return (
						<Text key={i} wrap="truncate" backgroundColor={cb}>
							<Text backgroundColor={bgc} color={t.gutter}>
								{single ? pad(r.newNo) : `${pad(r.oldNo)} ${pad(r.newNo)}`}{' '}
							</Text>
							<Text backgroundColor={bgc} color={fg} bold>
								{mark}
								{c ? CUR_MARK : ' '}
							</Text>
							{code(expand(r.text), file.path, name, t, cellFor(expand(r.text)), c)}
							{c ? ' '.repeat(cols) : ''}
						</Text>
					);
				})();
				const bw = Math.max(10, cols - 1);
				return [
					el,
					...(sent?.get(ri) ?? []).map((q, k) => <SentBox key={`sent${i}-${k}`} q={q} width={bw} />),
					...(ask && c ? [<AskBox key={`ask${i}`} text={ask.text} pos={ask.pos} width={bw} sel={askSel} />] : []),
				];
			})}
		</Box>
	);
}
