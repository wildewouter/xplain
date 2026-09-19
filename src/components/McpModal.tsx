import {Box} from 'ink';
import {ModalText as Text} from './ModalText.js';
import {useTheme} from '../theme.js';
import {wrapText} from './answerView.js';
import type {BridgeState, BridgeIntegration} from '../mcp/bridge.js';

export type McpConfirm = {kind: 'register' | 'unregister'; id: string};
export type McpPreview = {what: string; text: string};

export const MCP_CLIENTS_MAX = 2;
export const MCP_PREVIEW_MAX = 3;

export const intStatus = (i: BridgeIntegration) =>
	!i.canRegister ? 'copy-paste only' : i.registered ? (i.stale ? 'stale' : 'registered') : 'not registered';

export function McpModal({
	state,
	sel,
	confirm,
	preview,
	note,
	width,
}: {
	state: BridgeState;
	sel: number; // 0 = power row, 1.. = integrations
	confirm?: McpConfirm;
	preview?: McpPreview;
	note?: string;
	width: number;
}) {
	const t = useTheme();
	const inner = Math.max(10, width - 4);
	const cur = sel > 0 ? state.integrations[sel - 1] : undefined;
	const conf = confirm ? state.integrations.find((i) => i.id === confirm.id) : undefined;
	const shownNote = note ?? cur?.note;
	const row = (on: boolean) => ({
		backgroundColor: on ? t.selBg : undefined,
		color: on ? t.selFg : undefined,
	});
	const pv = preview ? wrapText(preview.text, inner) : [];
	const pvLines = pv.slice(0, MCP_PREVIEW_MAX);
	if (pv.length > MCP_PREVIEW_MAX)
		pvLines[MCP_PREVIEW_MAX - 1] = pvLines[MCP_PREVIEW_MAX - 1]!.slice(0, inner - 1) + '…';
	return (
		<Box
			flexDirection="column"
			borderStyle="round"
			borderColor={t.modalBorder}
			backgroundColor={t.modalBg}
			width={width}
		>
			<Text bold> MCP</Text>
			<Text wrap="truncate" {...row(sel === 0)}>
				{sel === 0 ? '>' : ' '} {state.starting ? '… starting' : state.running ? '● on ' : '○ off'}
				{state.running ? ` ${(state.url ?? '').replace(/^https?:\/\//, '').replace(/\/.*$/, '')}` : ''}
			</Text>
			{state.error && (
				<Text wrap="truncate" color={t.dels}>
					{' '}
					{state.error}
				</Text>
			)}
			<Text wrap="truncate" color={t.dim}>
				{' '}
				clients {state.clients.length} pending {state.pending} delivered {state.delivered}
			</Text>
			{state.clients.slice(0, MCP_CLIENTS_MAX).map((c) => (
				<Text key={c.id} wrap="truncate">
					{'   '}
					{c.name}
					{c.version ? ` ${c.version}` : ''}
					{c.polling ? ' ⟳' : ''}
				</Text>
			))}
			{state.clients.length > MCP_CLIENTS_MAX && (
				<Text wrap="truncate" color={t.dim}>
					{`   … +${state.clients.length - MCP_CLIENTS_MAX} more`}
				</Text>
			)}
			<Text color={t.accent}> INTEGRATIONS</Text>
			{state.integrations.map((i, k) => (
				<Text key={i.id} wrap="truncate" {...row(sel === k + 1)}>
					{sel === k + 1 ? '>' : ' '} {i.label} {i.busy ? '… ' : ''}
					{intStatus(i)}
					{i.canRegister && i.needsRestart ? '  restart needed' : ''}
				</Text>
			))}
			{confirm && conf && (
				<>
					<Text wrap="truncate" color={t.accent}>
						{' '}
						{confirm.kind === 'register'
							? `Register ${conf.label} MCP server?`
							: `Remove ${conf.label} registration?`}{' '}
						(y/n)
					</Text>
					<Text wrap="truncate" color={t.dim}>
						{' '}
						{confirm.kind === 'register' ? 'adds' : 'removes'} the xplain MCP server{' '}
						{confirm.kind === 'register' ? 'to' : 'from'} {conf.label} config
					</Text>
				</>
			)}
			{shownNote &&
				!confirm &&
				wrapText(shownNote, inner)
					.slice(0, 2)
					.map((l, i) => (
						<Text key={i} wrap="truncate" color={t.accent}>
							{' '}
							{l}
						</Text>
					))}
			{preview && !confirm && (
				<>
					<Text wrap="truncate" color={t.dim}>
						{' '}
						{preview.what}:
					</Text>
					{pvLines.map((l, i) => (
						<Text key={i} wrap="truncate">
							{' '}
							{l}
						</Text>
					))}
				</>
			)}
			<Text color={t.dim} wrap="truncate">
				{confirm ? ' y confirm  n/esc cancel' : ' j/k  enter register  d remove  c/w copy  R refresh  esc'}
			</Text>
		</Box>
	);
}
