import type {AgentId} from '../types.js';
export type AgentEvent =
	| {t: 'init'; sessionId?: string; model?: string}
	| {t: 'text'; delta: string}
	| {t: 'tool'; id: string; name: string; input?: unknown}
	| {t: 'toolResult'; id: string; ok: boolean}
	| {t: 'notice'; level: 'info' | 'warn'; msg: string}
	| {t: 'done'; text: string; sessionId?: string; costUsd?: number; isError: boolean}
	| {t: 'error'; msg: string};
export interface AskContext {
	cwd: string;
	sessionId?: string;
	mode: 'fork' | 'new';
	readOnly: boolean;
	systemAppend?: string;
}
export interface AgentClient {
	readonly id: AgentId;
	readonly supports: {resumeRunning: boolean; tokenStreaming: boolean; cancel: boolean; fork: boolean};
	send(prompt: string, ctx: AskContext): AsyncIterable<AgentEvent>;
	cancel(): Promise<void>;
}
export type ClientFactory = (id: AgentId) => AgentClient | undefined;
