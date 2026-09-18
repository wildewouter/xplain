import {Box} from 'ink';
import {ModalText as Text} from './ModalText.js';
import {useTheme} from '../theme.js';

export type Hit = {path: string; idx: readonly number[]};

export function SearchModal({
	query,
	hits,
	sel,
	height,
	width,
}: {
	query: string;
	hits: Hit[];
	sel: number;
	height: number;
	width: number;
}) {
	const t = useTheme();
	const vis = Math.max(1, height - 5); // border 2 + title + query + hint
	const start = Math.min(Math.max(0, sel - Math.floor(vis / 2)), Math.max(0, hits.length - vis));
	return (
		<Box
			flexDirection="column"
			borderStyle="round"
			borderColor={t.modalBorder}
			backgroundColor={t.modalBg}
			width={width}
			height={height}
		>
			<Text bold>
				{' '}
				Search ({hits.length ? sel + 1 : 0}/{hits.length})
			</Text>
			<Text wrap="truncate">
				{' '}
				<Text color={t.accent}>&gt; </Text>
				{query}
				<Text inverse> </Text>
			</Text>
			<Box flexDirection="column" height={vis}>
				{hits.slice(start, start + vis).map((h, k) => {
					const i = start + k;
					const on = i === sel;
					const set = new Set(h.idx);
					return (
						<Text key={i} wrap="truncate" backgroundColor={on ? t.selBg : undefined} color={on ? t.selFg : undefined}>
							{on ? '>' : ' '}{' '}
							{[...h.path].map((c, j) => (
								<Text
									key={j}
									backgroundColor={on ? t.selBg : undefined}
									color={on ? t.selFg : set.has(j) ? t.accent : t.modalFg}
									bold={set.has(j)}
								>
									{c}
								</Text>
							))}
						</Text>
					);
				})}
			</Box>
			<Text color={t.dim}> ↑↓/^n^p move enter open esc close</Text>
		</Box>
	);
}
