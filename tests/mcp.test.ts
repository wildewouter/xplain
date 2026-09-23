import {existsSync, mkdirSync, mkdtempSync, rmSync, statSync, readFileSync} from 'node:fs';
import {join} from 'node:path';
import {createServer} from 'node:net';
import {waitFor} from './helpers.js';
import {
	createHub,
	sanitize,
	createMcpServer,
	loadOrCreateToken,
	rotateToken,
	callTool,
	TOOLS,
	SERVER_INSTRUCTIONS,
	type HubEvent,
} from '../src/mcp/index.js';

let fail = 0;
const ok = (n: string, c: boolean) => {
	console.log(c ? 'PASS' : 'FAIL', n);
	if (!c) fail++;
};
const base = join(process.env.TMPDIR ?? '/tmp', 'tmp');
mkdirSync(base, {recursive: true});
const made: string[] = [];
const tmp = () => {
	const d = mkdtempSync(join(base, 'mcp-'));
	made.push(d);
	return d;
};

// fake timers
const fake = () => {
	let now = 0;
	const q: {at: number; fn: () => void; id: number}[] = [];
	let n = 0;
	return {
		timers: {
			now: () => now,
			setTimeout: (fn: () => void, ms: number) => {
				const id = ++n;
				q.push({at: now + ms, fn, id});
				return id;
			},
			clearTimeout: (h: unknown) => {
				const i = q.findIndex((x) => x.id === h);
				if (i >= 0) q.splice(i, 1);
			},
		},
		advance(ms: number) {
			now += ms;
			for (const x of q.filter((x) => x.at <= now)) {
				q.splice(q.indexOf(x), 1);
				x.fn();
			}
		},
		count: () => q.length,
	};
};
const Q = (threadId: string, turn = 1, message = 'm-' + threadId) => ({threadId, turn, message});
const tick = () => new Promise((r) => setImmediate(r));

