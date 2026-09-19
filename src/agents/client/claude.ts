import {spawn as nodeSpawn} from 'node:child_process';
import type {ChildProcess, SpawnOptions} from 'node:child_process';
import type {AgentClient, AgentEvent, AskContext} from './types.js';

export type SpawnFn = (cmd: string, args: string[], opts: SpawnOptions) => ChildProcess;
export interface ClaudeDeps {
	spawn?: SpawnFn;
	bin?: string;
	env?: NodeJS.ProcessEnv;
}
export interface ParseState {
	sawDelta: boolean; // text deltas streamed for current assistant message
}
export const newState = (): ParseState => ({sawDelta: false});

const isObj = (x: unknown): x is Record<string, any> => typeof x === 'object' && x !== null;

// Map one stream-json NDJSON line to zero or more events. Never throws.
export function parseLine(line: string, state: ParseState): AgentEvent[] {
	const s = line.trim();
	if (!s) return [];
	let o: unknown;
	try {
		o = JSON.parse(s);
	} catch {
		return [{t: 'notice', level: 'warn', msg: 'malformed line skipped'}];
	}
	if (!isObj(o)) return [];
	const sub = o.parent_tool_use_id != null;
	switch (o.type) {
		case 'system':
			if (o.subtype === 'init') return [{t: 'init', sessionId: o.session_id, model: o.model}];
			if (o.subtype === 'api_retry' || o.subtype === 'permission_denied') {
				const why = o.error ?? o.message ?? o.tool_name ?? '';
				return [{t: 'notice', level: 'warn', msg: `${o.subtype}${why ? ': ' + String(why) : ''}`}];
			}
			return [];
		case 'stream_event': {
			const e = o.event;
			if (sub || !isObj(e)) return [];
			if (e.type === 'message_start') state.sawDelta = false;
			if (
				e.type === 'content_block_delta' &&
				isObj(e.delta) &&
				e.delta.type === 'text_delta' &&
				typeof e.delta.text === 'string'
			) {
				state.sawDelta = true;
				return [{t: 'text', delta: e.delta.text}];
			}
			return [];
		}
		case 'assistant': {
			if (sub) return [];
			const content = o.message?.content;
			if (!Array.isArray(content)) return [];
			const out: AgentEvent[] = [];
			for (const b of content) {
				if (!isObj(b)) continue;
				if (b.type === 'tool_use') out.push({t: 'tool', id: String(b.id), name: String(b.name), input: b.input});
				else if (b.type === 'text' && !state.sawDelta && typeof b.text === 'string')
					out.push({t: 'text', delta: b.text});
			}
			state.sawDelta = false;
			return out;
		}
		case 'user': {
			if (sub) return [];
			const content = o.message?.content;
			if (!Array.isArray(content)) return [];
			const out: AgentEvent[] = [];
			for (const b of content)
				if (isObj(b) && b.type === 'tool_result')
					out.push({t: 'toolResult', id: String(b.tool_use_id), ok: !b.is_error});
			return out;
		}
		case 'result':
			return [
				{
					t: 'done',
					text: typeof o.result === 'string' ? o.result : '',
					sessionId: o.session_id,
					costUsd: typeof o.total_cost_usd === 'number' ? o.total_cost_usd : undefined,
					isError: Boolean(o.is_error) || (typeof o.subtype === 'string' && o.subtype.startsWith('error')),
				},
			];
		default:
			return [];
	}
}

export function buildArgs(prompt: string, ctx: AskContext): string[] {
	const a = ['-p', prompt, '--output-format', 'stream-json', '--verbose', '--include-partial-messages'];
	if (ctx.mode === 'fork' && ctx.sessionId) a.push('--resume', ctx.sessionId, '--fork-session');
	if (ctx.readOnly)
		a.push('--permission-mode', 'plan', '--allowedTools', 'Read,Grep,Glob', '--permission-prompts', 'none');
	if (ctx.systemAppend) a.push('--append-system-prompt', ctx.systemAppend);
	return a;
}

export function claudeClient(deps: ClaudeDeps = {}): AgentClient {
	const spawn: SpawnFn = deps.spawn ?? ((c, a, o) => nodeSpawn(c, a, o));
	const bin = deps.bin ?? 'claude';
	let child: ChildProcess | undefined;
	let cancelled = false;
	return {
		id: 'claude',
		supports: {resumeRunning: false, tokenStreaming: true, cancel: true, fork: true},
		async *send(prompt, ctx) {
			if (child) {
				yield {t: 'error', msg: 'request already in flight'};
				return;
			}
			cancelled = false;
			let cp: ChildProcess;
			try {
				cp = spawn(bin, buildArgs(prompt, ctx), {
					cwd: ctx.cwd,
					env: deps.env ?? process.env,
					stdio: ['ignore', 'pipe', 'pipe'],
				});
			} catch (e) {
				yield {t: 'error', msg: (e as NodeJS.ErrnoException).code === 'ENOENT' ? 'claude not found' : String(e)};
				return;
			}
			child = cp;
			// queue + wakeup bridge from callbacks to generator
			const q: AgentEvent[] = [];
			let ended = false;
			let wake: (() => void) | undefined;
			const push = (...e: AgentEvent[]) => {
				q.push(...e);
				wake?.();
			};
			let stderr = '';
			let sawDone = false;
			let buf = '';
			const st = newState();
			const feed = (l: string) => {
				for (const ev of parseLine(l, st)) {
					if (ev.t === 'done') sawDone = true;
					push(ev);
				}
			};
			cp.stdout?.on('data', (d: Buffer | string) => {
				buf += String(d);
				let i;
				while ((i = buf.indexOf('\n')) >= 0) {
					feed(buf.slice(0, i));
					buf = buf.slice(i + 1);
				}
			});
			cp.stderr?.on('data', (d: Buffer | string) => {
				stderr = (stderr + String(d)).slice(-2000);
			});
			let finished = false;
			const finish = (ev?: AgentEvent) => {
				if (finished) return;
				finished = true;
				if (buf.trim()) feed(buf);
				buf = '';
				if (ev && !sawDone) push(ev);
				ended = true;
				wake?.();
			};
			cp.on('error', (e: NodeJS.ErrnoException) =>
				finish({t: 'error', msg: e.code === 'ENOENT' ? 'claude not found' : e.message}),
			);
			cp.on('close', (code, sig) => {
				if (cancelled) return finish({t: 'error', msg: 'cancelled'});
				if (code === 0) return finish({t: 'error', msg: 'claude exited without result'});
				const tail = stderr.trim().split('\n').slice(-5).join('\n');
				finish({t: 'error', msg: `claude exited ${code ?? sig}${tail ? ': ' + tail : ''}`});
			});
			try {
				for (;;) {
					while (q.length) {
						const ev = q.shift()!;
						yield ev;
						if (ev.t === 'done') return;
					}
					if (ended) return;
					await new Promise<void>((r) => (wake = r));
					wake = undefined;
				}
			} finally {
				if (child === cp) child = undefined;
				if (cp.exitCode === null && cp.signalCode === null) cp.kill('SIGTERM');
			}
		},
		async cancel() {
			const cp = child;
			if (!cp) return;
			cancelled = true;
			cp.kill('SIGINT');
			const t = setTimeout(() => cp.kill('SIGTERM'), 3000);
			t.unref();
			cp.once('close', () => clearTimeout(t));
		},
	};
}
