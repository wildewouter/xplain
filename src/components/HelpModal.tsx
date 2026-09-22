import {Box} from 'ink';
import {ModalText as Text} from './ModalText.js';
import {KEYS} from '../keys.js';
import {useTheme} from '../theme.js';

type Row = {head: string} | {k: string; d: string};
// flat list: heading row before each group's key rows
export const helpRows: Row[] = [...new Set(KEYS.map((k) => k.g))].flatMap((g) => [
	{head: g},
	...KEYS.filter((k) => k.g === g).map(({k, d}) => ({k, d})),
]);
// border 2 + title 1 + status/close row 1
const CHROME = 4;
export const helpHeight = helpRows.length + CHROME;
export const helpView = (height: number) => Math.max(1, height - CHROME);
export const helpMax = (height: number) => Math.max(0, helpRows.length - helpView(height));

export function HelpModal({width, height, off = 0}: {width: number; height: number; off?: number}) {
	const t = useTheme();
	const view = helpView(height);
	const o = Math.min(helpMax(height), Math.max(0, off));
	const rows = helpRows.slice(o, o + view);
	const more = helpRows.length > view;
	const pos = `${o > 0 ? '▲ ' : ''}${Math.min(helpRows.length, o + view)}/${helpRows.length}${o < helpMax(height) ? ' ▼' : ''}`;
	return (
		<Box
			flexDirection="column"
			borderStyle="round"
			borderColor={t.modalBorder}
			backgroundColor={t.modalBg}
			width={width}
			height={height}
		>
			<Text bold> Help</Text>
			{rows.map((r, i) =>
				'head' in r ? (
					<Text key={`h${o + i}`} color={t.accent} wrap="truncate">
						{' '}
						{r.head}
					</Text>
				) : (
					<Text key={`k${o + i}`} wrap="truncate">
						{'   '}
						<Text color={t.mode}>{r.k.padEnd(13)}</Text>
						{r.d}
					</Text>
				),
			)}
			<Box justifyContent="space-between">
				<Text color={t.dim}> ?/esc/q close{more ? `  ${pos}` : ''}</Text>
				<Text color={t.dim}>Made by Wouter de Wild - 2026 </Text>
			</Box>
		</Box>
	);
}