// ---- hub
{
	const h = createHub();
	h.enqueue(Q('a'));
	h.enqueue(Q('b'));
	const r1 = await h.poll('c1', 1000);
	const r2 = await h.poll('c1', 1000);
	ok(
		'hub: fifo order',
		typeof r1 === 'object' && r1 !== null && r1.threadId === 'a' && typeof r2 === 'object' && r2?.threadId === 'b',
	);
	ok('hub: delivered count', h.status().delivered === 2);
}
{
	const h = createHub();
	const p = h.poll('c1', 1000);
	ok('hub: polling flagged', h.status().clients.find((c) => c.id === 'c1')?.polling === true);
	h.enqueue(Q('x'));
	const r = await p;
	ok('hub: waiting poll resolves on enqueue', typeof r === 'object' && r !== null && r.threadId === 'x');
	ok('hub: polling cleared', h.status().clients.find((c) => c.id === 'c1')?.polling === false);
}
{
	const f = fake();
	const h = createHub({timers: f.timers});
	const p = h.poll('c1', 5000);
	f.advance(4999);
	let done = false;
	void p.then(() => (done = true));
	await tick();
	ok('hub: not timed out early', !done);
	f.advance(1);
	ok('hub: timeout null', (await p) === null);
	ok('hub: timer cleaned', f.count() === 0);
	ok('hub: lastSeen updated', h.status().clients[0]!.lastSeen === 5000);
}
{
	const h = createHub();
	const p1 = h.poll('c1', 10000);
	const p2 = h.poll('c1', 10000);
	ok('hub: supersede older null', (await p1) === null);
	h.enqueue(Q('s'));
	const r = await p2;
	ok('hub: newer gets question', typeof r === 'object' && r !== null && r.threadId === 's');
	ok('hub: supersede consumed nothing extra', h.status().delivered === 1);
}
{
	const h = createHub();
	h.enqueue(Q('a'));
	h.enqueue(Q('b'));
	const r = (await h.poll('c1', 100)) as {threadId: string; turn: number; message: string};
	h.requeue(r);
	ok('hub: requeue front', h.pending()[0]!.threadId === 'a' && h.pending().length === 2);
	ok('hub: requeue delivered dec', h.status().delivered === 0);
	const r2 = await h.poll('c2', 100);
	ok('hub: requeued redelivered first', typeof r2 === 'object' && r2 !== null && r2.threadId === 'a');
}
{
	// stickiness
	const h = createHub();
	h.enqueue(Q('t', 1));
	await h.poll('c1', 100);
	const pa = h.poll('c1', 5000); // c1 polling
	const pb = h.poll('c2', 5000);
	h.enqueue(Q('t', 2));
	const r = await pa;
	ok('hub: sticky client gets follow-up', typeof r === 'object' && r !== null && r.threadId === 't' && r.turn === 2);
	let bDone = false;
	void pb.then(() => (bDone = true));
	await tick();
	ok('hub: other poller keeps waiting', !bDone);
	h.close();
	ok('hub: close -> closed', (await pb) === 'closed');
}
{
	const h = createHub();
	h.enqueue(Q('t', 1));
	await h.poll('c1', 100); // c1 sticky, not polling now
	h.enqueue(Q('t', 2));
	const r = await h.poll('c2', 100);
	ok('hub: sticky fallback to any poller', typeof r === 'object' && r !== null && r.turn === 2);
}
{
	// sanitize
	ok('sanitize: csi', sanitize('\x1b[31mred\x1b[0m') === 'red');
	ok('sanitize: osc bel', sanitize('x\x1b]0;title\x07y') === 'xy');
	ok('sanitize: osc st', sanitize('x\x1b]8;;http://a\x1b\\y') === 'xy');
	ok('sanitize: ctrl', sanitize('a\x00b\x07c\x08d\re\x7ff') === 'abcdef');
	ok('sanitize: keep nl tab', sanitize('a\nb\tc') === 'a\nb\tc');
	ok('sanitize: c1', sanitize('a\u009bb') === 'ab');
	ok('sanitize: lone esc', !sanitize('a\x1bb').includes('\x1b'));
	ok('sanitize: cap', sanitize('x'.repeat(30000)).length === 20000);
	ok('sanitize: non-string', sanitize(undefined) === '');
}
{
	const h = createHub();
	const ev: HubEvent[] = [];
	const un = h.subscribe((e) => ev.push(e));
	h.registerClient('c1', 'tool', '1.2');
	h.enqueue(Q('a', 3));
	await h.poll('c1', 100);
	ok(
		'hub: delivered event',
		ev.some((e) => e.type === 'delivered' && e.threadId === 'a' && e.clientId === 'c1'),
	);
	ok('hub: answer ok', h.answer('a', '\x1b[1mhi\x1b[0m') === true);
	const a = ev.find((e) => e.type === 'answer') as Extract<HubEvent, {type: 'answer'}>;
	ok('hub: answer event sanitized', a?.text === 'hi' && a.turn === 3 && a.threadId === 'a');
	ok('hub: answer unknown false', h.answer('zzz', 'x') === false);
	h.annotate({file: 'f\x1b[1m.ts', line: 4, text: 'n\x00ote', side: 'old'});
	const an = ev.find((e) => e.type === 'annotate') as Extract<HubEvent, {type: 'annotate'}>;
	ok('hub: annotate event', an?.file === 'f.ts' && an.line === 4 && an.text === 'note' && an.side === 'old');
	const st = h.status();
	ok('hub: status client info', st.clients[0]!.name === 'tool' && st.clients[0]!.version === '1.2' && st.running);
	un();
	const n = ev.length;
	h.enqueue(Q('q'));
	ok('hub: unsubscribe', ev.length === n);
	h.close();
	ok('hub: status not running', !h.status().running);
	ok('hub: poll after close', (await h.poll('c1', 10)) === 'closed');
	const h2 = createHub();
	h2.subscribe(() => {
		throw new Error('boom');
	});
	h2.enqueue(Q('z'));
	ok('hub: listener throw isolated', h2.pending().length === 1);
}
{
	const h = createHub();
	const ac = new AbortController();
	const p = h.poll('c1', 10000, ac.signal);
	ac.abort();
	ok('hub: abort -> null', (await p) === null);
	h.enqueue(Q('k'));
	ok('hub: abort consumed nothing', h.pending().length === 1);
	ok('hub: pre-aborted null', (await h.poll('c1', 10, ac.signal)) === null && h.pending().length === 1);
}

