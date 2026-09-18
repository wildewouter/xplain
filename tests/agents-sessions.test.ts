import {mkdirSync, mkdtempSync, utimesSync, writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {codexProvider, dedupeWrappers, parseRollout, rolloutId, statusFromTail} from '../src/agents/codex.js';
import {copilotProvider, parseWorkspace} from '../src/agents/copilot.js';
import {opencodeProvider} from '../src/agents/opencode.js';
import {inlineParams, nodeSqliteBackend, query} from '../src/agents/sqlite.js';

let fail = 0;
const ok = (n: string, c: boolean) => {
	console.log(c ? 'PASS' : 'FAIL', n);
	if (!c) fail++;
};
const tmp = mkdtempSync(join(tmpdir(), 'xp-s2-'));
const hd = join(tmp, 'home');
mkdirSync(hd);
writeFileSync(join(hd, 'state_5.sqlite'), '');
const UU = '019e3ffb-ec11-7201-bb6e-27dbd372728a';

// sqlite helper
const calls: string[] = [];
ok(
	'sqlite order + fallthrough',
	(
		await query(
			'd',
			's',
			[],
			[
				async () => {
					calls.push('a');
					throw new Error('x');
				},
				async () => {
					calls.push('b');
					return [{v: 1}];
				},
				async () => {
					calls.push('c');
					return [];
				},
			],
		)
	)[0]?.v === 1 && calls.join() === 'a,b',
);
ok('sqlite all fail -> []', (await query('d', 's', [], [async () => Promise.reject(new Error('x'))])).length === 0);
ok('sqlite no backends -> []', (await query('d', 's', [], [])).length === 0);
ok(
	'inline escape',
	inlineParams('a=? and b=? and c=?', ["o'k", 3, null]) ===
		'a=? and b=? and c=?'.replace('?', "'o''k'").replace('?', '3').replace('?', 'NULL'),
);
let realOk = true;
try {
	const {DatabaseSync} = await import('node:sqlite');
	const p = join(tmp, 't.db');
	const d = new DatabaseSync(p);
	d.exec("create table t(id integer, s text); insert into t values (1,'a'),(2,'b''c')");
	d.close();
	const r = await nodeSqliteBackend(p, 'select id from t where s = ?', ["b'c"]);
	ok('real node:sqlite', r.length === 1 && r[0]!.id === 2);
	ok('query() real', (await query(p, 'select count(*) as n from t', []))[0]?.n === 2);
	ok('query missing db -> []', (await query(join(tmp, 'no.db'), 'select 1', [])).length === 0);
} catch {
	realOk = false;
	console.log('SKIP real node:sqlite');
}
void realOk;

// copilot
const cdir = join(tmp, 'cop');
const mk = (id: string, pid: number, yaml: string) => {
	mkdirSync(join(cdir, id), {recursive: true});
	writeFileSync(join(cdir, id, `inuse.${pid}.lock`), '');
	writeFileSync(join(cdir, id, 'workspace.yaml'), yaml);
};
mk(
	's1',
	100,
	'id: s1\ncwd: /w/a\nclient_name: copilot-intellij\nname: Fix bug\ncreated_at: 2026-01-01T00:00:00.000Z\n',
);
mk('s2', 101, 'id: s2\ncwd: /w/b\nname: Stale\n');
mk('s3', 102, 'id: s3\ncwd: /w/c\nclient_name: github/cli\nname: Busy one\n');
writeFileSync(join(cdir, 's3', 'events.jsonl'), '{}');
mkdirSync(join(cdir, 'ide'));
writeFileSync(join(cdir, 'ide', 'nonce.lock'), 'SECRET');
ok('yaml parse', parseWorkspace('id: x\nname: "A b"\nfoo: 1\nname: dup\n').name === 'A b');
const cps = ['  100 05:00 copilot', '  102 01:00 copilot', '  200 00:10 gh copilot'].join('\n');
const cfake = {
	ps: async () => cps,
	lsof: async (p: number[]) => new Map(p.map((x) => [x, `/cwd/${x}`])),
	self: 1,
	dir: cdir,
	isAlive: (p: number) => p !== 101,
	comm: async (p: number) => p !== 102 || true,
	now: () => Date.now(),
};
const cl = await copilotProvider(cfake).list();
ok(
	'copilot live sessions, stale dropped',
	cl
		.filter((a) => a.name)
		.map((a) => a.pid)
		.sort()
		.join() === '100,102',
);
ok(
	'copilot fields',
	cl.find((a) => a.pid === 100)?.kind === 'copilot-intellij' &&
		cl.find((a) => a.pid === 100)?.name === 'Fix bug' &&
		cl.find((a) => a.pid === 100)?.cwd === '/w/a',
);
ok('copilot ps uptime reused', cl.find((a) => a.pid === 100)?.uptime === '05:00');
ok(
	'copilot busy/idle',
	cl.find((a) => a.pid === 102)?.status === 'busy' && cl.find((a) => a.pid === 100)?.status === 'idle',
);
ok(
	'copilot dedupe + leftover',
	cl.length === 3 && cl.filter((a) => a.pid === 100).length === 1 && cl.some((a) => a.pid === 200 && !a.name),
);
ok('copilot IDE lock ignored', !cl.some((a) => a.name === 'nonce'));
ok('copilot pid reuse guard', !(await copilotProvider({...cfake, comm: async () => false}).list()).some((a) => a.name));
ok('copilot missing dir', (await copilotProvider({...cfake, dir: join(tmp, 'no')}).list()).length === 3);

// codex
ok('rolloutId', rolloutId(`/x/rollout-2026-01-01T00-00-00-${UU}.jsonl`) === UU);
ok('parseRollout', parseRollout(`p1\nn/a/b\nn/x/rollout-2026-${UU}.jsonl\n`)?.endsWith('.jsonl') === true);
ok(
	'tail busy',
	statusFromTail('{"type":"task_started"}\n{"type":"task_complete"}\n{"type":"task_started"}\n') === 'busy',
);
ok('tail idle', statusFromTail('{"type":"task_started"}\n{"type":"task_complete"}\n') === 'idle');
const cx = [
	'  1 01:00 node /u/@openai/codex/bin/codex.js',
	'  2 01:00 /u/vendor/codex',
	'  3 01:00 codex app-server',
	'  4 01:00 codex mcp',
	'  5 01:00 codex exec-server',
].join('\n');
const cxBase = {
	ps: async () => cx,
	lsof: async (p: number[]) => new Map(p.map((x) => [x, '/proj'])),
	self: 99,
	home: join(tmp, 'nohome'),
	now: () => 1_000_000,
	mtime: () => 0,
	openRollout: async () => undefined,
	query: async () => [],
};
const c0 = await codexProvider(cxBase).list();
ok('codex dedupe wrapper, skip helpers', c0.length === 1 && c0[0]!.pid === 2 && !c0[0]!.name);
ok(
	'dedupeWrappers lone wrapper kept',
	dedupeWrappers([{agent: 'codex', pid: 1, uptime: '', cwd: '/x', cmd: 'node /a/codex.js'}]).length === 1,
);
const c1 = await codexProvider({
	...cxBase,
	openRollout: async () => `/r/rollout-x-${UU}.jsonl`,
	query: async (_d, sql) => (sql.includes('from threads where id') ? [{id: UU, title: 'T1', model: 'gpt'}] : []),
	readTail: () => '{"type":"task_started"}\n',
	home: join(tmp, 'home'),
}).list();
ok('codex (i) lsof rollout + tail busy', c1[0]?.name === 'T1' && c1[0]?.status === 'busy' && c1[0]?.kind === 'gpt');
writeFileSync(join(hd, 'logs_2.sqlite'), '');
writeFileSync(join(hd, 'logs_10.sqlite'), '');
const seen: string[] = [];
const qfake = async (db: string, sql: string, p: any[]) => {
	seen.push(db.split('/').pop() + ':' + p[0]);
	if (sql.includes('from logs')) return [{thread_id: 'LOG-ID'}];
	if (sql.includes('where id')) return [{id: p[0], first_user_message: 'hello', rollout_path: '/nope'}];
	return [];
};
const c2 = await codexProvider({...cxBase, home: hd, query: qfake, readTail: () => '{"type":"task_complete"}'}).list();
ok('codex (ii) logs highest version', seen[0] === 'logs_10.sqlite:pid:2:%' && c2[0]?.name === 'hello');
const c3 = await codexProvider({
	...cxBase,
	home: hd,
	query: async (_d, sql, p) =>
		sql.includes('from logs')
			? []
			: sql.includes('where id')
				? [{id: p[0], title: 'S', rollout_path: '/r/x.jsonl'}]
				: [{id: 'ST-1'}],
	mtime: () => 1_000_000 - 100,
	readTail: () => '',
}).list();
ok('codex (iii) state cwd + mtime busy', c3[0]?.name === 'S' && c3[0]?.status === 'busy');
const c4 = await codexProvider({
	...cxBase,
	home: hd,
	query: async () => {
		throw new Error('x');
	},
}).list();
ok('codex query throws -> bare row', c4.length === 1 && !c4[0]!.name);

// opencode
const oc = [
	'  10 01:00 opencode',
	'  11 01:00 opencode',
	'  12 01:00 opencode serve --port 1',
	'  13 01:00 opencode',
].join('\n');
const ofake = {
	ps: async () => oc,
	lsof: async (p: number[]) => new Map(p.map((x) => [x, x === 13 ? '/other' : '/proj'])),
	self: 99,
	db: 'x',
	now: () => 100_000,
	query: async (_d: string, _s: string, p: any[]) =>
		p[0] === '/proj'
			? [
					{title: 'new', time_updated: 99_000},
					{title: 'old', time_updated: 1},
				]
			: [],
};
const o = await opencodeProvider(ofake).list();
ok('opencode skips serve', o.map((a) => a.pid).join() === '10,11,13');
ok(
	'opencode Nth proc Nth session',
	o[0]!.name === 'new' && o[0]!.status === 'busy' && o[1]!.name === 'old' && o[1]!.status === 'idle',
);
ok('opencode no session -> bare', !o[2]!.name);
const o2 = await opencodeProvider({...ofake, query: async () => []}).list();
ok('opencode no db fallback', o2.length === 3 && !o2[0]!.name);
process.exit(fail ? 1 : 0);
