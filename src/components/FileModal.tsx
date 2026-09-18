import {Box, Text} from 'ink';
import type {DiffFile} from '../diff/load.js';

export const status = (f: DiffFile) =>
	f.from ? 'R' : f.hunks.length && f.hunks.every(h => /^@@ -0,0 /.test(h.header)) ? 'A' : f.hunks.length && f.hunks.every(h => / \+0,0 @@/.test(h.header)) ? 'D' : 'M';
const color = {A: 'green', D: 'red', R: 'yellow', M: 'cyan'} as const;

export function FileModal({files, sel, current, height, width}: {files: DiffFile[]; sel: number; current: number; height: number; width: number}) {
	const vis = Math.max(1, height - 4); // border 2 + title + hint
	const start = Math.min(Math.max(0, sel - Math.floor(vis / 2)), Math.max(0, files.length - vis));
	return (
		<Box flexDirection="column" borderStyle="round" backgroundColor="black" width={width} height={height}>
			<Text bold> Files ({sel + 1}/{files.length})</Text>
			<Box flexDirection="column" height={vis}>
				{files.slice(start, start + vis).map((f, k) => {
					const i = start + k;
					const s = status(f);
					return (
						<Text key={i} wrap="truncate" inverse={i === sel}>
							{i === sel ? '>' : ' '}<Text color={color[s]}>{s}</Text> {f.from ? `${f.from} -> ` : ''}{f.path}
							<Text color="green"> +{f.adds}</Text>
							<Text color="red"> -{f.dels}</Text>
							{i === current ? ' *' : ''}
						</Text>
					);
				})}
			</Box>
			<Text dimColor> j/k/↑↓ move  ctrl-d/u half page  enter open  esc/q close</Text>
		</Box>
	);
}