// ---- tools
const parse = (r: {content: {text: string}[]}) => JSON.parse(r.content[0]!.text);
{
	const h = createHub();
	ok(
		'tools: no prefix',
		TOOLS.every((t) => !t.name.includes('__') && !t.name.includes('xplain')),
	);
	ok(
		'tools: loop hint',
		TOOLS.find((t) => t.name === 'next_question')!.description.includes('call next_question again immediately'),
	);
	h.enqueue({threadId: 't1', turn: 2, message: 'why?', context: 'CTX'});
	const r = parse(await callTool('next_question', {wait_seconds: 1}, {hub: h, clientId: 'c'}));
	ok('tools: question shape', r.status === 'question' && r.thread_id === 't1' && r.turn === 2);
	ok('tools: question has context', r.question === 'why?\n\nCTX');
	const t0 = Date.now();
	const e = parse(await callTool('next_question', {wait_seconds: 1}, {hub: h, clientId: 'c'}));
	ok(
		'tools: no_question_yet',
		e.status === 'no_question_yet' &&
			e.call_again === true &&
			e.note === 'No question yet. Call next_question again immediately.',
	);
	ok('tools: waited ~1s', Date.now() - t0 >= 900);
	const f = fake();
	const h2 = createHub({timers: f.timers});
	let ms = -1;
	const orig = h2.poll.bind(h2);
	h2.poll = (id, w, s) => ((ms = w), orig(id, w, s));
	void callTool('next_question', {wait_seconds: 9999}, {hub: h2, clientId: 'c'});
	ok('tools: clamp max', ms === 120000);
	void callTool('next_question', {wait_seconds: -5}, {hub: h2, clientId: 'd'});
	ok('tools: clamp min', ms === 1000);
	void callTool('next_question', {}, {hub: h2, clientId: 'e'});
	ok('tools: default 45', ms === 45000);
	h2.close();
	const a = await callTool('answer', {thread_id: 't1', text: 'yes'}, {hub: h, clientId: 'c'});
	ok(
		'tools: answer ok',
		parse(a).ok === true && parse(a).note === 'Answer delivered. Call next_question again immediately.' && !a.isError,
	);
	const bad = await callTool('answer', {thread_id: 'nope', text: 'yes'}, {hub: h, clientId: 'c'});
	ok('tools: unknown thread isError', bad.isError === true && bad.content[0]!.text.includes('Unknown thread'));
	ok('tools: answer bad args', (await callTool('answer', {}, {hub: h, clientId: 'c'})).isError === true);
	h.enqueue(Q('p1', 1, 'x'.repeat(500)));
	const g = parse(await callTool('get_questions', {}, {hub: h, clientId: 'c'}));
	ok(
		'tools: get_questions list',
		g.questions.length === 1 && g.questions[0].thread_id === 'p1' && g.questions[0].preview.length <= 200,
	);
	ok('tools: get_questions no consume', h.pending().length === 1);
	const evs: HubEvent[] = [];
	h.subscribe((x) => evs.push(x));
	const an = await callTool('annotate', {file: 'a.ts', line: 3, text: 'hey'}, {hub: h, clientId: 'c'});
	ok('tools: annotate ok', parse(an).ok === true && evs.some((x) => x.type === 'annotate'));
	await callTool('annotate', {file: 'a.ts', line: 3, text: 'n', number: 4}, {hub: h, clientId: 'c'});
	await callTool('annotate', {file: 'a.ts', line: 3, text: 'n', number: 1.5}, {hub: h, clientId: 'c'});
	const nums = evs.flatMap((x) => (x.type === 'annotate' ? [x.number] : []));
	ok('tools: annotate number', nums.join() === ',4,');
	const fc = await callTool('files_changed', {paths: ['a.ts', 1]}, {hub: h, clientId: 'c'});
	ok(
		'tools: files_changed ok',
		parse(fc).ok === true && evs.some((x) => x.type === 'files_changed' && x.paths.join() === 'a.ts'),
	);
	ok(
		'tools: files_changed described as required + server instructions',
		/MUST/.test(TOOLS.find((t) => t.name === 'files_changed')?.description ?? '') &&
			SERVER_INSTRUCTIONS.includes('files_changed'),
	);
	ok('tools: annotate bad', (await callTool('annotate', {file: 'a'}, {hub: h, clientId: 'c'})).isError === true);
	ok('tools: unknown tool', (await callTool('zzz', {}, {hub: h, clientId: 'c'})).isError === true);
	h.close();
	const cl = parse(await callTool('next_question', {}, {hub: h, clientId: 'c'}));
	ok('tools: closed', cl.status === 'closed' && cl.note === 'xplain closed the session. Stop.');
}

