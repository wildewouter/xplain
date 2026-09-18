import {readdirSync, readFileSync, statSync} from 'node:fs';
import {homedir} from 'node:os';
import {join} from 'node:path';
import {BUSY_MS, fmtUp, isAlive as alive, only, psProvider, run, type PsDeps} from './ps.js';
import type {Agent, AgentProvider} from './types.js';

export const classifyCopilot = only('copilot');

const KEYS = ['id', 'cwd', 'name', 'branch', 'repository', 'client_name', 'created_at', 'updated_at'];

// Simple `key: value` scan of workspace.yaml. No yaml dep; only known keys.
export function parseWorkspace(text: string): Record<string, string> {
	const o: Record<string, string> = {};
	for (const line of text.split('\n')) {
		const m = /^([a-z_]+):\s*(.*?)\s*$/.exec(line);
		if (!m || !KEYS.includes(m[1]!) || o[m[1]!] !== undefined) continue;
		o[m[1]!] = m[2]!.replace(/^(["'])(.*)\1$/, '$2');
	}
	return o;
}

const commOk = async (pid: number) => {
	try {
		const c = (await run('ps', ['-o', 'comm=', '-p', String(pid)])).trim();
		return !c || /copilot|node|gh/i.test(c);
	} catch {
		return true;
	}
};

export type CopilotDeps = PsDeps & {
	dir?: string;
	isAlive?: (pid: number) => boolean;
	comm?: (pid: number) => Promise<boolean>;
	now?: () => number;
};

// Sessions from <dir>/<id>/inuse.<PID>.lock. Only inuse.*.lock in session dirs are looked at.
export async function listCopilotSessions(
	deps: CopilotDeps = {},
	upById = new Map<number, string>(),
): Promise<Agent[]> {
	const dir = deps.dir ?? join(homedir(), '.copilot', 'session-state');
	const isAlive = deps.isAlive ?? alive;
	const comm = deps.comm ?? commOk;
	const now = deps.now?.() ?? Date.now();
	const res: Agent[] = [];
	let ids: string[] = [];
	try {
		ids = readdirSync(dir);
	} catch {}
	for (const id of ids) {
		try {
			const sd = join(dir, id);
			const lock = readdirSync(sd).find((f) => /^inuse\.\d+\.lock$/.test(f));
			if (!lock) continue;
			const pid = Number(/\d+/.exec(lock)![0]);
			if (!pid || !isAlive(pid) || !(await comm(pid))) continue;
			const w = parseWorkspace(readFileSync(join(sd, 'workspace.yaml'), 'utf8'));
			let status = 'idle';
			try {
				if (now - statSync(join(sd, 'events.jsonl')).mtimeMs < BUSY_MS) status = 'busy';
			} catch {}
			const t = Date.parse(w.created_at ?? '');
			res.push({
				agent: 'copilot',
				pid,
				uptime: upById.get(pid) ?? (Number.isNaN(t) ? '' : fmtUp(now - t)),
				cwd: w.cwd ?? '',
				cmd: 'copilot',
				name: w.name || (w.id ?? id).slice(0, 8),
				status,
				kind: w.client_name,
			});
		} catch {}
	}
	return res;
}

export const copilotProvider = (deps: CopilotDeps = {}): AgentProvider => {
	const base = psProvider('copilot', 'Copilot', classifyCopilot, deps);
	return {
		id: 'copilot',
		label: 'Copilot',
		async list() {
			let ps: Agent[] = [];
			try {
				ps = await base.list();
			} catch {}
			const sess = await listCopilotSessions(deps, new Map(ps.map((a) => [a.pid, a.uptime])));
			const have = new Set(sess.map((a) => a.pid));
			return [...sess, ...ps.filter((a) => !have.has(a.pid))];
		},
	};
};
