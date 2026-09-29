// Runs one scenario in an isolated temp dir. Every wait is event-driven; the Guard only reports hangs.
import {execFile, execFileSync, spawn} from 'node:child_process';
import {
	chmodSync,
	copyFileSync,
	existsSync,
	lstatSync,
	mkdirSync,
	mkdtempSync,
	readdirSync,
	readFileSync,
	realpathSync,
	rmSync,
	statSync,
	symlinkSync,
	writeFileSync,
} from 'node:fs';
import {request, type ClientRequest} from 'node:http';
import {connect, createServer, type Server, type Socket} from 'node:net';
import {tmpdir} from 'node:os';
import {basename, dirname, isAbsolute, join, relative} from 'node:path';
import {matchText, pick, subset, type TextMatch} from './match.js';
import {splitArgs, stepKind, type Scenario, type ShimRule, type Step} from './scenario.js';
import {ExitError, Guard, HangError, Session, type BarrierKind} from './session.js';

export const ROOT = join(import.meta.dirname, '..', '..');
const E2E = join(ROOT, 'e2e');
const q = (s: string) => `'${s.replace(/'/g, `'\\''`)}'`;
// tsx picks tsconfig from cwd (the fixture repo), so point it at ours for the JSX settings
export const DEFAULT_BIN = `${q(join(ROOT, 'node_modules', '.bin', 'tsx'))} --tsconfig ${q(join(ROOT, 'tsconfig.json'))} ${q(join(ROOT, 'src', 'cli.tsx'))}`;

// tools the app may need, found on the host PATH once
const which = (n: string) => execFileSync('sh', ['-c', `command -v ${n}`], {encoding: 'utf8'}).trim();
const TOOLS: Record<string, string> = {node: process.execPath, git: which('git')};
const realBin = (name: string) =>
	TOOLS[name] ?? ['/usr/bin', '/bin'].map((d) => join(d, name)).find((p) => existsSync(p));

export type Result = {ok: boolean; ms: number; step?: number; stepText?: string; error?: string; dump?: string};

const listenOn = (port: number) =>
	new Promise<Server>((resolve, reject) => {
		const s = createServer((c) => c.destroy());
		s.once('error', reject);
		s.listen(port, '127.0.0.1', () => resolve(s));
	});
// a port the OS reports free, never handed to two scenarios of this run
const used = new Set<number>();
const freePort = async (): Promise<number> => {
	const s = await listenOn(0);
	const p = (s.address() as {port: number}).port;
	await new Promise((r) => s.close(r));
	if (used.has(p)) return freePort();
	used.add(p);
	return p;
};

class StepError extends Error {}
const fail = (m: string): never => {
	throw new StepError(m);
};

type Http = {status: number; headers: Record<string, unknown>; text: string; body?: unknown; result?: unknown};
type Req = {p: Promise<Http>; rq: ClientRequest; settle: () => void};
const TM_KEYS = ['equals', 'contains', 'notContains', 'matches'];
const isTextMatch = (x: unknown) =>
	typeof x === 'string' ||
	(!!x && typeof x === 'object' && !Array.isArray(x) && Object.keys(x).every((k) => TM_KEYS.includes(k)));
const octal = (m: unknown) => parseInt(String(m), 8);

/** Minimal glob over an absolute pattern: `*`, `?`, `[...]` within a segment, `**` = any depth. Sorted. */
function glob(pattern: string): string[] {
	const segRe = (seg: string) =>
		new RegExp(
			'^' +
				seg
					.replace(/[.+^${}()|\\]/g, '\\$&')
					.replace(/\[!/g, '[^')
					.replace(/\*/g, '[^/]*')
					.replace(/\?/g, '[^/]') +
				'$',
		);
	const ls = (d: string) => {
		try {
			return readdirSync(d, {withFileTypes: true});
		} catch {
			return [];
		}
	};
	let cur = ['/'];
	for (const seg of pattern.split('/').filter(Boolean)) {
		const next: string[] = [];
		for (const d of cur) {
			if (seg === '**') {
				const walk = (x: string) => {
					next.push(x);
					for (const e of ls(x)) if (e.isDirectory() && !e.name.startsWith('.')) walk(join(x, e.name));
				};
				walk(d);
			} else if (/[*?[]/.test(seg)) {
				const re = segRe(seg);
				for (const e of ls(d))
					if (re.test(e.name) && (seg.startsWith('.') || !e.name.startsWith('.'))) next.push(join(d, e.name));
			} else {
				const p = join(d, seg);
				try {
					lstatSync(p);
					next.push(p);
				} catch {}
			}
		}
		cur = [...new Set(next)];
	}
	return cur.sort();
}

/** rm -rf that first makes read-only dirs writable again (scenarios may chmod them). */
function removeTree(p: string) {
	try {
		rmSync(p, {recursive: true, force: true});
		return;
	} catch {}
	const fix = (x: string) => {
		let st;
		try {
			st = lstatSync(x);
		} catch {
			return;
		}
		if (!st.isDirectory()) return;
		chmodSync(x, 0o700);
		for (const e of readdirSync(x)) fix(join(x, e));
	};
	fix(p);
	rmSync(p, {recursive: true, force: true});
}

export async function runScenario(sc: Scenario, o: {bin: string; timeoutMs: number; keep: boolean}): Promise<Result> {
	const t0 = Date.now();
	const tmp = realpathSync(mkdtempSync(join(tmpdir(), 'xplain-e2e-'))); // real path: macOS /var -> /private/var
	const dirs = {
		TMP: tmp,
		REPO: join(tmp, 'repo'),
		HOME: join(tmp, 'home'),
		CONFIG: join(tmp, 'config'),
		STATE: join(tmp, 'state'),
		SHIMS: join(tmp, 'shims'),
	};
	const vars: Record<string, string> = {...dirs, ROOT};
	const sub = <T>(v: T): T => {
		if (typeof v === 'string')
			return v.replace(/\$\{(\w+)\}/g, (_, n: string) => vars[n] ?? fail(`unknown variable \${${n}}`)) as T;
		if (Array.isArray(v)) return v.map(sub) as T;
		if (v && typeof v === 'object') return Object.fromEntries(Object.entries(v).map(([k, x]) => [sub(k), sub(x)])) as T;
		return v;
	};
	const path = (p: unknown) => {
		const s = sub(String(p));
		return isAbsolute(s) ? s : join(dirs.REPO, s);
	};
	const rel = (f: string) => relative(dirs.TMP, f);
	const content = (c: unknown) => (typeof c === 'string' ? sub(c) : JSON.stringify(sub(c), null, 2) + '\n');
	const stderrFile = sc.tui && sc.captureStderr ? join(tmp, 'stderr.txt') : undefined;

	let session: Session | undefined;
	let out = '';
	let err = '';
	let exitCode: number | undefined;
	const guard = new Guard(o.timeoutMs);
	const aborts = new Set<() => void>();
	const inflight = new Map<string, Req>();
	const held = new Map<number, Server>();
	let stepNo = 0;
	let stepText: string | undefined;
	let rpcId = 0;
	// HTTP requests to the app: sent (connected or connecting) and settled (response, error after connect, abort)
	let httpSent = 0;
	let httpDone = 0;

	const guarded = <T>(p: Promise<T>, what: string) =>
		new Promise<T>((resolve, reject) => {
			const off = guard.on(() => reject(new HangError(`hang after ${o.timeoutMs}ms: ${what}`)));
			if (guard.expired) reject(new HangError(`hang after ${o.timeoutMs}ms: ${what}`));
			p.then(
				(v) => (off(), resolve(v)),
				(e) => (off(), reject(e)),
			);
		});

	// ---- fake CLIs ----
	const blocked = new Set<string>();
	const waiters = new Map<string, Socket[]>();
	let ctl: Server | undefined;
	let ctlPort = 0;
	const ctlSockets = new Set<Socket>();
	const startCtl = async () => {
		if (ctl) return;
		ctl = createServer((c) => {
			ctlSockets.add(c);
			c.on('close', () => ctlSockets.delete(c));
			c.on('error', () => {});
			let buf = '';
			c.setEncoding('utf8').on('data', (d: string) => {
				buf += d;
				const i = buf.indexOf('\n');
				if (i < 0) return;
				const name = buf.slice(0, i);
				if (blocked.has(name)) waiters.set(name, [...(waiters.get(name) ?? []), c]);
				else c.end('go\n');
			});
		});
		await new Promise<void>((r) => ctl!.listen(0, '127.0.0.1', r));
		ctlPort = (ctl.address() as {port: number}).port;
	};
	const shimRules = new Map<string, ShimRule[]>();
	const writeShim = async (name: string, rules: ShimRule[]) => {
		const r = sub(rules).map((x) => {
			if (!x.passthrough) return x;
			const exec =
				realBin(name) ??
				fail(`shim ${name}: passthrough, but no real ${name} in ${Object.keys(TOOLS).join(', ')}, /usr/bin, /bin`);
			return {...x, exec};
		});
		if (r.some((x) => x.block)) {
			await startCtl();
			blocked.add(name);
		} else blocked.delete(name);
		shimRules.set(name, rules);
		writeFileSync(join(dirs.SHIMS, `${name}.json`), JSON.stringify({rules: r, ctl: ctlPort}));
		const f = join(dirs.SHIMS, name);
		writeFileSync(
			f,
			`#!/bin/sh\nexec ${q(process.execPath)} ${q(join(E2E, 'lib', 'shim.mjs'))} ${q(dirs.SHIMS)} ${q(name)} "$@"\n`,
		);
		chmodSync(f, 0o755);
	};

	// ---- HTTP (MCP) ----
	const mcp = (req: Record<string, unknown>): Req => {
		const r = sub(req);
		let auth: string | undefined;
		if (r.auth === false) auth = undefined;
		else if (typeof r.auth === 'string') auth = r.auth;
		else {
			const f = join(dirs.STATE, 'xplain', 'mcp.json');
			if (!existsSync(f)) fail(`no MCP token file ${f} (is MCP running?)`);
			auth = `Bearer ${(JSON.parse(readFileSync(f, 'utf8')) as {token: string}).token}`;
		}
		const body =
			typeof r.raw === 'string'
				? r.raw.repeat(r.bodyRepeat === undefined ? 1 : Number(r.bodyRepeat))
				: JSON.stringify(
						r.body ??
							(r.tool !== undefined
								? {jsonrpc: '2.0', id: ++rpcId, method: 'tools/call', params: {name: r.tool, arguments: r.args ?? {}}}
								: {jsonrpc: '2.0', id: ++rpcId, method: r.method, params: r.params ?? {}}),
					);
		const port = Number(r.port ?? vars.PORT);
		const counted = !held.has(port); // requests to our own held ports never reach the app
		if (counted) httpSent++;
		let connected = false;
		let settled = false;
		const settle = () => {
			if (settled || !counted) return;
			settled = true;
			if (connected) httpDone++;
			else httpSent--; // never reached the app
		};
		let rq!: ClientRequest;
		const p = new Promise<Http>((resolve, reject) => {
			rq = request(
				{
					host: '127.0.0.1',
					port,
					path: String(r.path ?? '/mcp'),
					method: String(r.httpMethod ?? 'POST'),
					headers: {
						'content-type': 'application/json',
						accept: 'application/json, text/event-stream',
						...(auth ? {authorization: auth} : {}),
						...((r.headers ?? {}) as Record<string, string>),
					},
				},
				(res) => {
					let text = '';
					res.setEncoding('utf8');
					res.on('data', (d: string) => (text += d));
					res.on('error', (e) => (settle(), reject(e)));
					res.on('end', () => {
						const h: Http = {status: res.statusCode ?? 0, headers: res.headers, text};
						try {
							h.body = JSON.parse(text);
							const t = (h.body as {result?: {content?: {text?: string}[]}}).result?.content?.[0]?.text;
							if (typeof t === 'string')
								try {
									h.result = JSON.parse(t);
								} catch {
									h.result = t;
								}
						} catch {}
						settle();
						resolve(h);
					});
				},
			);
			rq.on('socket', (s) => {
				if (!s.connecting) connected = true;
				else s.once('connect', () => (connected = true));
			});
			rq.on('error', (e) => (settle(), reject(e)));
			aborts.add(() => rq.destroy());
			rq.end(body);
		});
		p.catch(() => {});
		return {p, rq, settle};
	};
	const checkHttp = (h: Http, st: Step) => {
		const e = sub((st.expect ?? {}) as Record<string, unknown>);
		const where = `http ${h.status} ${h.text.slice(0, 400)}`;
		if (e.status !== undefined && h.status !== e.status) fail(`${where}: expected status ${e.status}`);
		for (const [k, v] of [
			['body', h.body],
			['headers', h.headers],
		] as const)
			if (e[k] !== undefined) {
				const m = subset(v, e[k]);
				if (m) fail(`${where}: ${k} ${m}`);
			}
		if (e.result !== undefined) {
			const m =
				typeof h.result === 'string' && isTextMatch(e.result)
					? matchText(h.result, e.result as TextMatch, 'result')
					: subset(h.result, e.result, 'result');
			if (m) fail(`${where}: ${m}`);
		}
		if (e.text !== undefined) {
			const m = matchText(h.text, e.text as TextMatch, 'response text');
			if (m) fail(`${where}: ${m}`);
		}
		for (const [k, p] of Object.entries((st.capture ?? {}) as Record<string, string>)) {
			const v = pick({status: h.status, headers: h.headers, body: h.body, result: h.result, text: h.text}, p);
			if (v === undefined) fail(`${where}: capture ${k}: nothing at ${p}`);
			vars[k] = typeof v === 'string' ? v : JSON.stringify(v);
		}
	};
	const env = () => {
		const e: Record<string, string> = {
			PATH: `${dirs.SHIMS}:${join(tmp, 'bin')}:/usr/bin:/bin`, // host PATH is not inherited: agent CLIs only as shims
			HOME: dirs.HOME,
			XDG_CONFIG_HOME: dirs.CONFIG,
			XDG_STATE_HOME: dirs.STATE,
			XDG_CACHE_HOME: join(tmp, 'cache'),
			XDG_DATA_HOME: join(tmp, 'data'),
			TMPDIR: process.env.TMPDIR ?? '/tmp',
			TERM: 'xterm-256color',
			COLORTERM: 'truecolor',
			LANG: 'en_US.UTF-8',
			XPLAIN_SYNC: '1',
			XPLAIN_MCP_PORT: vars.PORT!,
			GIT_CONFIG_NOSYSTEM: '1',
			GIT_AUTHOR_NAME: 'x',
			GIT_AUTHOR_EMAIL: 'a@b.c',
			GIT_COMMITTER_NAME: 'x',
			GIT_COMMITTER_EMAIL: 'a@b.c',
		};
		for (const [k, v] of Object.entries(sub(sc.env))) {
			if (v === null) delete e[k];
			else e[k] = String(v);
		}
		return e;
	};
	const readErr = () => (stderrFile ? (existsSync(stderrFile) ? readFileSync(stderrFile, 'utf8') : '') : err);
	const dump = () => {
		if (session) {
			const c = session.cursor();
			const lines = session.lines().map((l, i) => `${String(i).padStart(3)}|${l}`);
			return [
				`--- screen ${session.cols}x${session.rows} (cursor ${c.row},${c.col}${c.visible ? '' : ' hidden'}; replies ${session.replies}/${session.sent}${session.appHttp ? `; app http ${session.appHttp.received}/${session.appHttp.done} vs sent ${httpSent}/${httpDone}` : ''}${session.exit ? `; exited ${session.exit.code}` : ''}) ---`,
				...lines,
				...(stderrFile ? ['--- stderr ---', readErr()] : []),
				'---',
			].join('\n');
		}
		return `--- exit ${exitCode} ---\n--- stdout ---\n${out}\n--- stderr ---\n${err}\n---`;
	};

	/** File content for writeFile generators. */
	const generate = (e: Record<string, unknown>): Buffer => {
		let b: Buffer;
		if (e.lines) {
			const l = e.lines as {from: number; to: number; text?: string};
			const t = l.text ?? '{n}';
			const rows: string[] = [];
			for (let n = l.from; n <= l.to; n++) rows.push(t.replaceAll('{n}', String(n)) + '\n');
			b = Buffer.from(rows.join(''));
		} else if (e.size !== undefined) {
			const size = Number(e.size);
			const fill = String(e.fill ?? 'x');
			const L = e.lineLength === undefined ? undefined : Number(e.lineLength);
			const unit = L ? fill.repeat(Math.ceil(L / fill.length)).slice(0, L - 1) + '\n' : fill;
			b = Buffer.from(unit.repeat(Math.ceil(size / unit.length) || 0)).subarray(0, size);
			if (b.length !== size) fail(`writeFile: fill must be single-byte characters to reach size ${size}`);
		} else if (e.bytes) {
			const x = e.bytes as {hex?: string; base64?: string};
			b = x.hex !== undefined ? Buffer.from(x.hex.replace(/\s+/g, ''), 'hex') : Buffer.from(x.base64!, 'base64');
		} else b = Buffer.from(content(e.content ?? ''));
		for (const off of ([] as number[]).concat((e.nulAt ?? []) as number[])) {
			if (off < 0 || off >= b.length) fail(`writeFile: nulAt ${off} outside ${b.length} bytes`);
			b[off] = 0;
		}
		return b;
	};

	const run = async (st: Step, exitNext: boolean) => {
		const k = stepKind(st);
		const v = k === 'keys' || k === 'paste' ? st[k] : sub(st[k]);
		const s = session!;
		const bk = (st.wait ?? 'idle') as BarrierKind | 'none';
		switch (k) {
			case 'keys':
				await s.keys(String(v), exitNext, bk);
				return;
			case 'paste':
				if (bk === 'none') s.send(String(v));
				else await s.barrier(exitNext, bk, String(v));
				return;
			case 'barrier':
				await s.barrier(exitNext, v as BarrierKind);
				return;
			case 'expectScreen': {
				const e = v as Record<string, unknown>;
				const lines = s.lines();
				const row = (r: number) => (r < 0 ? s.rows + r : r);
				const m = matchText(lines.join('\n'), e as TextMatch, 'screen');
				if (m) fail(m);
				for (const l of ([] as Record<string, unknown>[]).concat((e.line ?? []) as Record<string, unknown>[])) {
					const r = row(Number(l.row));
					const {row: _, ...tm} = l;
					const m2 = matchText(
						lines[r] ?? fail(`row ${r} outside the screen`),
						tm as TextMatch,
						`row ${r} ${JSON.stringify(lines[r] ?? '')}`,
					);
					if (m2) fail(m2);
				}
				for (const g of ([] as Record<string, unknown>[]).concat((e.region ?? []) as Record<string, unknown>[])) {
					const [a, b] = ((g.rows ?? [0, -1]) as number[]).map(row);
					const [c0, c1] = (g.cols ?? [0, s.cols - 1]) as number[];
					const txt = Array.from({length: b! - a! + 1}, (_, i) => {
						const {text, colOf} = s.rowInfo(a! + i);
						return [...text].filter((_, j) => colOf[j]! >= c0! && colOf[j]! <= c1!).join('');
					}).join('\n');
					const {rows: _r, cols: _c, ...tm} = g;
					const m2 = matchText(txt, tm as TextMatch, `region rows ${a}-${b} cols ${c0}-${c1}`);
					if (m2) fail(m2);
				}
				if (e.cursor) {
					const c = s.cursor();
					const m2 = subset(c, e.cursor, 'cursor');
					if (m2) fail(m2);
				}
				return;
			}
			case 'expectCell': {
				const e = v as Record<string, unknown>;
				const at = (e.at ?? {}) as {text?: string; nth?: number; offset?: number; row?: number; col?: number};
				let r = at.row !== undefined && at.row < 0 ? s.rows + at.row : at.row;
				let c = at.col;
				if (at.text !== undefined) {
					let nth = at.nth ?? 0;
					c = undefined;
					for (let y = r ?? 0; y <= (r ?? s.rows - 1) && c === undefined; y++) {
						const {text, colOf} = s.rowInfo(y);
						for (let i = text.indexOf(at.text); i >= 0; i = text.indexOf(at.text, i + 1))
							if (nth-- === 0) {
								r = y;
								c = colOf[i + (at.offset ?? 0)];
								break;
							}
					}
					if (c === undefined) fail(`expectCell: text ${JSON.stringify(at.text)} not on screen`);
				}
				if (r === undefined || c === undefined) fail('expectCell: at needs {text} or {row, col}');
				const cell = s.cell(r!, c!) ?? fail(`expectCell: no cell at ${r},${c}`);
				const want: Record<string, unknown> = {};
				for (const [key, x] of Object.entries(e))
					if (key !== 'at') want[key] = typeof x === 'string' && key !== 'chars' ? x.toLowerCase() : x;
				const m = subset(cell, want, `cell ${r},${c}`);
				if (m) fail(`${m} (cell ${JSON.stringify(cell)})`);
				return;
			}
			case 'expectGolden': {
				const name = String(v);
				const f = join(E2E, 'golden', name.endsWith('.txt') ? name : `${name}.txt`);
				let txt = s.lines().join('\n').replace(/\s+$/, '') + '\n';
				for (const n of ['REPO', 'HOME', 'CONFIG', 'STATE', 'SHIMS', 'TMP'] as const)
					txt = txt.split(dirs[n]).join(`\${${n}}`);
				if (process.env.E2E_UPDATE === '1') {
					mkdirSync(dirname(f), {recursive: true});
					writeFileSync(f, txt);
					return;
				}
				if (!existsSync(f)) fail(`golden ${relative(ROOT, f)} missing (run with E2E_UPDATE=1)`);
				const g = readFileSync(f, 'utf8');
				if (g !== txt) {
					const a = g.split('\n');
					const b = txt.split('\n');
					const i = a.findIndex((l, j) => l !== b[j]);
					fail(
						`golden ${relative(ROOT, f)} differs at row ${i}:\n  want ${JSON.stringify(a[i])}\n  got  ${JSON.stringify(b[i])}`,
					);
				}
				return;
			}
			case 'expectOsc52': {
				const e = (typeof v === 'string' ? {contains: v} : v) as TextMatch & {count?: number};
				if (typeof e === 'object' && e.count !== undefined && s.clipboard.length !== e.count)
					fail(`osc52: ${s.clipboard.length} copies, expected ${e.count}`);
				const last = s.clipboard.at(-1);
				if (last === undefined) {
					if (typeof e === 'object' && e.count === 0) return;
					fail('osc52: nothing copied');
				}
				const {count: _, ...m} = e as Record<string, unknown>;
				if (Object.keys(m).length) {
					const r = matchText(last!, m as TextMatch, `osc52 ${JSON.stringify(last)}`);
					if (r) fail(r);
				}
				return;
			}
			case 'captureScreen': {
				const e = v as {matches: string; row?: number; var?: string};
				const r = e.row === undefined ? undefined : e.row < 0 ? s.rows + e.row : e.row;
				const txt = r === undefined ? s.lines().join('\n') : s.lines()[r]!;
				const m =
					new RegExp(e.matches, 'm').exec(txt) ??
					fail(`captureScreen: /${e.matches}/m not on ${r === undefined ? 'screen' : `row ${r}`}`);
				for (const [n, x] of Object.entries(m.groups ?? {}))
					vars[n] = x ?? fail(`captureScreen: group ${n} did not match`);
				if (e.var) vars[e.var] = m[1] ?? m[0];
				return;
			}
			case 'httpStart': {
				const e = v as Record<string, unknown>;
				const {id, ...rq} = e;
				if (inflight.has(String(id))) fail(`httpStart: id ${id} already in flight`);
				inflight.set(String(id), mcp(rq));
				return;
			}
			case 'http':
			case 'httpAwait': {
				const e = v as Record<string, unknown>;
				let p: Promise<Http>;
				if (k === 'http') p = mcp(e).p;
				else {
					const id = String(typeof v === 'string' ? v : e.id);
					p = (inflight.get(id) ?? fail(`httpAwait: no httpStart with id ${id} in flight`)).p;
					inflight.delete(id);
				}
				const h = await guarded(p, `${k} response`);
				checkHttp(h, st);
				await s.barrier(exitNext);
				return;
			}
			case 'httpAbort': {
				const id = String(v);
				const r = inflight.get(id) ?? fail(`httpAbort: no httpStart with id ${id} in flight`);
				inflight.delete(id);
				await s.barrier(); // the app has received it (request counters), so the abort is a dropped connection
				r.rq.destroy();
				r.settle();
				await s.barrier(exitNext); // ... and has finished handling it
				return;
			}
			case 'expectRefused': {
				const port = Number(typeof v === 'object' ? ((v as {port?: unknown}).port ?? vars.PORT) : v);
				const res = await guarded(
					new Promise<string | undefined>((resolve) => {
						const c = connect(port, '127.0.0.1');
						c.on('connect', () => (c.destroy(), resolve(`connected to 127.0.0.1:${port}, expected refused`)));
						c.on('error', (er: NodeJS.ErrnoException) =>
							resolve(er.code === 'ECONNREFUSED' ? undefined : `127.0.0.1:${port}: ${er.code}, expected ECONNREFUSED`),
						);
					}),
					'connect',
				);
				if (res) fail(res);
				return;
			}
			case 'holdPort': {
				const e = v as {port?: unknown; var?: string};
				const port = e.var ? await freePort() : Number(e.port);
				if (held.has(port)) fail(`holdPort: ${port} already held`);
				try {
					held.set(port, await listenOn(port));
				} catch (er) {
					fail(`holdPort ${port}: ${(er as Error).message}`);
				}
				if (e.var) vars[e.var] = String(port);
				return;
			}
			case 'release': {
				const name = String(v);
				if (!blocked.has(name)) fail(`release: shim ${name} is not blocking`);
				await writeShim(
					name,
					(shimRules.get(name) ?? []).map(({block: _, ...r}) => r),
				);
				for (const c of waiters.get(name) ?? []) c.end('go\n');
				waiters.delete(name);
				await s.barrier(exitNext);
				return;
			}
			default:
				return runCommon(k, v, st);
		}
	};
	// steps valid in both TUI and non-TUI scenarios
	const runCommon = async (k: string, v: unknown, st: Step) => {
		switch (k) {
			case 'expectFile': {
				const e = v as Record<string, unknown>;
				let files: string[];
				if (e.glob !== undefined) {
					files = glob(path(e.glob));
					const want = e.count !== undefined ? Number(e.count) : e.exists === false ? 0 : 1;
					if (files.length !== want)
						fail(
							`glob ${e.glob}: ${files.length} matches, expected ${want}${files.length ? ':\n  ' + files.map(rel).join('\n  ') : ''}`,
						);
				} else {
					const f = path(e.path);
					let ex = true;
					try {
						lstatSync(f);
					} catch {
						ex = false;
					}
					if (ex !== (e.exists ?? true)) fail(`${rel(f)}: expected ${ex ? 'absent' : 'present'}`);
					files = ex ? [f] : [];
				}
				for (const f of files) {
					if (e.mode !== undefined) {
						const m = statSync(f).mode & 0o7777;
						if (m !== octal(e.mode)) fail(`${rel(f)}: mode ${m.toString(8)}, expected ${String(e.mode)}`);
					}
					if (e.name !== undefined) {
						const m = matchText(basename(f), e.name as TextMatch, `name ${basename(f)}`);
						if (m) fail(m);
					}
					const tm = Object.fromEntries(Object.entries(e).filter(([x]) => TM_KEYS.includes(x)));
					if (!Object.keys(tm).length && e.json === undefined) continue;
					if (statSync(f).isDirectory()) fail(`${rel(f)}: is a directory, content checks need a file`);
					const txt = readFileSync(f, 'utf8');
					const m = matchText(txt, tm as TextMatch, rel(f));
					if (m) fail(m);
					if (e.json !== undefined) {
						let j: unknown;
						try {
							j = JSON.parse(txt);
						} catch {
							fail(`${rel(f)}: not JSON`);
						}
						const m2 = subset(j, e.json);
						if (m2) fail(`${rel(f)}: ${m2}`);
					}
				}
				if (e.capturePath) vars[String(e.capturePath)] = files[0]!;
				return;
			}
			case 'writeFile': {
				const e = v as Record<string, unknown>;
				const f = path(e.path);
				mkdirSync(dirname(f), {recursive: true});
				writeFileSync(f, generate(e), {flag: e.append ? 'a' : 'w'});
				if (e.mode !== undefined) chmodSync(f, octal(e.mode));
				return;
			}
			case 'removeFile':
				rmSync(path(v), {recursive: true, force: true});
				return;
			case 'copyFile': {
				const e = v as {from: string; to: string; mode?: string};
				const to = path(e.to);
				mkdirSync(dirname(to), {recursive: true});
				copyFileSync(path(e.from), to);
				if (e.mode !== undefined) chmodSync(to, octal(e.mode));
				return;
			}
			case 'mkdir': {
				const e = (typeof v === 'string' ? {path: v} : v) as {path: string; mode?: string};
				const d = path(e.path);
				mkdirSync(d, {recursive: true});
				if (e.mode !== undefined) chmodSync(d, octal(e.mode));
				return;
			}
			case 'chmod': {
				const e = v as {path: string; mode: string};
				chmodSync(path(e.path), octal(e.mode));
				return;
			}
			case 'symlink': {
				const e = v as {target: string; path: string};
				const f = path(e.path);
				mkdirSync(dirname(f), {recursive: true});
				symlinkSync(e.target, f);
				return;
			}
			case 'git': {
				const raw = (st.git && typeof st.git === 'object' && !Array.isArray(st.git) ? st.git : {args: st.git}) as {
					args: string | string[];
					code?: number;
				};
				// split before substituting, so a variable value is always one argument
				const args = (Array.isArray(raw.args) ? raw.args : splitArgs(raw.args)).map((a) => sub(a));
				const r = await guarded(
					new Promise<{code: number; out: string}>((resolve) =>
						execFile(TOOLS.git!, args, {cwd: dirs.REPO, env: env()}, (er, so, se) =>
							resolve({code: er ? Number((er as {code?: number}).code ?? 1) : 0, out: so + se}),
						),
					),
					`git ${args.join(' ')}`,
				);
				if (r.code !== (raw.code ?? 0)) fail(`git ${JSON.stringify(args)}: exit ${r.code}: ${r.out}`);
				return;
			}
			case 'shim': {
				const e = v as {name: string; rules?: ShimRule[]; remove?: boolean} & ShimRule;
				if (e.remove) {
					rmSync(join(dirs.SHIMS, e.name), {force: true});
					rmSync(join(dirs.SHIMS, `${e.name}.json`), {force: true});
					blocked.delete(e.name);
					return;
				}
				const {name, rules, remove: _, ...rule} = e;
				await writeShim(name, rules ?? [rule]);
				return;
			}
			case 'expectShimCall': {
				const e = v as {
					name: string;
					args?: string[];
					argsPrefix?: string[];
					argsContain?: string[];
					count?: number;
					stdin?: TextMatch;
					cwd?: string;
					nth?: number;
					sequence?: string[][];
				};
				const f = join(dirs.SHIMS, 'calls.log');
				const calls = (existsSync(f) ? readFileSync(f, 'utf8').split('\n').filter(Boolean) : [])
					.map((l) => JSON.parse(l) as {name: string; argv: string[]; cwd: string; stdin?: string})
					.filter((c) => c.name === e.name);
				const show = () => calls.map((c, i) => `  #${i} ${JSON.stringify(c)}`).join('\n') || '  (none)';
				const prefix = (c: {argv: string[]}, p: string[]) => p.every((a, i) => c.argv[i] === a);
				if (e.sequence) {
					let i = 0;
					for (const c of calls) if (i < e.sequence.length && prefix(c, e.sequence[i]!)) i++;
					if (i < e.sequence.length)
						fail(
							`shim ${e.name}: calls lack ${JSON.stringify(e.sequence[i])} after ${JSON.stringify(e.sequence.slice(0, i))} in order; calls:\n${show()}`,
						);
					return;
				}
				const ok = (c: (typeof calls)[number]) =>
					(!e.args || JSON.stringify(c.argv) === JSON.stringify(e.args)) &&
					(!e.argsPrefix || prefix(c, e.argsPrefix)) &&
					(!e.argsContain || e.argsContain.every((a) => c.argv.includes(a))) &&
					(!e.stdin || !matchText(c.stdin ?? '', e.stdin)) &&
					(!e.cwd || c.cwd === e.cwd);
				if (e.nth !== undefined) {
					const c = calls[e.nth] ?? fail(`shim ${e.name}: no call #${e.nth}; calls:\n${show()}`);
					if (!ok(c)) fail(`shim ${e.name}: call #${e.nth} does not match; calls:\n${show()}`);
					if (e.count !== undefined && calls.length !== e.count)
						fail(`shim ${e.name}: ${calls.length} calls, expected ${e.count}; calls:\n${show()}`);
					return;
				}
				const hits = calls.filter(ok);
				const good = e.count !== undefined ? hits.length === e.count : hits.length > 0;
				if (!good) fail(`shim ${e.name}: ${hits.length} matching calls (want ${e.count ?? '>=1'}); calls:\n${show()}`);
				return;
			}
			case 'expectExit': {
				const want = typeof v === 'number' ? v : (v as {code?: number})?.code;
				if (session) await session.waitExit();
				const code = session ? session.exit!.code : exitCode;
				if (want !== undefined && code !== want) fail(`exit code ${code}, expected ${want}`);
				return;
			}
			case 'expectStdout':
			case 'expectStderr': {
				const m = matchText(
					k === 'expectStdout' ? out : readErr(),
					v as TextMatch,
					k === 'expectStdout' ? 'stdout' : 'stderr',
				);
				if (m) fail(m);
				return;
			}
		}
	};

	try {
		mkdirSync(dirs.HOME, {recursive: true});
		mkdirSync(dirs.SHIMS, {recursive: true});
		mkdirSync(join(tmp, 'bin'));
		for (const [n, p] of Object.entries(TOOLS)) symlinkSync(p, join(tmp, 'bin', n));
		execFileSync('sh', [join(E2E, 'fixture.sh'), dirs.REPO, sc.fixture], {stdio: 'pipe'});
		vars.PORT = String(await freePort());
		for (const [name, rules] of Object.entries(sc.shims)) await writeShim(name, rules);
		for (const [p, c] of Object.entries(sc.files)) {
			const f = path(p);
			mkdirSync(dirname(f), {recursive: true});
			writeFileSync(f, content(c));
		}
		const cwd = sc.cwd === undefined ? dirs.REPO : path(sc.cwd);
		mkdirSync(cwd, {recursive: true});
		const args = sub(sc.args);
		const firstExit = sc.steps[0] !== undefined && stepKind(sc.steps[0]) === 'expectExit';
		if (sc.tui) {
			stepText = '(startup)';
			session = new Session(o.bin, args, {cwd, env: env(), cols: sc.cols, rows: sc.rows, stderrFile}, guard, () => ({
				sent: httpSent,
				done: httpDone,
			}));
			await session.waitReady();
			if (!sc.startWithoutBarrier) await session.barrier(firstExit); // answered after the first full frame and initial load
		} else {
			stepText = '(run)';
			const ch = spawn('/bin/sh', ['-c', `exec ${o.bin} "$@"`, 'xplain', ...args], {
				cwd,
				env: env(),
				stdio: ['ignore', 'pipe', 'pipe'],
			});
			ch.stdout.setEncoding('utf8').on('data', (d: string) => (out += d));
			ch.stderr.setEncoding('utf8').on('data', (d: string) => (err += d));
			aborts.add(() => ch.kill('SIGKILL'));
			exitCode = await guarded(
				new Promise<number>((resolve) => ch.on('close', (c, sig) => resolve(c ?? (sig ? 128 : 1)))),
				'no exit',
			);
		}
		for (let i = 0; i < sc.steps.length; i++) {
			stepNo = i + 1;
			const st = sc.steps[i]!;
			const next = sc.steps[i + 1];
			const exitNext = next !== undefined && stepKind(next) === 'expectExit';
			stepText = JSON.stringify(st);
			if (sc.tui) await run(st, exitNext);
			else await runCommon(stepKind(st), sub(st[stepKind(st)]), st);
		}
		return {ok: true, ms: Date.now() - t0};
	} catch (e) {
		const kind =
			e instanceof HangError ? 'HANG' : e instanceof ExitError ? 'EXIT' : e instanceof StepError ? '' : 'ERROR';
		return {
			ok: false,
			ms: Date.now() - t0,
			step: stepNo,
			stepText,
			error: `${kind ? kind + ': ' : ''}${(e as Error).message}`,
			dump: dump(),
		};
	} finally {
		guard.clear();
		aborts.forEach((f) => f());
		session?.kill();
		for (const c of ctlSockets) c.destroy();
		ctl?.close();
		for (const srv of held.values()) srv.close();
		if (!o.keep) removeTree(tmp);
	}
}