// ---- token
{
	const d = join(tmp(), 'state');
	const a = loadOrCreateToken(d);
	const file = join(d, 'mcp.json');
	ok('token: created', existsSync(file) && a.token.length >= 43);
	ok('token: file mode 0600', (statSync(file).mode & 0o777) === 0o600);
	ok('token: dir mode 0700', (statSync(d).mode & 0o777) === 0o700);
	ok('token: stable', loadOrCreateToken(d).token === a.token);
	const r = rotateToken(d);
	ok('token: rotate changes', r.token !== a.token && loadOrCreateToken(d).token === r.token);
	ok('token: rotate mode', (statSync(file).mode & 0o777) === 0o600);
}

// ---- server
const h = createHub();
const dir = tmp();
const tok = loadOrCreateToken(dir).token;
const srv = createMcpServer({hub: h, port: 0, token: tok, dir});
const {url, port} = await srv.start();
ok('server: url', url === `http://127.0.0.1:${port}/mcp` && port > 0);
ok('server: port persisted', JSON.parse(readFileSync(join(dir, 'mcp.json'), 'utf8')).port === port);
const post = (body: unknown, headers: Record<string, string> = {}, signal?: AbortSignal) =>
	fetch(url, {
		method: 'POST',
		headers: {'content-type': 'application/json', authorization: `Bearer ${tok}`, ...headers},
		body: JSON.stringify(body),
		...(signal ? {signal} : {}),
	});
const rpc = (method: string, params?: unknown, id = 1, headers: Record<string, string> = {}) =>
	post({jsonrpc: '2.0', id, method, params}, headers);
const call = async (name: string, args: unknown, headers: Record<string, string> = {}) => {
	const j = (await (await rpc('tools/call', {name, arguments: args}, 2, headers)).json()) as {
		result: {content: {text: string}[]; isError?: boolean};
	};
	return {...j.result, json: () => JSON.parse(j.result.content[0]!.text)};
};

const init = await rpc('initialize', {
	protocolVersion: '2024-11-05',
	capabilities: {},
	clientInfo: {name: 'some-agent', version: '9.9'},
});
const ij = (await init.json()) as {
	result: {protocolVersion: string; capabilities: {tools: {listChanged: boolean}}; serverInfo: {name: string}};
	id: number;
};
const sid = init.headers.get('mcp-session-id') ?? '';
ok('server: init echoes version', init.status === 200 && ij.id === 1 && ij.result.protocolVersion === '2024-11-05');
ok('server: init caps', ij.result.capabilities.tools.listChanged === false && ij.result.serverInfo.name === 'xplain');
ok('server: session id', sid.length > 10);
ok(
	'server: client recorded',
	h.status().clients.some((c) => c.id === sid && c.name === 'some-agent' && c.version === '9.9'),
);
const init2 = await rpc('initialize', {protocolVersion: '1999-01-01', clientInfo: {name: 'x'}});
ok(
	'server: unknown version -> newest',
	((await init2.json()) as {result: {protocolVersion: string}}).result.protocolVersion === '2025-06-18',
);
const ni = await post({jsonrpc: '2.0', method: 'notifications/initialized'});
ok('server: initialized 202', ni.status === 202 && (await ni.text()) === '');
const pg = (await (await rpc('ping')).json()) as {result: object};
ok('server: ping', Object.keys(pg.result).length === 0);
const tl = (await (await rpc('tools/list')).json()) as {result: {tools: {name: string}[]}};
ok(
	'server: tools/list',
	tl.result.tools.map((t) => t.name).join(',') === 'next_question,answer,get_questions,annotate,files_changed',
);
const um = await rpc('does/not/exist');
const umj = (await um.json()) as {error: {code: number}};
ok('server: unknown method -32601', umj.error.code === -32601);
const noauth = await fetch(url, {method: 'POST', body: '{}'});
ok('server: 401 no token', noauth.status === 401);
await noauth.text();
const badtok = await fetch(url, {method: 'POST', headers: {authorization: 'Bearer wrong'}, body: '{}'});
ok('server: 401 wrong token', badtok.status === 401);
await badtok.text();
const org = await rpc('ping', undefined, 1, {origin: 'http://evil.example'});
ok('server: Origin 403', org.status === 403);
await org.text();
const orgOk = await rpc('ping', undefined, 1, {origin: 'http://localhost:3000'});
ok('server: loopback Origin ok', orgOk.status === 200);
await orgOk.text();
const get = await fetch(url, {headers: {authorization: `Bearer ${tok}`}});
ok('server: GET 405 (non-POST)', get.status === 405);
await get.text();
const nf = await fetch(url.replace('/mcp', '/other'), {method: 'POST', headers: {authorization: `Bearer ${tok}`}});
ok('server: other path 404', nf.status === 404);
await nf.text();
const bad = await post('x'.repeat(10)).then((r) => r);
ok('server: non-object -> invalid request', ((await bad.json()) as {error: {code: number}}).error.code === -32600);
const pe = await fetch(url, {method: 'POST', headers: {authorization: `Bearer ${tok}`}, body: '{nope'});
ok('server: parse error 400', pe.status === 400);
await pe.text();
const big = await fetch(url, {
	method: 'POST',
	headers: {authorization: `Bearer ${tok}`},
	body: 'x'.repeat(1024 * 1024 + 10),
}).catch(() => null);
ok('server: body cap', big === null || big.status === 413);
await big?.text().catch(() => '');
const batch = (await (
	await post([
		{jsonrpc: '2.0', id: 1, method: 'ping'},
		{jsonrpc: '2.0', id: 2, method: 'tools/list'},
	])
).json()) as {id: number}[];
ok('server: batch', Array.isArray(batch) && batch.length === 2 && batch[1]!.id === 2);
const ut = await rpc('tools/call', {name: 'nope', arguments: {}});
ok('server: unknown tool rpc error', ((await ut.json()) as {error: {code: number}}).error.code === -32602);

