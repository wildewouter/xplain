import {mkdtempSync, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createAskController} from '../src/ask/index.js';
import {createMcpBridge} from '../src/mcp/bridge.js';
import {createHub, type Hub} from '../src/mcp/index.js';
import {waitFor} from './helpers.js';
import type {AgentIntegration, McpEndpoint} from '../src/integrations/index.js';

let fail = 0;
const ok = (n: string, c: boolean) => {
	console.log(c ? 'PASS' : 'FAIL', n);
	if (!c) fail++;
};
const tick = (ms = 20) => new Promise((r) => setTimeout(r, ms));
const TOKEN = 'tok-secret-0123456789abcdef';
const dirs: string[] = [];
const tmp = () => {
	const d = mkdtempSync(join(tmpdir(), 'xplain-bridge-'));
	dirs.push(d);
	return d;
};

const fakeInt = (id: string, o: Partial<AgentIntegration> & {reg?: boolean; stale?: boolean; msg?: string} = {}) => {
	const calls: string[] = [];
	const i: AgentIntegration = {
		id,
		label: id.toUpperCase(),
		needsRestart: true,
		pollSeconds: 30,
		canRegister: true,
		registerCommand: (ep: McpEndpoint) => `add ${id} ${ep.url} --token ${ep.token}`,
		watchPrompt: (ep?: McpEndpoint) => `watch ${id} ${ep?.url ?? ''}`,
		async register(ep) {
			calls.push('register');
			await tick(30);
			return {ok: true, message: o.msg ?? `done ${ep.token}`, needsRestart: true};
		},
		async unregister() {
			calls.push('unregister');
			return {ok: true, message: 'removed'};
		},
		async isRegistered() {
			calls.push('isRegistered');
			return {registered: o.reg ?? false, stale: o.stale ?? false};
		},
		...o,
	};
	return {i, calls};
};

type Fx = ReturnType<typeof mk>;
function mk(o: {failStart?: Error; ints?: AgentIntegration[]} = {}) {
	const controller = createAskController();
	let hub!: Hub;
	let started = 0;
	let stopped = 0;
	const copied: string[] = [];
	const bridge = createMcpBridge({
		controller,
		stateDir: '/nonexistent',
		port: 1234,
		integrations: o.ints ?? [],
		createHub: () => (hub = createHub()),
		loadToken: () => ({token: TOKEN}),
		createServer: () => ({
			async start() {
				if (o.failStart) throw o.failStart;
				started++;
				return {url: 'http://127.0.0.1:1234/mcp', port: 1234};
			},
			async stop() {
				stopped++;
			},
		}),
		clipboard: (t) => void copied.push(t),
	});
	return {controller, bridge, hub: () => hub, started: () => started, stopped: () => stopped, copied};
}
const addQ = (f: Fx, message = 'why?') =>
	f.controller.add({
		file: 'a.ts',
		index: 3,
		side: 'new',
		line: 7,
		text: 'const x = 1;',
		message,
		context: ['ctx1', 'ctx2'],
	}).id!;

// start / stop / dispose
{
	const f = mk();
	let n = 0;
	f.bridge.subscribe(() => n++);
	await f.bridge.start();
	const s = f.bridge.getState();
	ok('start: running + url + port', s.running && s.url === 'http://127.0.0.1:1234/mcp' && s.port === 1234);
	ok('start: masked token only', s.tokenMasked !== undefined && !JSON.stringify(s).includes(TOKEN));
	ok('subscribe notified', n > 0);
	await f.bridge.start();
	ok('double start noop', f.started() === 1);
	await f.bridge.stop();
	ok(
		'stop: off + server stopped + hub closed',
		!f.bridge.getState().running && f.stopped() === 1 && !f.hub().status().running,
	);
	await f.bridge.stop();
	await f.bridge.start();
	ok('restart works', f.bridge.getState().running && f.started() === 2);
	f.bridge.dispose();
	f.bridge.dispose();
	await waitFor(() => f.stopped() === 2);
	ok('dispose idempotent, stopped', !f.bridge.getState().running && f.stopped() === 2);
	await f.bridge.start();
	ok('start after dispose ignored', !f.bridge.getState().running);
}

