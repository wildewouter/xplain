import {createIntegrations, byId} from '../src/integrations/index.js';
import type {IntegrationDeps, RunResult} from '../src/integrations/types.js';

let fail = 0;
const ok = (n: string, c: boolean) => {
	console.log(c ? 'PASS' : 'FAIL', n);
	if (!c) fail++;
};
const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);

const ep = {url: 'http://127.0.0.1:47615/mcp', token: 'SECRETTOKEN123', port: 47615};
type Call = {cmd: string; args: string[]};
type Handler = (cmd: string, args: string[]) => RunResult | Error;
const mk = (h: Handler) => {
	const calls: Call[] = [];
	const deps: IntegrationDeps = {
		cwd: '/w',
		home: '/h',
		run: async (cmd, args) => {
			calls.push({cmd, args});
			const r = h(cmd, args);
			if (r instanceof Error) throw r;
			return r;
		},
	};
	return {calls, list: createIntegrations(deps)};
};
const good: Handler = () => ({code: 0, stdout: '', stderr: ''});
const enoent: Handler = () => Object.assign(new Error(`spawn x ENOENT SECRETTOKEN123`), {code: 'ENOENT'});
const bad: Handler = () => ({code: 1, stdout: '', stderr: 'boom SECRETTOKEN123'});

{
	const {list} = mk(good);
	ok(
		'registry order',
		same(
			list.map((i) => i.id),
			['claude', 'codex', 'opencode', 'copilot'],
		),
	);
	ok('byId hit', byId(list, 'codex')?.id === 'codex');
	ok('byId miss', byId(list, 'nope') === undefined);
	ok(
		'canRegister',
		same(
			list.map((i) => i.canRegister),
			[true, true, false, true],
		),
	);
	for (const i of list) {
		const w = i.watchPrompt(ep);
		const cmd = i.registerCommand(ep);
		ok(`${i.id} watch has own poll`, w.includes(`wait_seconds=${i.pollSeconds}`));
		if (i.id === 'claude' || i.id === 'opencode')
			ok(`${i.id} watch mentions follow_up`, w.includes('follow_up=true') && w.includes('same thread_id'));
		ok(`${i.id} cmd has url+token`, cmd.includes(ep.url) && cmd.includes(ep.token));
		// shared builder for cli integrations; opencode has its own
		if (i.id === 'claude' || i.id === 'opencode') {
			ok(
				`${i.id} watch loop contract`,
				w.includes('next_question') && w.includes('answer') && /immediately/i.test(w) && w.includes('closed'),
			);
		}
	}
	const op = byId(list, 'opencode')!;
	ok(
		'opencode snippet',
		['"type": "remote"', 'xplain_*', 'opencode.json', 'xplain_next_question', '"oauth": false', '120000'].every((s) =>
			op.registerCommand(ep).includes(s),
		),
	);
	const cl = byId(list, 'claude')!.registerCommand(ep);
	ok(
		'claude notes',
		['--scope local', 'claude --resume', '--allowedTools "mcp__xplain"', 'CLAUDE_CODE_MCP_AUTO_BACKGROUND_MS=0'].every(
			(s) => cl.includes(s),
		),
	);
	const cx = byId(list, 'codex')!.registerCommand(ep);
	ok(
		'codex notes',
		[
			'export XPLAIN_MCP_TOKEN=SECRETTOKEN123',
			'--bearer-token-env-var XPLAIN_MCP_TOKEN',
			'tool_timeout_sec = 120',
			'default_tools_approval_mode = "approve"',
			'codex resume --last',
			'[mcp_servers.xplain]',
		].every((s) => cx.includes(s)),
	);
	const cp = byId(list, 'copilot')!.registerCommand(ep);
	ok(
		'copilot notes',
		['--timeout 200000', "--allow-tool='xplain'", '--additional-mcp-config', '/mcp'].every((s) => cp.includes(s)),
	);
}

const specs: {id: string; bin: string; add: string[]; rm: string[]; get: string[]}[] = [
	{
		id: 'claude',
		bin: 'claude',
		add: [
			'mcp',
			'add',
			'xplain',
			ep.url,
			'--transport',
			'http',
			'--scope',
			'local',
			'--header',
			`Authorization: Bearer ${ep.token}`,
		],
		rm: ['mcp', 'remove', 'xplain', '-s', 'local'],
		get: ['mcp', 'get', 'xplain'],
	},
	{
		id: 'codex',
		bin: 'codex',
		add: ['mcp', 'add', 'xplain', '--url', ep.url, '--bearer-token-env-var', 'XPLAIN_MCP_TOKEN'],
		rm: ['mcp', 'remove', 'xplain'],
		get: ['mcp', 'get', 'xplain', '--json'],
	},
	{
		id: 'copilot',
		bin: 'copilot',
		add: [
			'mcp',
			'add',
			'xplain',
			ep.url,
			'--transport',
			'http',
			'--header',
			`Authorization: Bearer ${ep.token}`,
			'--timeout',
			'200000',
		],
		rm: ['mcp', 'remove', 'xplain'],
		get: ['mcp', 'get', 'xplain', '--json'],
	},
];

