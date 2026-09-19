import {Box} from 'ink';
import {ModalText as Text} from './ModalText.js';
import {useTheme} from '../theme.js';

export const ASK_H = 4;
export const ASK_MAX = 5; // selected lines shown before "… +N more"

export type AskSel = {head: string; lines: string[]};
// rows the selection preview adds above the input: header + shown lines (+ overflow row)
export const askExtra = (n: number) => 1 + Math.min(n, ASK_MAX) + (n > ASK_MAX ? 1 : 0);
export const askH = (s?: AskSel) => ASK_H + (s ? askExtra(s.lines.length) : 0);

export function AskBox({text, pos, width, sel}: {text: string; pos: number; width: number; sel?: AskSel}) {
	const t = useTheme();
	const inner = Math.max(4, width - 4);
	// keep caret visible: show window of text ending near caret
	const start = Math.max(0, pos - inner + 1);
	const shown = text.slice(start, start + inner);
	const p = pos - start;
	return (
		<Box
			flexDirection="column"
			borderStyle="round"
			borderColor={t.modalBorder}
			backgroundColor={t.modalBg}
			width={width}
			height={askH(sel)}
		>
			{sel && (
				<>
					<Text wrap="truncate" color={t.accent}>
						{' '}
						{sel.head}
					</Text>
					{sel.lines.slice(0, ASK_MAX).map((l, i) => (
						<Text key={i} wrap="truncate" color={t.dim}>
							{' > '}
							{l}
						</Text>
					))}
					{sel.lines.length > ASK_MAX && (
						<Text wrap="truncate" color={t.dim}>
							{` … +${sel.lines.length - ASK_MAX} more`}
						</Text>
					)}
				</>
			)}
			<Text wrap="truncate">
				{' '}
				{shown.slice(0, p)}
				<Text inverse>{shown[p] ?? ' '}</Text>
				{shown.slice(p + 1)}
			</Text>
			<Text color={t.dim}>{' enter send  esc cancel'}</Text>
		</Box>
	);
}

// submitted question kept inline under its anchor row
export type SentQ = {id?: string; focused?: boolean; head: string; lines: string[]; message: string};
export const SENT_MAX = 3;
export const sentH = (q: SentQ) =>
	2 +
	1 +
	(q.lines.length ? Math.min(q.lines.length, SENT_MAX) + (q.lines.length > SENT_MAX ? 1 : 0) : 0) +
	1 +
	(q.focused ? 1 : 0);

const SENT_HINTS = [
	'e/enter edit  D delete  J/K prev/next  esc back',
	'e edit  D delete  esc back',
	'e edit  D delete',
	'e edit',
];
export const sentHint = (width: number) => {
	const room = width - 2 - 1; // borders + leading space
	return SENT_HINTS.find((h) => h.length <= room) ?? '';
};

export function SentBox({q, width}: {q: SentQ; width: number}) {
	const t = useTheme();
	return (
		<Box
			flexDirection="column"
			borderStyle={q.focused ? 'bold' : 'round'}
			borderColor={q.focused ? t.accent : t.dim}
			width={width}
			height={sentH(q)}
		>
			<Text wrap="truncate" color={t.accent} bold={q.focused}>
				{q.focused ? ' ▸ sent  ' : ' sent  '}
				{q.head}
			</Text>
			{q.lines.slice(0, SENT_MAX).map((l, i) => (
				<Text key={i} wrap="truncate" color={t.dim}>
					{' > '}
					{l}
				</Text>
			))}
			{q.lines.length > SENT_MAX && (
				<Text wrap="truncate" color={t.dim}>
					{` … +${q.lines.length - SENT_MAX} more`}
				</Text>
			)}
			<Text wrap="truncate"> {q.message}</Text>
			{q.focused && (
				<Text wrap="truncate" color={t.dim}>
					{' ' + sentHint(width)}
				</Text>
			)}
		</Box>
	);
}
