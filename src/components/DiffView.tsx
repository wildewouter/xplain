import {Box, Text} from 'ink';
import type {DiffFile} from '../diff/load.js';
import {hl} from '../highlight.js';

export type Row = {kind: 'hunk'; text: string} | {kind: 'note'; text: string} | {kind: 'line'; type: 'add' | 'del' | 'normal'; oldNo?: number; newNo?: number; text: string};

export const toRows = (f: DiffFile): Row[] => [
	...(f.note ? [{kind: 'note', text: f.note} as Row] : []),
	...f.hunks.flatMap(h => [{kind: 'hunk', text: h.header} as Row, ...h.lines.map(l => ({kind: 'line', ...l}) as Row)]),
];

const pad = (n?: number) => (n === undefined ? '    ' : String(n).padStart(4));

export function DiffView({file, rows, offset, height}: {file: DiffFile; rows: Row[]; offset: number; height: number}) {
	return (
		<Box flexDirection="column" height={height}>
			{rows.slice(offset, offset + height).map((r, i) => {
				if (r.kind === 'hunk') return <Text key={i} color="cyan" wrap="truncate">{r.text}</Text>;
				if (r.kind === 'note') return <Text key={i} dimColor>{r.text}</Text>;
				const bg = r.type === 'add' ? '#1f4d2b' : r.type === 'del' ? '#5a1f26' : undefined;
				const mark = r.type === 'add' ? '+' : r.type === 'del' ? '-' : ' ';
				const fg = r.type === 'add' ? 'greenBright' : r.type === 'del' ? 'redBright' : 'gray';
				return (
					<Text key={i} wrap="truncate">
						<Text backgroundColor={bg} color="gray">{pad(r.oldNo)} {pad(r.newNo)} </Text>
						<Text backgroundColor={bg} color={fg} bold>{mark} </Text>
						{hl(r.text.replace(/\t/g, '  '), file.path)}
					</Text>
				);
			})}
		</Box>
	);
}
