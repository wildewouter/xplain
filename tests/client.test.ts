import {EventEmitter} from 'node:events';
import type {ChildProcess} from 'node:child_process';
import {claudeClient, parseLine, newState} from '../src/agents/client/claude.js';
import {createClient} from '../src/agents/client/index.js';
import type {AgentEvent, AskContext} from '../src/agents/client/index.js';

let fail = 0;
const ok = (n: string, c: boolean) => {
	console.log(c ? 'PASS' : 'FAIL', n);
	if (!c) fail++;
};

class Fake extends EventEmitter {
	stdout = new EventEmitter();
	stderr = new EventEmitter();
	exitCode: number | null = null;
	signalCode: string | null = null;
	signals: string[] = [];
	kill(s: string) {
		this.signals.push(s);
		if (s === 'SIGINT') setImmediate(() => this.end(null, 'SIGINT'));
		return true;
	}
	out(s: string) {
		this.stdout.emit('data', Buffer.from(s));
	}
	end(code: number | null, sig: string | null = null) {
		this.exitCode = code;
		this.signalCode = sig;
		this.emit('close', code, sig);
	}
}
const rig = (script: (f: Fake) => void | Promise<void>) => {
	const calls: {cmd: string; args: string[]; opts: any}[] = [];
	const fakes: Fake[] = [];
	const spawn = (cmd: string, args: string[], opts: any) => {
		const f = new Fake();
		fakes.push(f);
		calls.push({cmd, args, opts});
		setImmediate(() => void script(f));
		return f as unknown as ChildProcess;
	};
	return {calls, fakes, client: claudeClient({spawn, env: {X: '1'}})};
};
const ctx: AskContext = {cwd: '/w', sessionId: 'S1', mode: 'fork', readOnly: true};
const collect = async (it: AsyncIterable<AgentEvent>) => {
	const r: AgentEvent[] = [];
	for await (const e of it) r.push(e);
	return r;
};
const J = (o: unknown) => JSON.stringify(o) + '\n';
const delta = (text: string, p: string | null = null) =>
	J({
		type: 'stream_event',
		parent_tool_use_id: p,
		event: {type: 'content_block_delta', delta: {type: 'text_delta', text}},
	});

// pure parser
{
	const s = newState();
	ok(
		'init',
		JSON.stringify(parseLine(J({type: 'system', subtype: 'init', session_id: 'a', model: 'm'}), s)) ===
			'[{"t":"init","sessionId":"a","model":"m"}]',
	);
	ok(
		'unknown ignored',
		parseLine('{"type":"wat"}', s).length === 0 && parseLine('[]', s).length === 0 && parseLine('', s).length === 0,
	);
	ok('malformed -> notice', parseLine('{bad', s)[0]?.t === 'notice');
	const s2 = newState();
	const asst = J({
		type: 'assistant',
		message: {
			content: [
				{type: 'text', text: 'hi'},
				{type: 'tool_use', id: 't', name: 'Read', input: {a: 1}},
			],
		},
	});
	const r = parseLine(asst, s2);
	ok('assistant w/o deltas emits text+tool', r.length === 2 && r[0]?.t === 'text' && r[1]?.t === 'tool');
	parseLine(delta('x'), s2);
	ok('assistant after deltas skips text', parseLine(asst, s2).length === 1);
	ok(
		'tool_result error',
		(
			parseLine(
				J({type: 'user', message: {content: [{type: 'tool_result', tool_use_id: 't', is_error: true}]}}),
				s,
			)[0] as any
		).ok === false,
	);
}