// port in use
{
	const f = mk({failStart: Object.assign(new Error('listen EADDRINUSE'), {code: 'EADDRINUSE'})});
	await f.bridge.start();
	const s = f.bridge.getState();
	ok('port in use: error state', !s.running && !s.starting && /1234 in use/.test(s.error ?? ''));
	const g = mk({failStart: new Error('boom')});
	await g.bridge.start();
	ok('other error surfaced', g.bridge.getState().error === 'boom');
}

// ask sets pending before enqueue (delivery may be synchronous)
{
	const f = mk();
	const id = addQ(f);
	await f.bridge.start();
	const h = f.hub();
	const orig = h.enqueue.bind(h);
	let seen: string | undefined;
	h.enqueue = (x) => {
		seen = f.controller.answer(id)?.status;
		return orig(x);
	};
	f.bridge.ask(f.controller.getState().questions[0]!);
	ok('ask: pending already set when enqueued', seen === 'pending');
}

// ask
{
	const f = mk();
	const id = addQ(f);
	const q = f.controller.getState().questions[0]!;
	ok('ask while off returns false', f.bridge.ask(q) === false && f.controller.answer(id) === undefined);
	await f.bridge.start();
	f.bridge.ask(q);
	const p = f.hub().pending();
	ok(
		'enqueued threadId/turn/message',
		p.length === 1 && p[0]!.threadId === id && p[0]!.turn === 1 && p[0]!.message === 'why?',
	);
	ok(
		'enqueued context block',
		/File: a\.ts/.test(p[0]!.context ?? '') &&
			/Lines: 7/.test(p[0]!.context ?? '') &&
			/const x = 1;/.test(p[0]!.context ?? '') &&
			/ctx2/.test(p[0]!.context ?? ''),
	);
	ok('pending answer set', f.controller.answer(id)?.status === 'pending');
	// delivered -> streaming
	f.hub().registerClient('c1', 'my-agent', '1.2');
	const got = await f.hub().poll('c1', 100);
	ok('poll got question', typeof got === 'object' && got !== null && got.threadId === id);
	ok(
		'streaming with agent name',
		f.controller.answer(id)?.status === 'streaming' && f.controller.answer(id)?.agent === 'my-agent',
	);
	ok(
		'state delivered + client listed',
		f.bridge.getState().delivered === 1 && f.bridge.getState().clients[0]?.name === 'my-agent',
	);
	f.hub().answer(id, 'because');
	const a = f.controller.answer(id);
	ok('answer -> done with agent', a?.status === 'done' && a.text === 'because' && a.agent === 'my-agent');
	ok('unknown thread answer ignored', (f.hub().answer('zzz', 'x'), f.controller.getState().questions.length === 1));
	// stop cancels pending
	const id2 = addQ(f, 'second');
	f.bridge.ask(f.controller.getState().questions.find((x) => x.id === id2)!);
	await f.bridge.stop();
	const c = f.controller.answer(id2);
	ok('stop cancels live', c?.status === 'cancelled' && c.text === 'MCP stopped');
	ok('stop keeps done', f.controller.answer(id)?.status === 'done');
	f.bridge.dispose();
}

// files_changed
{
	const f = mk();
	await f.bridge.start();
	let got: string[] | undefined;
	f.bridge.onFilesChanged((p) => (got = p));
	f.hub().filesChanged(['x.ts']);
	ok('files_changed notifies', got?.join() === 'x.ts');
	f.bridge.dispose();
}

// annotate
{
	const f = mk();
	await f.bridge.start();
	f.hub().annotate({file: 'b.ts', line: 9, text: 'note!', side: 'old'});
	const q = f.controller.getState().questions[0];
	ok(
		'annotate adds agent comment',
		q?.origin === 'agent' &&
			q.file === 'b.ts' &&
			q.line === 9 &&
			q.side === 'old' &&
			q.message === 'note!' &&
			q.index === 0,
	);
	f.bridge.dispose();
}

