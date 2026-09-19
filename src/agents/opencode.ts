import {homedir} from 'node:os';
import {join} from 'node:path';
import {BUSY_MS, only, psProvider, type PsDeps} from './ps.js';
import {query as sqlQuery, type Param, type Row} from './sqlite.js';
import type {AgentProvider} from './types.js';

const base = only('opencode');
// Not `opencode serve` / other non-TUI subcommands.
export const classifyOpenCode = (cmd: string) =>
	base(cmd) && !cmd.trim().split(/\s+/).slice(1).includes('serve') ? 'opencode' : undefined;

export type OpenCodeDeps = PsDeps & {
	db?: string;
	query?: (db: string, sql: string, params: Param[]) => Promise<Row[]>;
	now?: () => number;
};

const SQL =
	'select id, title, time_updated from session where directory = ? and parent_id is null and time_archived is null order by time_updated desc';

export const opencodeProvider = (deps: OpenCodeDeps = {}): AgentProvider => {
	const ps = psProvider('opencode', 'OpenCode', classifyOpenCode, deps);
	return {
		id: 'opencode',
		label: 'OpenCode',
		async list() {
			const rows = await ps.list();
			const db =
				deps.db ?? join(process.env.XDG_DATA_HOME || join(homedir(), '.local', 'share'), 'opencode', 'opencode.db');
			const q = deps.query ?? sqlQuery;
			const now = deps.now?.() ?? Date.now();
			const seen = new Map<string, number>();
			const cache = new Map<string, Row[]>();
			for (const a of rows) {
				if (!a.cwd) continue;
				try {
					let ss = cache.get(a.cwd);
					if (!ss) cache.set(a.cwd, (ss = await q(db, SQL, [a.cwd])));
					const n = seen.get(a.cwd) ?? 0;
					seen.set(a.cwd, n + 1);
					const s = ss[n];
					if (!s) continue;
					a.sessionId = s.id ? String(s.id) : undefined;
					a.name = String(s.title ?? '') || undefined;
					a.status = now - Number(s.time_updated) < BUSY_MS ? 'busy' : 'idle';
				} catch {}
			}
			return rows;
		},
	};
};
