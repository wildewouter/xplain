import {execFile} from 'node:child_process';
import {basename, dirname} from 'node:path';
import type {Agent, AgentId, AgentProvider} from './types.js';

const NAMES: Record<string, AgentId> = {
	claude: 'claude',
	'claude-code': 'claude',
	copilot: 'copilot',
	codex: 'codex',
	opencode: 'opencode',
};
const RUNTIMES = new Set(['node', 'nodejs', 'bun', 'deno', 'tsx', 'npx']);
const strip = (s: string) => basename(s).replace(/\.(m|c)?js$/, '');
const named = (tok: string): AgentId | undefined => {
	const b = strip(tok);
	if (NAMES[b]) return NAMES[b];
	if (/^(cli|index|main)$/.test(b)) return NAMES[basename(dirname(tok))];
	if (NAMES[basename(dirname(dirname(tok)))] && /^(bin|dist)$/.test(basename(dirname(tok))))
		return NAMES[basename(dirname(dirname(tok)))];
};

// Classify one command line (argv joined by spaces).
export function classify(cmd: string): AgentId | undefined {
	const t = cmd.trim().split(/\s+/);
	if (!t[0]) return undefined;
	const exe = basename(t[0]);
	if (exe === 'gh') {
		const sub = t.slice(1).find((x) => !x.startsWith('-'));
		return sub === 'copilot' ? 'copilot' : undefined;
	}
	if (RUNTIMES.has(exe)) {
		const script = t.slice(1).find((x) => !x.startsWith('-'));
		return script ? named(script) : undefined;
	}
	return NAMES[exe];
}

export type Classifier = (cmd: string) => AgentId | undefined;

// Parse `ps -axo pid=,etime=,command=` output into agents (cwd blank).
export function parsePs(out: string, self = process.pid, classifier: Classifier = classify): Agent[] {
	const res: Agent[] = [];
	for (const line of out.split('\n')) {
		const m = /^\s*(\d+)\s+(\S+)\s+(.+)$/.exec(line);
		if (!m) continue;
		const pid = Number(m[1]);
		if (pid === self) continue;
		const agent = classifier(m[3]!);
		if (agent) res.push({agent, pid, uptime: m[2]!, cwd: '', cmd: m[3]!});
	}
	return res.sort((a, b) => a.agent.localeCompare(b.agent) || a.pid - b.pid);
}

export const run = (cmd: string, args: string[]) =>
	new Promise<string>((resolve, reject) =>
		execFile(cmd, args, {maxBuffer: 16 * 1024 * 1024}, (e, so) => (e && !so ? reject(e) : resolve(String(so)))),
	);

export const listPs = () => run('ps', ['-axo', 'pid=,etime=,command=']);

// Map pid -> cwd via lsof. Never throws.
export async function lsofCwds(pids: number[]): Promise<Map<number, string>> {
	const cwds = new Map<number, string>();
	if (!pids.length) return cwds;
	try {
		const out = await run('lsof', ['-a', '-p', pids.join(','), '-d', 'cwd', '-Fn']);
		let pid = 0;
		for (const l of out.split('\n')) {
			if (l.startsWith('p')) pid = Number(l.slice(1));
			else if (l.startsWith('n') && pid) cwds.set(pid, l.slice(1));
		}
	} catch {}
	return cwds;
}

export type PsDeps = {
	ps?: () => Promise<string>;
	lsof?: (pids: number[]) => Promise<Map<number, string>>;
	self?: number;
};

export function psProvider(id: AgentId, label: string, classifier: Classifier, deps: PsDeps = {}): AgentProvider {
	const ps = deps.ps ?? listPs;
	const lsof = deps.lsof ?? lsofCwds;
	return {
		id,
		label,
		async list() {
			const rows = parsePs(await ps(), deps.self ?? process.pid, classifier);
			const need = rows.filter((a) => !a.cwd);
			if (need.length) {
				const cwds = await lsof(need.map((a) => a.pid));
				for (const a of need) a.cwd = cwds.get(a.pid) ?? '';
			}
			return rows;
		},
	};
}

export const only =
	(id: AgentId): Classifier =>
	(cmd) =>
		classify(cmd) === id ? id : undefined;

export const isAlive = (pid: number) => {
	try {
		process.kill(pid, 0);
		return true;
	} catch (e: any) {
		return e?.code === 'EPERM';
	}
};

export const fmtUp = (ms: number) => {
	const s = Math.max(0, Math.floor(ms / 1000));
	const d = Math.floor(s / 86400);
	const p = (n: number) => String(n).padStart(2, '0');
	const t = `${p(Math.floor((s % 86400) / 3600))}:${p(Math.floor((s % 3600) / 60))}:${p(s % 60)}`;
	return d ? `${d}-${t}` : t;
};

export const BUSY_MS = 5000;