// registration
{
	const a = fakeInt('aa', {reg: true});
	const b = fakeInt('bb', {reg: true, stale: true});
	const c = fakeInt('cc', {canRegister: false, isRegistered: undefined, register: undefined, unregister: undefined});
	const f = mk({ints: [a.i, b.i, c.i]});
	const st = () => f.bridge.getState().integrations;
	ok(
		'integrations listed generically',
		st()
			.map((x) => x.label)
			.join() === 'AA,BB,CC' && st()[2]!.canRegister === false,
	);
	ok('registered unknown while off', st()[0]!.registered === undefined);
	ok('commandText null when off', f.bridge.commandText('aa') === null && f.bridge.watchText('aa') === null);
	await f.bridge.register('aa');
	ok('register while off -> note', st()[0]!.note === 'start MCP first');
	await f.bridge.start();
	await waitFor(() => st()[1]!.stale === true);
	ok('refresh statuses', st()[0]!.registered === true && st()[0]!.stale === false && st()[1]!.stale === true);
	ok('no-register integration untouched', st()[2]!.registered === undefined);
	const p = f.bridge.register('aa');
	await waitFor(() => st()[0]!.busy === true);
	ok('busy while registering', st()[0]!.busy === true);
	await p;
	ok(
		'register note masked + restart hint',
		st()[0]!.busy === false &&
			/done \*\*\*/.test(st()[0]!.note ?? '') &&
			/restart the agent session, then paste the watch prompt/.test(st()[0]!.note ?? ''),
	);
	await f.bridge.unregister('aa');
	ok('unregister note', st()[0]!.note === 'removed' && a.calls.includes('unregister'));
	await f.bridge.register('cc');
	ok('copy-paste only note', st()[2]!.note === 'copy-paste only');
	const cmd = f.bridge.commandText('aa')!;
	ok('commandText uses live endpoint', cmd.includes('http://127.0.0.1:1234/mcp') && cmd.includes(TOKEN));
	ok('commandText masked', !f.bridge.commandText('aa', true)!.includes(TOKEN));
	ok('watchText live', f.bridge.watchText('aa') === 'watch aa http://127.0.0.1:1234/mcp');
	f.bridge.copy(cmd);
	ok('copy uses clipboard', f.copied.length === 1 && f.copied[0] === cmd);
	f.bridge.dispose();
}
{
	const a = fakeInt('aa', {isRegistered: async () => Promise.reject(new Error(`bad ${TOKEN}`))});
	const f = mk({ints: [a.i]});
	await f.bridge.start();
	await waitFor(() => !!f.bridge.getState().integrations[0]!.note);
	const n = f.bridge.getState().integrations[0]!.note ?? '';
	ok('refresh error -> masked note', /bad \*\*\*/.test(n) && !n.includes(TOKEN));
	f.bridge.dispose();
}

// follow-up (fake hub/server)
{
	const f = mk();
	const id = addQ(f);
	ok('followUp requires running', !f.bridge.followUp(id, 'x') && f.controller.turns(id).length === 1);
	await f.bridge.start();
	f.bridge.ask(f.controller.getState().questions[0]!);
	ok('followUp refused while live', !f.bridge.followUp(id, 'x') && f.controller.turns(id).length === 1);
	const r1 = await f.hub().poll('c1', 100);
	ok('turn 1 not follow-up', typeof r1 === 'object' && r1 !== null && !r1.followUp && r1.turn === 1);
	f.hub().answer(id, 'A1');
	ok('turn 1 done', f.controller.answer(id)?.status === 'done');
	ok('followUp accepted', f.bridge.followUp(id, 'Q2'));
	ok(
		'followUp pending on turn 2 only',
		f.controller.turns(id)[1]!.answer?.status === 'pending' && f.controller.turns(id)[0]!.answer?.text === 'A1',
	);
	const pend = f.hub().pending()[0]!;
	ok(
		'followUp enqueued with history',
		pend.followUp === true &&
			pend.turn === 2 &&
			pend.threadId === id &&
			pend.message === 'Q2' &&
			pend.context === undefined &&
			JSON.stringify(pend.history) === JSON.stringify([{turn: 1, message: 'why?', answer: 'A1'}]),
	);
	const r2 = await f.hub().poll('c1', 100);
	ok(
		'followUp delivered -> streaming turn 2',
		typeof r2 === 'object' && f.controller.answer(id)?.status === 'streaming',
	);
	f.hub().answer(id, 'A2');
	const th = f.controller.turns(id);
	ok('answer maps to turn 2', th[1]!.answer?.text === 'A2' && th[0]!.answer?.text === 'A1');
	ok('followUp unknown thread', !f.bridge.followUp('zz', 'x'));
	// live turn 3 cancelled on stop
	ok('turn 3', f.bridge.followUp(id, 'Q3'));
	await f.bridge.stop();
	const th2 = f.controller.turns(id);
	ok('stop cancels live turn only', th2[2]!.answer?.status === 'cancelled' && th2[1]!.answer?.status === 'done');
	f.bridge.dispose();
}

