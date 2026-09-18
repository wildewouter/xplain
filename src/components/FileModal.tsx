import {Box} from 'ink';
import {ModalText as Text} from './ModalText.js';
import type {DiffFile} from '../diff/load.js';
import {useTheme} from '../theme.js';

export const status = (f: DiffFile) =>
	f.from
		? 'R'
		: f.hunks.length && f.hunks.every((h) => /^@@ -0,0 /.test(h.header))
			? 'A'
			: f.hunks.length && f.hunks.every((h) => / \+0,0 @@/.test(h.header))
				? 'D'
				: 'M';

export function FileModal({
	files,
	sel,
	current,
	height,
	width,
}: {
	files: DiffFile[];
	sel: number;
	current: number;
	height: number;
	width: number;
}) {
	const t = useTheme();
	const color = {A: t.adds, D: t.dels, R: t.mode, M: t.accent};
	const vis = Math.max(1, height - 4); // border 2 + title + hint
	const start = Math.min(Math.max(0, sel - Math.floor(vis / 2)), Math.max(0, files.length - vis));
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
				Files ({sel + 1}/{files.length})
			</Text>
			<Box flexDirection="column" height={vis}>
				{files.slice(start, start + vis).map((f, k) => {
					const i = start + k;
					const s = status(f);
					return (
						<Text
							key={i}
							wrap="truncate"
							backgroundColor={i === sel ? t.selBg : undefined}
							color={i === sel ? t.selFg : undefined}
						>
							{i === sel ? '>' : ' '}
							<Text backgroundColor={i === sel ? t.selBg : undefined} color={i === sel ? t.selFg : color[s]}>
								{s}
							</Text>{' '}
							{f.from ? `${f.from} -> ` : ''}
							{f.path}
							<Text backgroundColor={i === sel ? t.selBg : undefined} color={i === sel ? t.selFg : t.adds}>
								{' '}
								+{f.adds}
							</Text>
							<Text backgroundColor={i === sel ? t.selBg : undefined} color={i === sel ? t.selFg : t.dels}>
								{' '}
								-{f.dels}
							</Text>
							{i === current ? ' *' : ''}
						</Text>
					);
				})}
			</Box>
			<Text color={t.dim}> j/k/↑↓ move d/u half page enter open esc/q close</Text>
		</Box>
	);
}
