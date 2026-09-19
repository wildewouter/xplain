import {rmSync, mkdtempSync, writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {claudeProvider, parseSession} from '../src/agents/claude.js';
import {codexProvider} from '../src/agents/codex.js';
import {copilotProvider} from '../src/agents/copilot.js';
import {listAgents} from '../src/agents/index.js';
import {opencodeProvider} from '../src/agents/opencode.js';
import {classify, parsePs} from '../src/agents/ps.js';
import type {AgentProvider} from '../src/agents/types.js';

let fail = 0;
const ok = (n: string, c: boolean) => {
	console.log(c ? 'PASS' : 'FAIL', n);
	if (!c) fail++;
};
ok('claude', classify('claude --resume') === 'claude');
ok('claude abs path', classify('/Users/x/.local/bin/claude') === 'claude');
ok('claude-code', classify('claude-code') === 'claude');
ok('node claude script', classify('node /usr/lib/node_modules/@anthropic-ai/claude-code/cli.js') === 'claude');
ok('node flags claude script', classify('/usr/bin/node --no-warnings /opt/claude/bin/claude.js') === 'claude');
ok('codex', classify('codex exec foo') === 'codex');
ok('node codex', classify('node /usr/lib/node_modules/@openai/codex/bin/codex.js') === 'codex');
ok('opencode', classify('/home/u/.opencode/bin/opencode') === 'opencode');
ok('copilot', classify('copilot') === 'copilot');
ok('gh copilot', classify('gh copilot suggest hi') === 'copilot');
ok('gh pr not copilot', classify('gh pr list') === undefined);
ok('grep noise', classify('grep claude') === undefined);
ok('xplain noise', classify('node /Users/w/Projects/xplain/dist/cli.js --claude') === undefined);
ok('vim noise', classify('vim /repo/claude/codex.md') === undefined);
ok('ps noise', classify('ps -axo pid=,etime=,command= codex') === undefined);
ok('tsx xplain in claude dir', classify('tsx /Users/w/claude/xplain/src/cli.tsx') === undefined);
const out = [
	'  10 01:02 claude',
	'  9 00:05 codex exec',
	'  11 1-02:03:04 node /x/opencode/cli.js',
	'  12 00:01 gh copilot',
	'  13 00:01 grep claude',
	'  14 00:01 vim x.md',
	'  99 00:01 claude',
	'  8 00:01 claude',
].join('\n');
const r = parsePs(out, 99);
ok('parse count, self excluded', r.length === 5 && !r.some((a) => a.pid === 99));
ok(
	'sorted agent then pid',
	r.map((a) => `${a.agent}${a.pid}`).join() === 'claude8,claude10,codex9,copilot12,opencode11',
);
ok('uptime parsed', r.find((a) => a.pid === 11)?.uptime === '1-02:03:04');
const sj = (o: object) => JSON.stringify(o);
const v = parseSession(
	sj({pid: 5, sessionId: 'eabb4231-8bc5', cwd: '/x', startedAt: 1000, kind: 'bg', name: 'my job', status: 'busy'}),
	() => true,
	1000 + 90061000,
);
ok(
	'session valid',
	v?.agent === 'claude' &&
		v.pid === 5 &&
		v.name === 'my job' &&
		v.status === 'busy' &&
		v.kind === 'bg' &&
		v.cwd === '/x',
);
ok('session sessionId', v?.sessionId === 'eabb4231-8bc5');
ok('session uptime', v?.uptime === '1-01:01:01');
ok('session name fallback', parseSession(sj({pid: 5, sessionId: 'eabb4231-8bc5'}))?.name === 'eabb4231');
ok('session malformed', parseSession('{nope') === undefined);
ok('session non-object', parseSession('42') === undefined);
ok('session missing pid', parseSession(sj({sessionId: 'a'})) === undefined);
ok('session dead pid', parseSession(sj({pid: 5}), () => false) === undefined);

const fake = {
	ps: async () => out,
	lsof: async (pids: number[]) => new Map(pids.map((p) => [p, `/cwd/${p}`])),
	self: 99,
};
const ids = (l: {pid: number}[]) => l.map((a) => a.pid).join();
const cp = await copilotProvider(fake).list();
ok('copilot provider', cp.length === 1 && cp[0]!.pid === 12 && cp[0]!.agent === 'copilot' && cp[0]!.cwd === '/cwd/12');
ok('codex provider', ids(await codexProvider(fake).list()) === '9');
ok('opencode provider', ids(await opencodeProvider(fake).list()) === '11');
ok('provider ids', copilotProvider().id === 'copilot' && codexProvider().label === 'Codex');
ok('lsof failure gives blank cwd', (await codexProvider({...fake, lsof: async () => new Map()}).list())[0]?.cwd === '');
const dir = mkdtempSync(join(tmpdir(), 'xp-sess-'));
writeFileSync(join(dir, '5.json'), sj({pid: 5, sessionId: 'abcdefghij', cwd: '/c'}));
writeFileSync(join(dir, '6.json'), sj({pid: 6}));
writeFileSync(join(dir, 'x.txt'), 'nope');
writeFileSync(join(dir, '7.json'), '{bad');
const cl = await claudeProvider({dir, isAlive: (p) => p === 5, now: () => 0}).list();
ok('claude provider', cl.length === 1 && cl[0]!.pid === 5 && cl[0]!.name === 'abcdefgh' && cl[0]!.cwd === '/c');
ok('claude provider missing dir', (await claudeProvider({dir: join(dir, 'nope')}).list()).length === 0);
const row = (agent: any, pid: number) => ({agent, pid, uptime: '', cwd: '', cmd: ''});
const P = (id: any, f: () => Promise<any>): AgentProvider => ({id, label: id, list: f});
const reg = await listAgents([
	P('opencode', async () => [row('opencode', 3)]),
	P('codex', async () => {
		throw new Error('boom');
	}),
	P('claude', async () => [row('claude', 20), row('claude', 4)]),
]);
ok('throwing provider skipped', reg.length === 3);
ok('registry sorted', reg.map((a) => `${a.agent}${a.pid}`).join() === 'claude4,claude20,opencode3');
rmSync(dir, {recursive: true, force: true});
process.exit(fail ? 1 : 0);