for (const s of specs) {
	const t = (n: string) => `${s.id} ${n}`;
	{
		const m = mk(good);
		const r = await byId(m.list, s.id)!.register!(ep);
		ok(
			t('register argv (remove then add), restart, no token in msg'),
			r.ok &&
				r.needsRestart === true &&
				m.calls.length === 2 &&
				same(m.calls[0], {cmd: s.bin, args: s.rm}) &&
				same(m.calls[1], {cmd: s.bin, args: s.add}) &&
				!r.message.includes(ep.token),
		);
	}
	if (s.id !== 'codex') {
		// regression: variadic options (-H/--header) swallow trailing positionals
		ok(
			t('name+url first after mcp add, no option before them'),
			s.add[0] === 'mcp' &&
				s.add[1] === 'add' &&
				s.add[2] === 'xplain' &&
				s.add[3] === ep.url &&
				!s.add[2]!.startsWith('-') &&
				!s.add[3]!.startsWith('-'),
		);
		const m = mk(good);
		await byId(m.list, s.id)!.register!(ep);
		const a = m.calls[1]!.args;
		ok(t('real argv positionals first'), a[2] === 'xplain' && a[3] === ep.url);
		const cmd = byId(m.list, s.id)!.registerCommand(ep);
		ok(t('copy command has positionals first'), cmd.startsWith(`${s.bin} mcp add xplain ${ep.url} --`));
	}
	{
		const m = mk(good);
		const r = await byId(m.list, s.id)!.unregister!();
		ok(t('unregister argv'), r.ok && m.calls.length === 1 && same(m.calls[0], {cmd: s.bin, args: s.rm}));
	}
	{
		const m = mk(() => ({code: 0, stdout: JSON.stringify({url: ep.url}), stderr: ''}));
		const g = await byId(m.list, s.id)!.isRegistered!(ep);
		ok(t('isRegistered same url + get argv'), g.registered && !g.stale && same(m.calls[0], {cmd: s.bin, args: s.get}));
	}
}
// error handling + get parsing live in shared cliIntegration: test once (claude)
{
	const id = 'claude';
	const t = (n: string) => `${id} ${n}`;
	{
		// remove fails, add still runs and succeeds
		const m = mk((_c, a) =>
			a[1] === 'remove' ? {code: 1, stdout: '', stderr: 'not found'} : {code: 0, stdout: '', stderr: ''},
		);
		const r = await byId(m.list, id)!.register!(ep);
		ok(t('remove failure ignored'), r.ok && m.calls.length === 2);
	}
	{
		const m = mk((_c, a) =>
			a[1] === 'add' ? {code: 2, stdout: '', stderr: 'boom SECRETTOKEN123'} : {code: 0, stdout: '', stderr: ''},
		);
		const r = await byId(m.list, id)!.register!(ep);
		ok(
			t('add nonzero => fail, token masked'),
			!r.ok && r.message.includes('exit 2') && !r.message.includes(ep.token) && r.message.includes('boom'),
		);
	}
	{
		const m = mk(enoent);
		const i = byId(m.list, id)!;
		const r = await i.register!(ep);
		ok(
			t('register ENOENT, no token'),
			!r.ok && r.message === `${i.label} CLI not found` && !r.message.includes(ep.token),
		);
		const u = await i.unregister!();
		ok(t('unregister ENOENT'), !u.ok && u.message.includes('not found'));
		const g = await i.isRegistered!(ep);
		ok(t('isRegistered ENOENT'), g.registered === false);
	}
	{
		const m = mk(() => new Error('weird SECRETTOKEN123'));
		const r = await byId(m.list, id)!.register!(ep);
		ok(t('generic throw masked'), !r.ok && !r.message.includes(ep.token));
	}
	{
		const m = mk(bad);
		await byId(m.list, id)!.isRegistered!(ep);
		const r = await byId(m.list, id)!.unregister!();
		ok(t('unregister nonzero, token masked'), !r.ok && !r.message.includes(ep.token));
	}
	{
		const m = mk(() => ({code: 0, stdout: JSON.stringify({url: 'http://127.0.0.1:1/mcp'}), stderr: ''}));
		const g = await byId(m.list, id)!.isRegistered!(ep);
		ok(t('isRegistered stale'), g.registered && g.stale === true);
	}
	{
		const m = mk(() => ({code: 0, stdout: 'xplain:\n  Type: http\n  URL: http://127.0.0.1:9/mcp\n', stderr: ''}));
		const g = await byId(m.list, id)!.isRegistered!(ep);
		ok(t('text url stale'), g.registered && g.stale === true);
	}
	{
		const m = mk(() => ({code: 0, stdout: 'gibberish', stderr: ''}));
		const g = await byId(m.list, id)!.isRegistered!(ep);
		ok(t('unparseable => registered non-stale'), g.registered && !g.stale);
	}
	{
		const m = mk(bad);
		const g = await byId(m.list, id)!.isRegistered!(ep);
		ok(t('not registered on nonzero'), g.registered === false);
	}
}
{
	const m = mk(() => ({code: 0, stdout: JSON.stringify({xplain: {url: 'http://other/mcp'}}), stderr: ''}));
	const g = await byId(m.list, 'copilot')!.isRegistered!(ep);
	ok('copilot nested .xplain.url stale', g.registered && g.stale === true);
}
if (fail) {
	console.log(`${fail} FAILED`);
	process.exit(1);
}