// tools/call
h.enqueue({threadId: 'S1', turn: 1, message: 'what is this?', context: 'file a.ts'});
await call('next_question', {wait_seconds: 1}, {'mcp-session-id': sid}); // deliver S1 so it can be answered
const t0 = Date.now();
const lp = call('next_question', {wait_seconds: 10}, {'mcp-session-id': sid});
await waitFor(() => h.status().clients.some((c) => c.polling));
h.enqueue({threadId: 'S2', turn: 1, message: 'later'});
const lr = await lp;
ok('server: long-poll resolves on enqueue', lr.json().thread_id === 'S2' && Date.now() - t0 < 5000);
const evs: HubEvent[] = [];
h.subscribe((e) => evs.push(e));
const ans = await call('answer', {thread_id: 'S1', text: 'it is a file'});
ok(
	'server: answer over rpc -> hub answer event',
	ans.json().ok === true && evs.some((e) => e.type === 'answer' && e.threadId === 'S1' && e.text === 'it is a file'),
);
// supersede via server
const sp1 = call('next_question', {wait_seconds: 20}, {'mcp-session-id': 'dup'});
await waitFor(() => h.status().clients.find((c) => c.id === 'dup')?.polling === true);
const sp2 = call('next_question', {wait_seconds: 20}, {'mcp-session-id': 'dup'});
ok('server: superseded poll returns no_question_yet fast', (await sp1).json().status === 'no_question_yet');
h.enqueue({threadId: 'S3', turn: 1, message: 'third'});
ok('server: newer poll gets it', (await sp2).json().thread_id === 'S3');

// abort mid-poll
{
	const ac = new AbortController();
	const p = post(
		{jsonrpc: '2.0', id: 5, method: 'tools/call', params: {name: 'next_question', arguments: {wait_seconds: 30}}},
		{'mcp-session-id': 'ab'},
		ac.signal,
	).catch(() => 'aborted');
	await waitFor(() => h.status().clients.find((c) => c.id === 'ab')?.polling === true);
	ok('server: abort: poller registered', h.status().clients.find((c) => c.id === 'ab')?.polling === true);
	ac.abort();
	await p;
	await waitFor(() => h.status().clients.find((c) => c.id === 'ab')?.polling === false);
	ok('server: abort: poller removed', h.status().clients.find((c) => c.id === 'ab')?.polling === false);
	h.enqueue({threadId: 'S4', turn: 1, message: 'kept'});
	await waitFor(() => h.pending().length === 1);
	ok('server: abort: question requeued/not consumed', h.pending().length === 1 && h.pending()[0]!.threadId === 'S4');
}

// close
{
	const cp = call('next_question', {wait_seconds: 20}, {'mcp-session-id': 'cl'});
	await waitFor(() => h.status().clients.find((c) => c.id === 'cl')?.polling === true);
	h.close();
	await cp;
}

// port in use
{
	const blocker = createServer();
	await new Promise<void>((r) => blocker.listen(0, '127.0.0.1', () => r()));
	const bp = (blocker.address() as {port: number}).port;
	const s2 = createMcpServer({hub: createHub(), port: bp, token: 't'});
	const err = await s2.start().then(
		() => null,
		(e: Error) => e,
	);
	ok('server: port in use rejects', err !== null && err.message.includes(String(bp)) && err.message.includes('in use'));
	await new Promise<void>((r) => blocker.close(() => r()));
}

