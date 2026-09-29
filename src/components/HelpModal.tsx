import {Box} from 'ink';
import {ModalText as Text} from './ModalText.js';
import {CTX_LABEL, hasMotions, keysFor, type HelpCtx} from '../keys.js';
import {useTheme} from '../theme.js';

type Item = {k: string; d: string};
type Group = {g: string; items: Item[]};
const CREDIT = 'Made by Wouter de Wild - 2026';
// contexts where `?` is typed text, so it can't close the panel
const TYPING: HelpCtx[] = ['editor', 'find', 'goto', 'search'];

// word-wrap `s` into lines of at most `w` columns
const wrap = (s: string, w: number) => {
	const out: string[] = [];
	let cur = '';
	for (const word of s.split(' ')) {
		if (cur && cur.length + 1 + word.length <= w) cur += ` ${word}`;
		else {
			if (cur) out.push(cur);
			cur = word;
			while (cur.length > w) {
				out.push(cur.slice(0, w));
				cur = cur.slice(w);
			}
		}
	}
	out.push(cur);
	return out;
};
const wrapped = (s: string, w: number) => wrap(s, w).length;

export function HelpPanel({
	ctx,
	width,
	maxHeight,
	motions = false,
}: {
	ctx: HelpCtx;
	width: number;
	maxHeight: number;
	motions?: boolean;
}) {
	const t = useTheme();
	const keys = keysFor(ctx, motions);
	const groups: Group[] = [...new Set(keys.map((k) => k.g))].map((g) => ({
		g,
		items: keys.filter((k) => k.g === g).map(({k, d}) => ({k, d})),
	}));
	const close = !TYPING.includes(ctx);
	const hint = close ? (!motions && hasMotions(ctx) ? ' ? move keys' : ' ? close') : '';
	const title = ` Help · ${CTX_LABEL[ctx]}`;
	// key column: indent 2 + longest key + 1
	const pad = Math.max(0, ...keys.map((k) => k.k.length)) + 1;
	// fit to content: rows, headings, title, hint + credit; clamp to `width`
	const natural = Math.max(
		2 + pad + Math.max(0, ...keys.map((k) => k.d.length)),
		...groups.map((g) => 1 + g.g.length),
		title.length,
		(hint ? hint.length + 1 : 0) + CREDIT.length + 1,
	);
	const inner = Math.max(1, Math.min(width, natural + 2) - 2);
	const descW = Math.max(1, inner - 2 - pad);
	const heightOf = (gs: Group[]) =>
		gs.reduce((h, g) => h + 1 + g.items.reduce((s, i) => s + wrapped(i.d, descW), 0), 0);
	// border 2 + title 1 + close row
	const avail = Math.max(1, maxHeight - 3 - (close ? 1 : 0));
	const total = keys.length;
	const cut = heightOf(groups) > avail;
	const limit = cut ? Math.max(0, avail - 1) : avail;
	// truncate to the height limit (whole rows only)
	let shown = 0;
	let bodyH = 0;
	const body: Group[] = [];
	for (const g of groups) {
		if (bodyH + 2 > limit) break;
		bodyH += 1;
		const items: Item[] = [];
		for (const i of g.items) {
			const ih = wrapped(i.d, descW);
			if (bodyH + ih > limit) break;
			bodyH += ih;
			items.push(i);
		}
		if (!items.length) {
			bodyH -= 1;
			break;
		}
		shown += items.length;
		body.push({g: g.g, items});
		if (items.length < g.items.length) break;
	}
	const credit = inner >= (hint ? hint.length + 1 : 0) + CREDIT.length + 1 && (close || (!cut && bodyH + 1 <= avail));
	return (
		<Box
			flexDirection="column"
			borderStyle="round"
			borderColor={t.modalBorder}
			backgroundColor={t.modalBg}
			width={inner + 2}
		>
			<Text bold wrap="truncate">
				{title}
			</Text>
			{body.map((g, gi) => (
				<Box key={gi} flexDirection="column">
					<Text color={t.accent} wrap="truncate">
						{' '}
						{g.g}
					</Text>
					{g.items.map((r, ri) => (
						<Box key={ri} flexDirection="row">
							<Box width={2 + pad} flexShrink={0}>
								<Text color={t.mode} wrap="truncate">
									{'  '}
									{r.k}
								</Text>
							</Box>
							<Box width={descW} flexShrink={0}>
								<Text wrap="wrap">{wrap(r.d, descW).join('\n')}</Text>
							</Box>
						</Box>
					))}
				</Box>
			))}
			{cut && (
				<Text color={t.dim} wrap="truncate">
					{' '}
					… {total - shown} more
				</Text>
			)}
			{(close || credit) && (
				<Box flexDirection="row" justifyContent={hint ? 'space-between' : 'flex-end'}>
					{close && <Text color={t.dim}>{hint}</Text>}
					{credit && <Text color={t.dim}>{CREDIT} </Text>}
				</Box>
			)}
		</Box>
	);
}
