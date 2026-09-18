import {Box} from 'ink';
import {ModalText as Text} from './ModalText.js';
import type {Agent} from '../agents/index.js';
import {useTheme} from '../theme.js';

const cell = (s: string | undefined, n: number) => (s ?? '').slice(0, n).padEnd(n);

export function AgentsModal({
	agents,
	sel,
	height,
	width,
}: {
	agents: Agent[] | null;
	sel: number;
	height: number;
	width: number;
}) {
	const t = useTheme();
	const list = agents ?? [];
	const vis = Math.max(1, height - 4);
	const start = Math.min(Math.max(0, sel - Math.floor(vis / 2)), Math.max(0, list.length - vis));
	return (
		<Box
			flexDirection="column"
			borderStyle="round"
			borderColor={t.modalBorder}
			backgroundColor={t.modalBg}
			width={width}
			height={height}
		>
			<Text bold> Agents ({agents ? list.length : '...'})</Text>
			<Box flexDirection="column" height={vis}>
				{agents === null ? (
					<Text color={t.dim}> loading...</Text>
				) : list.length === 0 ? (
					<Text color={t.dim}> no running agents found</Text>
				) : (
					list.slice(start, start + vis).map((a, k) => {
						const i = start + k;
						return (
							<Text
								key={a.pid}
								wrap="truncate"
								backgroundColor={i === sel ? t.selBg : undefined}
								color={i === sel ? t.selFg : undefined}
							>
								{i === sel ? '>' : ' '} {a.agent.padEnd(9)} {cell(a.name, 20)} {cell(a.status, 5)} {cell(a.kind, 11)}{' '}
								{String(a.pid).padEnd(7)} {a.uptime.padEnd(11)} {a.cwd}
							</Text>
						);
					})
				)}
			</Box>
			<Text color={t.dim}> j/k/↑↓ move r refresh esc/q/A close</Text>
		</Box>
	);
}
