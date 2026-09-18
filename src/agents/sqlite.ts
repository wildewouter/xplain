import {execFile} from 'node:child_process';

export type Row = Record<string, unknown>;
export type Param = string | number | null;
export type Backend = (dbPath: string, sql: string, params: Param[]) => Promise<Row[]>;

// node:sqlite (Node >= 22.5). Experimental warning is filtered so it cannot leak into the TUI.
export const nodeSqliteBackend: Backend = async (dbPath, sql, params) => {
	const orig = process.emitWarning;
	process.emitWarning = ((w: any, ...a: any[]) => {
		const msg = typeof w === 'string' ? w : (w?.message ?? '');
		if (/sqlite/i.test(msg)) return;
		return (orig as any).call(process, w, ...a);
	}) as typeof process.emitWarning;
	let mod: any;
	try {
		mod = await import('node:sqlite');
	} finally {
		process.emitWarning = orig;
	}
	const db = new mod.DatabaseSync(dbPath, {readOnly: true});
	try {
		db.exec('PRAGMA busy_timeout = 2000');
		return db.prepare(sql).all(...params) as Row[];
	} finally {
		db.close();
	}
};

const lit = (p: Param) => (p === null ? 'NULL' : typeof p === 'number' ? String(p) : `'${p.replace(/'/g, "''")}'`);
// Inline params for the CLI (no bind support). Only `?` placeholders; values escaped.
export const inlineParams = (sql: string, params: Param[]) => {
	let i = 0;
	return sql.replace(/\?/g, () => lit(params[i++] ?? null));
};

export const cliBackend: Backend = (dbPath, sql, params) =>
	new Promise((resolve, reject) =>
		execFile(
			'sqlite3',
			['-readonly', '-json', '-cmd', '.timeout 2000', dbPath, inlineParams(sql, params)],
			{maxBuffer: 16 * 1024 * 1024},
			(e, so) => {
				if (e) return reject(e);
				const s = String(so).trim();
				try {
					resolve(s ? JSON.parse(s) : []);
				} catch (err) {
					reject(err);
				}
			},
		),
	);

export const BACKENDS: Backend[] = [nodeSqliteBackend, cliBackend];

// Read-only query; first backend that works wins. Never throws.
export async function query(
	dbPath: string,
	sql: string,
	params: Param[] = [],
	backends: Backend[] = BACKENDS,
): Promise<Row[]> {
	for (const b of backends) {
		try {
			return await b(dbPath, sql, params);
		} catch {}
	}
	return [];
}
