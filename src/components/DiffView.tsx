import {Box, Text} from 'ink';
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

const pad = (n?: number) => (n === undefined ? '    ' : String(n).padStart(4));

const look = (t: Theme, type: 'add' | 'del' | 'normal') =>
	type === 'add'
		? {bg: t.addBg, mark: '+', fg: t.addMark}
		: type === 'del'
			? {bg: t.delBg, mark: '-', fg: t.delMark}
			: {bg: undefined, mark: ' ', fg: t.gutter};

function Side({l, no, file, w, name}: {l?: Ln; no?: number; file: DiffFile; w: number; name: ThemeName}) {
	const t = useTheme();
	if (!l) return <Box width={w} />;
	const {bg, mark, fg} = look(t, l.type);
	return (
		<Box width={w}>
			<Text wrap="truncate">
				<Text backgroundColor={bg} color={t.gutter}>
					{pad(no)}{' '}
				</Text>
				<Text backgroundColor={bg} color={fg} bold>
					{mark}{' '}
				</Text>
				{hl(l.text.replace(/\t/g, '  '), file.path, name)}
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
}: {
	file: DiffFile;
	rows: (Row | SRow)[];
	offset: number;
	height: number;
	cols: number;
	name: ThemeName;
	single?: boolean;
}) {
	const t = useTheme();
	const w = Math.floor((cols - 1) / 2);
	return (
		<Box flexDirection="column" height={height}>
			{rows.slice(offset, offset + height).map((r, i) => {
				if (r.kind === 'hunk')
					return (
						<Text key={i} color={t.hunk} wrap="truncate">
							{r.text}
						</Text>
					);
				if (r.kind === 'pair')
					return (
						<Box key={i}>
							<Side l={r.l} no={r.l?.oldNo} file={file} w={w} name={name} />
							<Text color={t.dim}>│</Text>
							<Side l={r.r} no={r.r?.newNo} file={file} w={w} name={name} />
						</Box>
					);
				if (r.kind === 'note')
					return (
						<Text key={i} color={t.dim}>
							{r.text}
						</Text>
					);
				const {bg, mark, fg} = look(t, r.type);
				return (
					<Text key={i} wrap="truncate">
						<Text backgroundColor={bg} color={t.gutter}>
							{single ? pad(r.newNo) : `${pad(r.oldNo)} ${pad(r.newNo)}`}{' '}
						</Text>
						<Text backgroundColor={bg} color={fg} bold>
							{mark}{' '}
						</Text>
						{hl(r.text.replace(/\t/g, '  '), file.path, name)}
					</Text>
				);
			})}
		</Box>
	);
}
