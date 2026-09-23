import {Box} from 'ink';
import {ModalText as Text} from './ModalText.js';
import {useTheme} from '../theme.js';
import type {BodyLine} from './answerView.js';
import {Spinner} from '../spinner.js';

export const ASK_H = 4;
export const ASK_MAX = 5; // selected lines shown before "… +N more"

export type AskSel = {head: string; lines: string[]};
// rows the selection preview adds above the input: header + shown lines (+ overflow row)
export const askExtra = (n: number) => 1 + Math.min(n, ASK_MAX) + (n > ASK_MAX ? 1 : 0);
export const askH = (s?: AskSel) => ASK_H + (s ? askExtra(s.lines.length) : 0);

// hint row under the input
export type AskMode = 'save' | 'ask';
export const askHint = (width: number, mode?: AskMode) => {
	const room = width - 2 - 1;
	const opts = mode
		? [
				`[${mode}] enter send  tab save/ask  esc cancel`,
				'enter send  tab save/ask  esc cancel',
				'enter send  esc cancel',
				'enter send',
			]
		: ['enter send  esc cancel', 'enter send'];
	return opts.find((h) => h.length <= room) ?? opts[opts.length - 1]!.slice(0, Math.max(0, room));
};

export function AskBox({
	text,
	pos,
	width,
	sel,
	mode,
}: {
	text: string;
	pos: number;
	width: number;
	sel?: AskSel;
	mode?: AskMode;
}) {
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
			<Text color={t.dim} wrap="truncate">
				{' ' + askHint(width, mode)}
			</Text>
		</Box>
	);
}

// submitted question kept inline under its anchor row
export type SentQ = {
	id?: string;
	focused?: boolean;
	saved?: boolean; // saved only, not asked
	canAsk?: boolean; // a asks the agent
	canFollow?: boolean; // a opens the follow-up input
	head: string;
	lines: string[];
	body: BodyLine[]; // thread lines shown (already windowed)
	more?: number; // unfocused: hidden lines
	pos?: string; // focused + overflowing: "12-24/58"
	scroll?: boolean; // focused thread scrolls (j/k)
};
export const SENT_MAX = 3;
// rows of a sent box without its body lines
export const sentBase = (q: Pick<SentQ, 'lines' | 'focused'>) =>
	2 +
	1 +
	(q.lines.length ? Math.min(q.lines.length, SENT_MAX) + (q.lines.length > SENT_MAX ? 1 : 0) : 0) +
	(q.focused ? 1 : 0);
export const sentH = (q: SentQ) => sentBase(q) + q.body.length + (q.more ? 1 : 0);

export const sentHint = (width: number, q: Pick<SentQ, 'canAsk' | 'canFollow' | 'scroll'>) => {
	const room = width - 2 - 1; // borders + leading space
	const a = q.canFollow ? '  a follow up' : q.canAsk ? '  a ask' : '';
	const sc = q.scroll ? '  j/k scroll' : '';
	const opts = [
		`e edit  D delete${a}${sc}  esc back`,
		`e edit  D delete${a}  esc back`,
		`e edit  D delete${a}`,
		'e edit  D delete',
		'e edit',
	];
	return opts.find((h) => h.length <= room) ?? '';
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
				{q.saved && <Text color={t.dim}>{'  saved · not asked'}</Text>}
				{q.pos && <Text color={t.dim}>{'  ↕ ' + q.pos}</Text>}
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
			{q.body.map((l, i) =>
				l.k === 'div' ? (
					<Text key={i} wrap="truncate" color={l.err ? t.dels : t.accent}>
						{('─ ' + l.t + ' ').padEnd(Math.max(1, width - 2), '─')}
					</Text>
				) : (
					<Text key={i} wrap="truncate" color={l.err ? t.dels : l.k === 'fu' ? t.accent : undefined}>
						{' '}
						{l.live && <>{l.live === 'streaming' ? <Spinner color={t.accent} /> : <Text color={t.accent}>⠿</Text>} </>}
						{l.t}
					</Text>
				),
			)}
			{!!q.more && (
				<Text wrap="truncate" color={t.dim}>
					{` … +${q.more} more`}
				</Text>
			)}
			{q.focused && (
				<Text wrap="truncate" color={t.dim}>
					{' ' + sentHint(width, q)}
				</Text>
			)}
		</Box>
	);
}
