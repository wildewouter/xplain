import {readdirSync, readFileSync} from 'node:fs';
import {homedir} from 'node:os';
import {join} from 'node:path';
import {only} from './ps.js';
import type {Agent, AgentProvider} from './types.js';

export const classifyClaude = only('claude');

const fmtUp = (ms: number) => {
	const s = Math.max(0, Math.floor(ms / 1000));
	const d = Math.floor(s / 86400);
	const p = (n: number) => String(n).padStart(2, '0');
	const t = `${p(Math.floor((s % 86400) / 3600))}:${p(Math.floor((s % 3600) / 60))}:${p(s % 60)}`;
	return d ? `${d}-${t}` : t;
};

// Parse one Claude Code session file (~/.claude/sessions/<pid>.json) into a row.
export function parseSession(
	text: string,
	isAlive: (pid: number) => boolean = () => true,
	now = Date.now(),
): Agent | undefined {
	let j: any;
	try {
		j = JSON.parse(text);
	} catch {
		return undefined;
	}
	if (!j || typeof j !== 'object') return undefined;
	const pid = j.pid;
	if (!Number.isInteger(pid) || pid <= 0 || !isAlive(pid)) return undefined;
	const sid = typeof j.sessionId === 'string' ? j.sessionId : '';
	const str = (v: unknown) => (typeof v === 'string' && v ? v : undefined);
	return {
		agent: 'claude',
		pid,
		uptime: typeof j.startedAt === 'number' ? fmtUp(now - j.startedAt) : '',
		cwd: str(j.cwd) ?? '',
		cmd: 'claude',
		name: str(j.name) ?? (sid ? sid.slice(0, 8) : undefined),
		status: str(j.status),
		kind: str(j.kind),
	};
}

const alive = (pid: number) => {
	try {
		process.kill(pid, 0);
		return true;
	} catch (e: any) {
		return e?.code === 'EPERM';
	}
};

export type ClaudeDeps = {dir?: string; isAlive?: (pid: number) => boolean; now?: () => number};

export function listClaudeSessions(
	dir = join(process.env.CLAUDE_CONFIG_DIR || join(homedir(), '.claude'), 'sessions'),
	isAlive = alive,
	now = Date.now(),
): Agent[] {
	const res: Agent[] = [];
	let files: string[] = [];
	try {
		files = readdirSync(dir);
	} catch {}
	for (const f of files) {
		if (!f.endsWith('.json')) continue;
		try {
			const a = parseSession(readFileSync(join(dir, f), 'utf8'), isAlive, now);
			if (a) res.push(a);
		} catch {}
	}
	return res;
}

export const claudeProvider = (deps: ClaudeDeps = {}): AgentProvider => ({
	id: 'claude',
	label: 'Claude Code',
	async list() {
		return listClaudeSessions(deps.dir, deps.isAlive, deps.now?.());
	},
});
