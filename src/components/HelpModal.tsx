import {Box} from 'ink';
import {ModalText as Text} from './ModalText.js';
import {KEYS} from '../keys.js';
import {useTheme} from '../theme.js';

export const helpHeight = KEYS.length + new Set(KEYS.map((k) => k.g)).size + 4;

export function HelpModal({width, height}: {width: number; height: number}) {
	const t = useTheme();
	const groups = [...new Set(KEYS.map((k) => k.g))];
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
			{groups.map((g) => (
				<Box key={g} flexDirection="column">
					<Text color={t.accent}> {g}</Text>
					{KEYS.filter((k) => k.g === g).map((k) => (
						<Text key={k.k} wrap="truncate">
							{'   '}
							<Text color={t.mode}>{k.k.padEnd(11)}</Text>
							{k.d}
						</Text>
					))}
				</Box>
			))}
			<Box justifyContent="space-between">
				<Text color={t.dim}> ?/esc/q close</Text>
				<Text color={t.dim}>Made by Wouter de Wild - 2026 </Text>
			</Box>
		</Box>
	);
}