const happy = (f: Fake) => {
	f.out(J({type: 'system', subtype: 'init', session_id: 'N', model: 'm'}));
	f.out(delta('Hel'));
	f.out(delta('lo'));
	f.out(delta('SUB', 'toolu_1'));
	f.out(
		J({
			type: 'assistant',
			message: {
				content: [
					{type: 'text', text: 'Hello'},
					{type: 'tool_use', id: 't1', name: 'Read', input: {}},
				],
			},
		}),
	);
	f.out(
		J({type: 'assistant', parent_tool_use_id: 'x', message: {content: [{type: 'tool_use', id: 'z', name: 'Sub'}]}}),
	);
	f.out(J({type: 'user', message: {content: [{type: 'tool_result', tool_use_id: 't1'}]}}));
	f.out(J({type: 'system', subtype: 'api_retry', error: 'overloaded'}));
	f.out('not json\n');
	const res = J({
		type: 'result',
		subtype: 'success',
		is_error: false,
		result: 'Hello',
		session_id: 'N',
		total_cost_usd: 0.01,
	});
	f.out(res.slice(0, 20));
	f.out(res.slice(20));
	f.end(0);
};
{
	const {client, calls} = rig(happy);
	const ev = await collect(client.send('q?', ctx));
	const t = ev.map((e) => e.t).join(',');
	ok('event order', t === 'init,text,text,tool,toolResult,notice,notice,done');
	ok(
		'text deltas only main',
		ev
			.filter((e) => e.t === 'text')
			.map((e: any) => e.delta)
			.join('') === 'Hello',
	);
	const d = ev.at(-1) as any;
	ok('done fields', d.text === 'Hello' && d.sessionId === 'N' && d.costUsd === 0.01 && d.isError === false);
	ok(
		'api_retry notice',
		ev.some((e: any) => e.t === 'notice' && e.msg.includes('api_retry')),
	);
	const a = calls[0]!.args;
	ok('cmd claude, cwd, env', calls[0]!.cmd === 'claude' && calls[0]!.opts.cwd === '/w' && calls[0]!.opts.env.X === '1');
	ok('stdin ignored', calls[0]!.opts.stdio[0] === 'ignore');
	ok(
		'base flags',
		a[0] === '-p' &&
			a[1] === 'q?' &&
			a.includes('stream-json') &&
			a.includes('--verbose') &&
			a.includes('--include-partial-messages'),
	);
	ok('fork flags', a.includes('--resume') && a[a.indexOf('--resume') + 1] === 'S1' && a.includes('--fork-session'));
	ok(
		'readOnly flags',
		a[a.indexOf('--permission-mode') + 1] === 'plan' &&
			a[a.indexOf('--allowedTools') + 1] === 'Read,Grep,Glob' &&
			a[a.indexOf('--permission-prompts') + 1] === 'none',
	);
	ok('no --bare', !a.includes('--bare'));
	ok('no systemAppend by default', !a.includes('--append-system-prompt'));
}
{
	const {client, calls} = rig(happy);
	await collect(client.send('q', {cwd: '/w', mode: 'new', readOnly: false, systemAppend: 'BE BRIEF'}));
	const a = calls[0]!.args;
	ok('mode new: no fork flags', !a.includes('--resume') && !a.includes('--fork-session'));
	ok('not readOnly: no plan flags', !a.includes('--permission-mode') && !a.includes('--allowedTools'));
	ok('systemAppend', a[a.indexOf('--append-system-prompt') + 1] === 'BE BRIEF');
}
{
	const {client, calls} = rig(happy);
	await collect(client.send('q', {cwd: '/w', mode: 'fork', readOnly: true}));
	ok('fork w/o sessionId: no resume', !calls[0]!.args.includes('--resume'));
}
{
	const {client} = rig((f) => {
		f.out(J({type: 'result', subtype: 'error_max_turns', is_error: true, result: 'boom', session_id: 'N'}));
		f.end(1);
	});
	const ev = await collect(client.send('q', ctx));
	ok('is_error result', ev.length === 1 && (ev[0] as any).t === 'done' && (ev[0] as any).isError === true);
}
{
	const {client} = rig((f) => {
		f.stderr.emit('data', Buffer.from('bad thing\nlast line'));
		f.end(2);
	});
	const ev = await collect(client.send('q', ctx));
	ok(
		'nonzero exit -> error w/ stderr',
		ev.length === 1 && ev[0]!.t === 'error' && (ev[0] as any).msg.includes('last line'),
	);
}
{
	const {client} = rig((f) => {
		f.emit('error', Object.assign(new Error('x'), {code: 'ENOENT'}));
	});
	const ev = await collect(client.send('q', ctx));
	ok('ENOENT', ev.length === 1 && (ev[0] as any).msg === 'claude not found');
	const c2 = claudeClient({
		spawn: () => {
			throw Object.assign(new Error('x'), {code: 'ENOENT'});
		},
	});
	const e2 = await collect(c2.send('q', ctx));
	ok('ENOENT sync throw', (e2[0] as any).msg === 'claude not found');
}
{
	const {client, fakes} = rig((f) => f.out(delta('a')));
	const p = collect(client.send('q', ctx));
	await new Promise((r) => setTimeout(r, 20));
	const second = await collect(client.send('q2', ctx));
	ok('second concurrent send errors', second.length === 1 && second[0]!.t === 'error');
	await client.cancel();
	const ev = await p;
	ok('cancel sends SIGINT', fakes[0]!.signals[0] === 'SIGINT');
	ok('cancel -> error cancelled', ev.at(-1)!.t === 'error' && (ev.at(-1) as any).msg === 'cancelled');
	const again = rig(happy);
	ok('cancel idle noop', (await again.client.cancel(), true));
}
{
	const c = claudeClient();
	ok(
		'supports',
		JSON.stringify(c.supports) === '{"resumeRunning":false,"tokenStreaming":true,"cancel":true,"fork":true}' &&
			c.id === 'claude',
	);
	ok('createClient claude', createClient('claude')?.id === 'claude');
	ok(
		'createClient others undefined',
		(['copilot', 'codex', 'opencode'] as const).every((i) => createClient(i) === undefined),
	);
}
process.exit(fail ? 1 : 0);