await srv.stop();
const after = await fetch(url, {method: 'POST', body: '{}'}).then(
	() => 'up',
	() => 'down',
);
ok('server: stop closes', after === 'down');
await srv.stop(); // idempotent

// ---- follow-ups
{
	const h = createHub();
	const dirty = '\x1b[31mred\x1b[0m';
	const hist = Array.from({length: 7}, (_, i) => ({
		turn: i + 1,
		message: `m${i + 1}${dirty}`,
		answer: 'x'.repeat(5000),
	}));
	h.enqueue({threadId: 'f1', turn: 8, message: 'q8', followUp: true, history: hist});
	const p0 = h.pending()[0]!;
	ok(
		'hub: history capped to last 5',
		p0.history?.length === 5 && p0.history[0]!.turn === 3 && p0.history[4]!.turn === 7,
	);
	ok('hub: history sanitized + capped', p0.history![0]!.message === 'm3red' && p0.history![0]!.answer!.length === 4000);
	ok('hub: followUp flag kept', p0.followUp === true);
	const evs: HubEvent[] = [];
	h.subscribe((e) => evs.push(e));
	const got = parse(await callTool('next_question', {wait_seconds: 1}, {hub: h, clientId: 'a'}));
	ok('tools: follow-up fields', got.follow_up === true && got.turn === 8 && got.previous.length === 5);
	ok('tools: previous shape', got.previous[0].turn === 3 && got.previous[0].question === 'm3red');
	ok('tools: follow-up prefix', got.question === 'Follow-up to your earlier answer (thread f1, turn 8): q8');
	h.answer('f1', 'ans');
	ok(
		'hub: answer event carries turn',
		evs.some((e) => e.type === 'answer' && e.threadId === 'f1' && e.turn === 8),
	);
	// stickiness: follow-up prefers same client, but other poller may take it when owner is not polling
	h.enqueue({threadId: 'f1', turn: 9, message: 'q9', followUp: true, history: []});
	const other = h.poll('b', 50);
	const gb = await other;
	ok('hub: follow-up goes to other poller when owner not polling', typeof gb === 'object' && gb?.turn === 9);
	h.enqueue({threadId: 'f1', turn: 10, message: 'q10', followUp: true, history: []});
	const pa = h.poll('b', 50); // b owns f1 now and polls -> gets it
	const ga = await pa;
	ok('hub: follow-up sticky to delivering client', typeof ga === 'object' && ga?.turn === 10);
	const hp = createHub();
	const wa = hp.poll('a', 300);
	hp.enqueue({threadId: 's', turn: 1, message: 'one'});
	ok('hub: setup a owns thread', ((await wa) as {threadId: string}).threadId === 's');
	const wa2 = hp.poll('a', 300);
	const wb = hp.poll('b', 80);
	await tick();
	hp.enqueue({threadId: 's', turn: 2, message: 'two', followUp: true, history: []});
	const r = await wa2;
	ok('hub: follow-up to owner when both poll', typeof r === 'object' && r?.turn === 2 && r.followUp === true);
	ok('hub: other poller not blocked (times out null)', (await wb) === null);
	// get_questions marks follow-ups
	const h3 = createHub();
	h3.enqueue({threadId: 'g', turn: 2, message: 'gg', followUp: true, history: []});
	h3.enqueue({threadId: 'h', turn: 1, message: 'hh'});
	const gq = parse(await callTool('get_questions', {}, {hub: h3, clientId: 'z'}));
	ok(
		'tools: get_questions marks follow-ups',
		gq.questions[0].follow_up === true && gq.questions[1].follow_up === false,
	);
	const h4 = createHub();
	h4.enqueue({threadId: 't', turn: 1, message: 'plain'});
	const n4 = parse(await callTool('next_question', {wait_seconds: 1}, {hub: h4, clientId: 'z'}));
	ok('tools: turn 1 plain', n4.follow_up === false && !('previous' in n4) && n4.question === 'plain');
	ok(
		'tools: descriptions mention follow_up',
		TOOLS.find((t) => t.name === 'next_question')!.description.includes('follow_up'),
	);
}

for (const d of made) rmSync(d, {recursive: true, force: true});
console.log(fail === 0 ? 'ALL PASS' : `${fail} FAIL`);
process.exit(fail === 0 ? 0 : 1);
