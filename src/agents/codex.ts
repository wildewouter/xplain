import {closeSync, openSync, readdirSync, readSync, statSync} from 'node:fs';
import {homedir} from 'node:os';
import {join} from 'node:path';
import {BUSY_MS, only, psProvider, run, type PsDeps} from './ps.js';
import {query as sqlQuery, type Param, type Row} from './sqlite.js';
import type {Agent, AgentProvider} from './types.js';

const base = only('codex');
const SKIP = new Set(['app-server', 'exec-server', 'mcp', 'mcp-server']);
// Not app-server / exec-server / mcp helpers.
export const classifyCodex = (cmd: string) =>
	base(cmd) &&
	!cmd
		.trim()
		.split(/\s+/)
		.slice(1)
		.some((t) => SKIP.has(t))
		? 'codex'
		: undefined;

const RUNTIME = /^(node|nodejs|bun|deno|tsx|npx)$/;
const isWrapper = (cmd: string) => RUNTIME.test(cmd.trim().split(/\s+/)[0]!.replace(/^.*\//, ''));

// Drop node wrapper rows when a native child with same cwd exists (pairwise).
export function dedupeWrappers(rows: Agent[]): Agent[] {
	const drop = new Set<number>();
	const byCwd = new Map<string, Agent[]>();
	for (const a of rows) byCwd.set(a.cwd, [...(byCwd.get(a.cwd) ?? []), a]);
	for (const [cwd, g] of byCwd) {
		if (!cwd) continue;
		const w = g.filter((a) => isWrapper(a.cmd));
		const n = g.length - w.length;
		for (const a of w.slice(0, n)) drop.add(a.pid);
	}
	return rows.filter((a) => !drop.has(a.pid));
}

const UUID = /([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})\.jsonl$/i;
export const rolloutId = (path: string) => UUID.exec(path)?.[1];

// Parse `lsof -p PID -Fn` output: first open rollout-*.jsonl path.
export function parseRollout(out: string): string | undefined {
	for (const l of out.split('\n'))
		if (l.startsWith('n') && /\/rollout-[^/]*\.jsonl$/.test(l) && rolloutId(l)) return l.slice(1);
}

// Status from rollout tail: last task_started w/o later task_complete = busy.
export function statusFromTail(tail: string): 'busy' | 'idle' {
	let s: 'busy' | 'idle' = 'idle';
	for (const l of tail.split('\n')) {
		if (l.includes('"task_started"')) s = 'busy';
		else if (l.includes('"task_complete"')) s = 'idle';
	}
	return s;
}

const highest = (dir: string, prefix: string) => {
	try {
		const re = new RegExp(`^${prefix}_(\\d+)\\.sqlite$`);
		const f = readdirSync(dir)
			.map((n) => ({n, v: Number(re.exec(n)?.[1] ?? -1)}))
			.filter((x) => x.v >= 0)
			.sort((a, b) => b.v - a.v)[0];
		return f ? join(dir, f.n) : undefined;
	} catch {}
};

const readTail = (path: string, n = 65536) => {
	const fd = openSync(path, 'r');
	try {
		const size = statSync(path).size;
		const len = Math.min(n, size);
		const b = Buffer.alloc(len);
		readSync(fd, b, 0, len, size - len);
		return b.toString('utf8');
	} finally {
		closeSync(fd);
	}
};

export type CodexDeps = PsDeps & {
	home?: string;
	openRollout?: (pid: number) => Promise<string | undefined>;
	query?: (db: string, sql: string, params: Param[]) => Promise<Row[]>;
	readTail?: (path: string) => string;
	mtime?: (path: string) => number;
	now?: () => number;
};

const T_COLS = 'id, title, first_user_message, git_branch, model, rollout_path';

export const codexProvider = (deps: CodexDeps = {}): AgentProvider => {
	const ps = psProvider('codex', 'Codex', classifyCodex, deps);
	return {
		id: 'codex',
		label: 'Codex',
		async list() {
			const rows = dedupeWrappers(await ps.list());
			const home = deps.home ?? process.env.CODEX_HOME ?? join(homedir(), '.codex');
			const q = deps.query ?? sqlQuery;
			const tail = deps.readTail ?? readTail;
			const mtime = deps.mtime ?? ((p: string) => statSync(p).mtimeMs);
			const now = deps.now?.() ?? Date.now();
			const open =
				deps.openRollout ??
				(async (pid: number) => {
					try {
						return parseRollout(await run('lsof', ['-p', String(pid), '-Fn']));
					} catch {}
				});
			const logs = highest(home, 'logs');
			const state = highest(home, 'state');
			const used = new Set<string>();
			for (const a of rows) {
				try {
					let id: string | undefined;
					let path: string | undefined;
					try {
						path = await open(a.pid);
						id = path && rolloutId(path);
					} catch {}
					if (!id && logs) {
						try {
							const r = await q(
								logs,
								'select thread_id from logs where process_uuid like ? and thread_id is not null order by id desc limit 1',
								[`pid:${a.pid}:%`],
							);
							id = r[0]?.thread_id ? String(r[0].thread_id) : undefined;
						} catch {}
					}
					if (!id && state && a.cwd) {
						try {
							const r = await q(
								state,
								"select id from threads where cwd = ? and archived = 0 and thread_source = 'user' order by updated_at desc",
								[a.cwd],
							);
							id = r.map((x) => String(x.id)).find((x) => !used.has(x));
						} catch {}
					}
					if (!id) continue;
					used.add(id);
					if (state) {
						try {
							const t = (await q(state, `select ${T_COLS} from threads where id = ?`, [id]))[0];
							if (t) {
								a.name = String(t.title || t.first_user_message || '').slice(0, 80) || undefined;
								a.kind = t.model ? String(t.model) : undefined;
								path ??= t.rollout_path ? String(t.rollout_path) : undefined;
							}
						} catch {}
					}
					a.name ??= id.slice(0, 8);
					if (path) {
						try {
							a.status = now - mtime(path) < BUSY_MS ? 'busy' : statusFromTail(tail(path));
						} catch {}
					}
				} catch {}
			}
			return rows;
		},
	};
};