// END-TO-END: real hub + real server on port 0
{
	const dir = tmp();
	const controller = createAskController();
	const bridge = createMcpBridge({controller, stateDir: dir, port: 0, integrations: []});
	await bridge.start();
	const s = bridge.getState();
	ok('e2e started', s.running && !!s.url && (s.port ?? 0) > 0);
	// real token, read back from file
	const {readMcpConfig} = await import('../src/mcp/index.js');
	const token = readMcpConfig(dir)!.token;
	ok('e2e token not in state', !JSON.stringify(bridge.getState()).includes(token));
	const H = {
		authorization: `Bearer ${token}`,
		'content-type': 'application/json',
		accept: 'application/json, text/event-stream',
	};
	let rid = 1;
	let sid: string | undefined;
	const rpc = async (method: string, params: unknown) => {
		const r = await fetch(s.url!, {
			method: 'POST',
			headers: {...H, ...(sid ? {'mcp-session-id': sid} : {})},
			body: JSON.stringify({jsonrpc: '2.0', id: rid++, method, params}),
		});
		sid ??= r.headers.get('mcp-session-id') ?? undefined;
		const t = await r.text();
		const j =
			t.startsWith('event:') || t.startsWith('data:')
				? t
						.split('\n')
						.find((l) => l.startsWith('data:'))!
						.slice(5)
				: t;
		return JSON.parse(j) as {result?: {content?: {text: string}[]}};
	};
	await rpc('initialize', {
		protocolVersion: '2025-06-18',
		capabilities: {},
		clientInfo: {name: 'fake-agent', version: '9'},
	});
	const id = controller.add({file: 'e.ts', index: 0, side: 'new', line: 2, text: 'x', message: 'explain?'}).id!;
	bridge.ask(controller.getState().questions[0]!);
	const nq = await rpc('tools/call', {name: 'next_question', arguments: {wait_seconds: 2}});
	const txt = nq.result?.content?.[0]?.text ?? '';
	ok('e2e agent receives question', txt.includes('explain?') && txt.includes(id));
	await rpc('tools/call', {name: 'answer', arguments: {thread_id: id, text: 'the answer'}});
	const a = controller.answer(id);
	ok('e2e answer done', a?.status === 'done' && a.text === 'the answer' && a.agent === 'fake-agent');
	const t1 = JSON.parse(txt);
	ok('e2e turn 1 follow_up false, no previous', t1.turn === 1 && t1.follow_up === false && !('previous' in t1));
	ok('e2e followUp accepted', bridge.followUp(id, 'and why so?'));
	const nq2 = await rpc('tools/call', {name: 'next_question', arguments: {wait_seconds: 2}});
	const t2 = JSON.parse(nq2.result?.content?.[0]?.text ?? '{}');
	ok(
		'e2e turn 2 follow-up fields',
		t2.turn === 2 &&
			t2.follow_up === true &&
			t2.thread_id === id &&
			t2.previous?.length === 1 &&
			t2.previous[0].turn === 1 &&
			t2.previous[0].question === 'explain?' &&
			t2.previous[0].answer === 'the answer',
	);
	ok('e2e turn 2 text prefix', t2.question === `Follow-up to your earlier answer (thread ${id}, turn 2): and why so?`);
	await rpc('tools/call', {name: 'answer', arguments: {thread_id: id, text: 'second answer'}});
	const th = controller.turns(id);
	ok(
		'e2e thread turn 2 done',
		th.length === 2 &&
			th[0]!.answer?.text === 'the answer' &&
			th[1]!.answer?.status === 'done' &&
			th[1]!.answer.text === 'second answer' &&
			controller.answer(id)?.text === 'second answer',
	);
	await bridge.stop();
	bridge.dispose();
}

for (const d of dirs) rmSync(d, {recursive: true, force: true});
process.exit(fail ? 1 : 0);
