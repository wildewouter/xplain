import {render} from 'ink-testing-library';
import App from '../src/app.js';
import {join} from 'node:path';
import {readFileSync} from 'node:fs';
import {createMcpBridge, type McpBridge} from '../src/mcp/bridge.js';
import {createHub, type Hub} from '../src/mcp/index.js';
import type {AgentIntegration} from '../src/integrations/index.js';
import type {AskController} from '../src/ask/index.js';
import {cwd, tick, ok, keyPress, finish} from './keysHelpers.js';
// ---- MCP bridge in the UI (real bridge, fake hub server/integrations/clipboard) ----
{
	const TOK = 'tok-secret-0123456789abcdef';
	type Rig = {
		hub: () => Hub;
		copied: string[];
		calls: string[];
		reg: {registered: boolean; stale?: boolean};
		disposed: () => number;
		factory: (c: AskController) => McpBridge;
	};
	const rig = (): Rig => {
		let hub!: Hub;
		let disposed = 0;
		const copied: string[] = [];
		const calls: string[] = [];
		const reg: {registered: boolean; stale?: boolean} = {registered: false};
		const mkInt = (id: string, label: string, canRegister: boolean): AgentIntegration => ({
			id,
			label,
			needsRestart: true,
			pollSeconds: 30,
			canRegister,
			registerCommand: (ep) => `add ${id} ${ep.url} --token ${ep.token}`,
			watchPrompt: () => `watch prompt for ${id}`,
			...(canRegister
				? {
						register: async (ep) => {
							calls.push('register ' + id);
							reg.registered = true;
							reg.stale = false;
							return {ok: true, message: `registered ${ep.token}`, needsRestart: true};
						},
						unregister: async () => {
							calls.push('unregister ' + id);
							reg.registered = false;
							return {ok: true, message: 'removed'};
						},
						isRegistered: async () => (id === 'aa' ? {...reg} : {registered: false}),
					}
				: {}),
		});
		return {
			hub: () => hub,
			copied,
			calls,
			reg,
			disposed: () => disposed,
			factory: (controller) => {
				const b = createMcpBridge({
					controller,
					stateDir: '/nonexistent',
					integrations: [mkInt('aa', 'Alpha', true), mkInt('bb', 'Beta', false)],
					createHub: () => (hub = createHub()),
					loadToken: () => ({token: TOK}),
					createServer: () => ({
						start: async () => ({url: 'http://127.0.0.1:47615/mcp', port: 47615}),
						stop: async () => {},
					}),
					clipboard: (t) => void copied.push(t),
				});
				const d = b.dispose;
				b.dispose = () => {
					disposed++;
					d();
				};
				return b;
			},
		};
	};
	const mkm = (props: Record<string, unknown> = {}, cols?: number) => {
		const rg = rig();
		const r = render(<App args={[]} cwd={cwd} mcp={rg.factory} {...props} />);
		if (cols) Object.defineProperty(r.stdout, 'columns', {value: cols});
		const g = () => r.lastFrame() ?? '';
		const w = keyPress(r);
		return {r, g, w, rg};
	};
	{
		const {r, g, w, rg} = mkm();
		await tick();
		const lines0 = g().split('\n').length;
		ok('chip off', g().includes('[mcp: off]') && g().includes('[unified]'));
		await w('M');
		ok('M opens modal', g().includes('MCP') && g().includes('○ off') && g().includes('INTEGRATIONS'));
		ok(
			'integrations by generic label',
			g().includes('Alpha') && g().includes('Beta') && g().includes('copy-paste only'),
		);
		ok('modal keeps header visible', g().split('\n')[0]!.includes('[mcp:'));
		await w('f');
		ok('other keys ignored in modal', !g().includes('Files (') && g().includes('INTEGRATIONS'));
		await w('\x1b');
		ok('esc closes', !g().includes('INTEGRATIONS'));
		await w('M');
		await w('q');
		await w('M');
		await w('M');
		await w('M');
		await w('j');
		await w('c');
		ok('copy while off -> note', g().includes('start MCP first') && rg.copied.length === 0);
		await w('k');
		await w('j');
		await w('\r');
		ok('register while off -> note', g().includes('start MCP first') && !g().includes('(y/n)'));
		await w('k');
		await w('\r');
		await tick();
		ok(
			'power on',
			g().includes('● on') && g().includes('127.0.0.1:47615') && /clients 0\s+pending 0\s+delivered 0/.test(g()),
		);
		await w('\x1b');
		ok('chip on', g().includes('[mcp: on]'));
		ok('viewport height unchanged', g().split('\n').length === lines0);
		await w('M');
		await w('j');
		await w('\r');
		ok(
			'register confirm',
			g().includes('Register Alpha MCP server? (y/n)') && g().includes('adds the xplain MCP server'),
		);
		await w('n');
		ok('n cancels confirm', !g().includes('(y/n)') && rg.calls.length === 0);
		await w('\r');
		await w('y');
		await tick();
		ok('y registers', rg.calls.includes('register aa'));
		ok(
			'restart hint after register, token masked',
			g().includes('restart the agent session, then paste the') && g().includes('watch prompt') && !g().includes(TOK),
		);
		await w('\r');
		ok(
			'enter on registered -> note, no confirm',
			g().includes('already registered (d to remove)') && !g().includes('(y/n)') && rg.calls.length === 1,
		);
		await w('r');
		await w('u');
		ok('r/u ignored', !g().includes('(y/n)') && rg.calls.length === 1);
		await w('d');
		ok('unregister confirm', g().includes('Remove Alpha registration? (y/n)'));
		await w('y');
		await tick();
		ok('y unregisters', rg.calls.includes('unregister aa') && g().includes('removed'));
		await w('c');
		ok(
			'c copies command (full)',
			rg.copied.length === 1 && rg.copied[0]!.includes(TOK) && rg.copied[0]!.includes('add aa'),
		);
		ok(
			'c preview masked',
			g().includes('add aa http://127.0.0.1:47615/mcp') && !g().includes(TOK) && g().includes('***'),
		);
		await w('w');
		ok('w copies watch + preview', rg.copied[1] === 'watch prompt for aa' && g().includes('watch prompt for aa'));
		await w('d');
		ok('d on not registered -> note', g().includes('not registered') && !g().includes('(y/n)'));
		await w('r');
		await w('u');
		ok('r/u ignored when unregistered', !g().includes('(y/n)') && rg.calls.length === 2);
		await w('\r');
		await w('n');
		await w('j');
		await w('\r');
		ok('non-registerable enter: note', g().includes('copy-paste only: c copies the snippet') && !g().includes('(y/n)'));
		await w('d');
		ok('non-registerable d: note', g().includes('copy-paste only: c copies the snippet') && !g().includes('(y/n)'));
		ok('hint row', g().includes('j/k  enter register  d remove  c/w copy  R refresh  esc'));
		await w('k');
		rg.reg.registered = true;
		rg.reg.stale = true;
		await w('R');
		await tick();
		ok('stale shown', g().includes('stale'));
		await w('\r');
		ok('enter on stale -> register confirm', g().includes('Register Alpha MCP server? (y/n)'));
		await w('y');
		await tick();
		ok('stale re-registered', rg.calls.filter((c) => c === 'register aa').length === 2);
		await w('j');
		await w('k');
		await w('k');
		await w('R');
		await w('k');
		await w('\r');
		await tick();
		ok('power off', g().includes('○ off'));
		await w('\x1b');
		await w('?');
		await w('?');
		r.unmount();
	}
	{
		const {r, g, w, rg} = mkm();
		await tick();
		await w('i');
		await w('j');
		await w('\r');
		ok('mode default save when off', g().includes('[save] enter send  tab save/ask  esc cancel'));
		await w('\t');
		ok('tab to ask while off -> note', g().includes('MCP is off (M to start)') && g().includes('[save]'));
		await w('\x1b');
		await w('\x1b');
		await w('M');
		await w('\r');
		await tick();
		await w('\x1b');
		await w('i');
		await w('\r');
		ok('mode default ask when on', g().includes('[ask] enter send'));
		await w('\t');
		ok('tab -> save', g().includes('[save] enter send'));
		await w('\t');
		ok('tab -> ask', g().includes('[ask] enter send'));
		await w('\t');
		await w('\x1b');
		await w('\r');
		ok('chosen mode remembered while running', g().includes('[save] enter send'));
		await w('\x1b');
		await w('\x1b');
		await w('M');
		await w('\r');
		await tick();
		await w('\r');
		await tick();
		await w('\x1b');
		await w('i');
		await w('j');
		await w('\r');
		ok('mode resets to ask after restart', g().includes('[ask] enter send'));
		r.unmount();
		void rg;
	}
	for (const [name, props, keysFn] of [
		['unified', {}, async (w: (k: string) => Promise<void>) => void (await w('i'), await w('j'), await w('\r'))],
		[
			'split',
			{split: true},
			async (w: (k: string) => Promise<void>) => void (await w('\t'), await w('i'), await w('j'), await w('\r')),
		],
		[
			'browse',
			{},
			async (w: (k: string) => Promise<void>) =>
				void (await w('F'),
				await w('a.ts'),
				await w('\r'),
				await tick(),
				await w('i'),
				await w('j'),
				await w('j'),
				await w('\r')),
		],
	] as const) {
		const {r, g, w, rg} = mkm(props);
		await tick();
		await w('M');
		await w('\r');
		await tick();
		await w('\x1b');
		await keysFn(w);
		await w('why?');
		const before = g().split('\n').length;
		await w('\r');
		const h = rg.hub();
		const delivered: string[] = [];
		h.subscribe((e) => e.type === 'delivered' && delivered.push(e.threadId));
		ok(
			`${name}: enqueued via bridge`,
			h.pending().length === 1 && h.pending()[0]!.message === 'why?' && /File: /.test(h.pending()[0]!.context ?? ''),
		);
		if (name === 'unified')
			ok(`${name}: waiting shown`, g().includes('waiting for agent…') && g().includes('question sent to agent'));
		h.registerClient('c1', 'agent-x', '1');
		await h.poll('c1', 100);
		await tick();
		if (name === 'unified')
			ok(`${name}: working shown`, g().includes('agent working…') && g().includes('answer · agent-x'));
		h.answer(delivered.at(-1) ?? 'none', 'the answer text');
		await tick();
		ok(`${name}: answer shown`, g().includes('the answer text') && g().includes('answer · agent-x · done'));
		if (name === 'unified') ok(`${name}: viewport height unchanged`, g().split('\n').length === before);
		r.unmount();
	}
	{
		const {r, g, w, rg} = mkm({split: false}, 40);
		await tick();
		await w('M');
		await w('\r');
		await tick();
		await w('j');
		await w('c');
		ok(
			'narrow: modal fits width',
			g().includes('INTEGRATIONS') &&
				g()
					.split('\n')
					.every((l) => l.length <= 40),
		);
		r.unmount();
		void rg;
	}
	{
		// saved-only comments: marker, a / A ask later
		const mkc = (props: Record<string, unknown> = {}, cols?: number) => {
			const rg = rig();
			let ctl!: AskController;
			const r = render(
				<App
					args={[]}
					cwd={cwd}
					mcp={(c) => {
						ctl = c;
						return rg.factory(c);
					}}
					{...props}
				/>,
			);
			if (cols) Object.defineProperty(r.stdout, 'columns', {value: cols});
			const g = () => r.lastFrame() ?? '';
			const w = keyPress(r);
			const on = async () => {
				await w('M');
				await w('\r');
				await tick();
				await w('\x1b');
			};
			const save = async (msg: string, save = true) => {
				await w('a');
				await w(msg);
				if (g().includes(save ? '[ask]' : '[save]')) await w('\t');
				await w('\r');
			};
			return {r, g, w, rg, on, save, ctl: () => ctl};
		};
		for (const [name, props, keysFn] of [
			['unified', {}, async (w: (k: string) => Promise<void>) => void (await w('i'), await w('j'))],
			[
				'split',
				{split: true},
				async (w: (k: string) => Promise<void>) => void (await w('\t'), await w('i'), await w('j')),
			],
			[
				'browse',
				{},
				async (w: (k: string) => Promise<void>) =>
					void (await w('F'), await w('a.ts'), await w('\r'), await tick(), await w('i'), await w('j'), await w('j')),
			],
		] as const) {
			const m = mkc(props);
			const {r, g, w, rg} = m;
			await tick();
			await m.on();
			await keysFn(w);
			const before0 = g().split('\n').length;
			await m.save('later?');
			ok(`${name}: saved marker`, g().includes('saved · not asked') && g().includes('question saved'));
			await w('K');
			ok(`${name}: hint shows a ask`, g().includes('a ask  esc back'));
			const before = g().split('\n').length;
			await w('a');
			const h = rg.hub();
			ok(
				`${name}: a enqueues once`,
				h.pending().length === 1 &&
					h.pending()[0]!.message === 'later?' &&
					/File: /.test(h.pending()[0]!.context ?? '') &&
					g().includes('question queued') &&
					g().includes('waiting for agent…') &&
					g().includes('▸') &&
					!g().includes('saved · not asked') &&
					!g().includes('a ask  esc back'),
			);
			await w('a');
			ok(`${name}: second a waits`, g().includes('still waiting for the agent') && h.pending().length === 1);
			if (name === 'unified') {
				ok('viewport height unchanged', g().split('\n').length === before || g().split('\n').length === before0);
			}
			r.unmount();
		}
		{
			const m = mkc();
			const {r, g, w, rg} = m;
			await tick();
			await m.on();
			await w('i');
			await w('j');
			await m.save('q1');
			await w('a');
			ok('unfocused a opens ask box', g().includes('enter send'));
			await w('\x1b');
			r.unmount();
			void rg;
		}
		{
			// MCP off
			const m = mkc();
			const {r, g, w, rg} = m;
			await tick();
			await w('i');
			await w('j');
			await m.save('q1', false);
			ok('off: marker shown', g().includes('saved · not asked'));
			await w('K');
			await w('a');
			ok('off: a note, nothing enqueued', g().includes('MCP is off (M to start)') && g().includes('saved · not asked'));
			await w('A');
			ok('off: A note', g().includes('MCP is off (M to start)'));
			ok(
				'off: no hub',
				(() => {
					try {
						return rg.hub()?.pending().length ?? 0;
					} catch {
						return 0;
					}
				})() === 0,
			);
			r.unmount();
		}
		{
			// retry, agent note, bulk
			const m = mkc();
			const {r, g, w, rg} = m;
			await tick();
			await m.on();
			await w('i');
			await w('j');
			await m.save('q1');
			await w('j');
			await m.save('q2', false);
			await w('j');
			await m.save('q3');
			const h = rg.hub();
			ok('asked comment has no marker', (g().match(/saved · not asked/g) ?? []).length === 2);
			await w('J');
			await w('A');
			ok(
				'A queues all eligible in order, skips asked',
				h.pending().length === 3 &&
					h.pending()[1]!.message === 'q1' &&
					h.pending()[2]!.message === 'q3' &&
					g().includes('queued 2 questions'),
			);
			await w('A');
			ok('A nothing to ask', g().includes('nothing to ask'));
			const ids = m
				.ctl()
				.getState()
				.questions.map((q) => q.id!);
			m.ctl().setAnswer(ids[0]!, {status: 'error', text: '', error: 'boom'});
			await tick();
			ok('error comment shows hint', g().includes('a ask  esc back'));
			await w('a');
			ok('error retry enqueues', h.pending().length === 4 && h.pending()[3]!.message === 'q1');
			m.ctl().setAnswer(ids[0]!, {status: 'cancelled', text: ''});
			await tick();
			await w('A');
			ok('cancelled retry via A', h.pending().length === 5 && g().includes('queued 1 question'));
			m.ctl().add({file: 'README.md', index: 0, line: 3, text: 'x', message: 'note', origin: 'agent'});
			await tick();
			await w('\x1b');
			await w('K');
			ok('agent note focused', g().includes('agent note'));
			await w('a');
			ok('agent note rejected', g().includes("agent notes can't be asked") && h.pending().length === 5);
			r.unmount();
		}
		{
			const m = mkc({}, 44);
			const {r, g, w} = m;
			await tick();
			await m.on();
			await w('i');
			await w('j');
			await m.save('q1');
			await w('K');
			ok(
				'narrow: hint fits, keeps e/D',
				g()
					.split('\n')
					.every((l) => l.length <= 44) && g().includes('e edit  D delete'),
			);
			r.unmount();
		}
	}
	const starts = (b: McpBridge) => {
		let n = 0;
		let stops = 0;
		const s = b.start;
		const t = b.stop;
		b.start = () => (n++, s());
		b.stop = () => (stops++, t());
		return {n: () => n, stops: () => stops};
	};
	const mkb = (props: Record<string, unknown>, start?: () => Promise<void>) => {
		const rg = rig();
		let cnt: ReturnType<typeof starts> | undefined;
		let bb!: McpBridge;
		const r = render(
			<App
				args={[]}
				cwd={cwd}
				mcp={(c) => {
					bb = rg.factory(c);
					if (start) bb.start = start;
					cnt = starts(bb);
					return bb;
				}}
				{...props}
			/>,
		);
		const g = () => r.lastFrame() ?? '';
		const w = keyPress(r);
		return {r, g, w, cnt: () => cnt!, bb: () => bb};
	};
	{
		// autostart + config modal
		const cp = join(process.env.TMPDIR ?? '.', 'tmp', `mcpa${Date.now()}.json`);
		const {r, g, w, cnt} = mkb({configPath: cp, confirmQuit: false});
		await tick();
		await w('C');
		ok('config lists mcp on startup', /mcp on startup\s+\[off\]\s+on/.test(g()));
		ok('config fits 24 rows', g().split('\n').length <= 24);
		for (let k = 0; k < 5; k++) await w('j');
		await w('l');
		await w('\r');
		ok(
			'enter saves mcp autostart, no start/stop',
			/mcp on startup\s+off\s+\[on\]/.test(g()) &&
				JSON.parse(readFileSync(cp, 'utf8')).mcp.autostart === true &&
				cnt().n() === 0 &&
				cnt().stops() === 0 &&
				!g().includes('[mcp: on]'),
		);
		r.unmount();
	}
	{
		const {r, g, cnt} = mkb({mcpAutostart: true});
		await tick();
		await tick();
		ok('autostart: start once, header on, ask default', cnt().n() === 1 && g().includes('[mcp: on]'));
		r.unmount();
	}
	{
		const {r, g, w, cnt} = mkb({mcpAutostart: false});
		await tick();
		ok('autostart off: no start', cnt().n() === 0 && g().includes('[mcp: off]'));
		await w('j');
		r.unmount();
	}
	{
		const rg = rig();
		const r = render(
			<App
				args={[]}
				cwd={cwd}
				mcpAutostart
				mcp={(c) =>
					createMcpBridge({
						controller: c,
						stateDir: '/nonexistent',
						integrations: [],
						loadToken: () => ({token: TOK}),
						createHub: () => createHub(),
						createServer: () => ({
							start: async () => {
								throw Object.assign(new Error('boom ' + TOK), {code: 'EADDRINUSE'});
							},
							stop: async () => {},
						}),
					})
				}
			/>,
		);
		void rg;
		await tick();
		await tick();
		const f = r.lastFrame() ?? '';
		ok(
			'autostart fail: note, MCP off, no token',
			f.includes('mcp autostart failed: port 47615 in use') && f.includes('[mcp: off]') && !f.includes(TOK),
		);
		await keyPress(r)('j');
		ok('autostart fail: app usable', (r.lastFrame() ?? '').includes('[mcp: off]'));
		r.unmount();
	}
	{
		const {r, w, rg} = mkm({confirmQuit: false});
		await tick();
		await w('q');
		await tick();
		ok('quit disposes bridge', rg.disposed() >= 1);
		r.unmount();
	}
	{
		// threads: rendering, follow-up input, scroll (real bridge)
		const mkt = (props: Record<string, unknown> = {}, cols?: number) => {
			const rg = rig();
			let ctl!: AskController;
			const r = render(
				<App
					args={[]}
					cwd={cwd}
					mcp={(c) => {
						ctl = c;
						return rg.factory(c);
					}}
					{...props}
				/>,
			);
			if (cols) Object.defineProperty(r.stdout, 'columns', {value: cols});
			const g = () => r.lastFrame() ?? '';
			const w = keyPress(r);
			const ws = async (ks: string) => {
				for (const k of ks) await w(k);
			};
			const on = async () => {
				await w('M');
				await w('\r');
				await tick();
				await w('\x1b');
			};
			// comment on row 1 (MCP off => saved only)
			const add = async (msg = 'why?') => {
				await ws('ij');
				await w('a');
				await ws(msg);
				await w('\r');
			};
			const id = () => ctl.getState().questions[0]!.id!;
			const ans = (text: string, status: 'done' | 'streaming' | 'error' = 'done', turn?: number) =>
				ctl.setAnswer(id(), {status, text, agent: 'ag', ...(status === 'error' ? {error: text} : {})}, turn);
			return {r, g, w, ws, on, add, id, ans, rg, ctl: () => ctl};
		};
		const rows = (n: number, tag = 'row') =>
			Array.from({length: n}, (_, i) => `${tag}-${String(i + 1).padStart(4, '0')}`).join('\n');
		const at = (g: string, s: string) => g.indexOf(s);
		for (const [name, props, pre] of [
			['unified', {}, ''],
			['split', {split: true}, '\t'],
			['browse', {}, 'F'],
		] as const) {
			const m = mkt(props);
			await tick();
			if (name === 'browse') {
				await m.ws('a.ts');
				await m.w('\r');
				await tick();
				await m.ws('ijj');
				await m.w('a');
				await m.ws('q1');
				await m.w('\r');
			} else {
				if (pre) await m.w(pre);
				await m.add('q1');
			}
			m.ans('first answer');
			m.ctl().followUp(m.id(), 'and then?');
			m.ans('second answer', 'done');
			await tick();
			const f = m.g();
			ok(
				`${name}: thread 2 turns in order`,
				at(f, 'q1') < at(f, 'first answer') &&
					at(f, 'first answer') < at(f, 'follow-up: and then?') &&
					at(f, 'follow-up: and then?') < at(f, 'second answer') &&
					f.split('answer · ag · done').length === 3,
			);
			m.ctl().followUp(m.id(), 'third');
			await tick();
			ok(`${name}: new turn waits`, m.g().includes('follow-up: third'));
			m.r.unmount();
		}
		{
			// follow-up input via the bridge
			const m = mkt();
			const {g, w, ws, rg} = m;
			await tick();
			await m.on();
			await ws('ij');
			await w('a');
			await ws('q1');
			await w('\r');
			await w('K');
			await w('a');
			ok(
				'a on live: note',
				g().includes('still waiting for the agent') && !g().includes('follow-up  ') && !g().includes('esc cancel'),
			);
			const h = rg.hub();
			const delivered: string[] = [];
			h.subscribe((e) => e.type === 'delivered' && delivered.push(e.threadId));
			h.registerClient('c1', 'agent-x', '1');
			await h.poll('c1', 100);
			await tick();
			h.answer(delivered[0]!, 'ans1');
			await tick();
			ok('hint: follow up', g().includes('a follow up'));
			const lines0 = g().split('\n').length;
			await w('a');
			ok(
				'a on answered: follow-up input',
				g().includes('follow-up') && g().includes('enter send  esc cancel') && !g().includes('tab save/ask'),
			);
			ok('follow-up input: viewport height unchanged', g().split('\n').length === lines0);
			await w('\r');
			ok('empty ignored', g().includes('enter send  esc cancel') && h.pending().length === 0);
			await ws('more?');
			await w('\x1b');
			ok('esc cancels', !g().includes('enter send') && h.pending().length === 0);
			await w('a');
			ok('reopen empty', !g().includes('more?'));
			await ws('more?');
			await w('\r');
			await tick();
			ok(
				'send: queued + waiting',
				h.pending().length === 1 &&
					h.pending()[0]!.message === 'more?' &&
					h.pending()[0]!.turn === 2 &&
					g().includes('follow-up queued') &&
					g().includes('follow-up: more?') &&
					g().includes('waiting for agent…') &&
					!g().includes('enter send'),
			);
			await w('e');
			ok('edit after follow-up: note', g().includes("can't edit after follow-ups") && !g().includes('enter send'));
			await w('a');
			ok('a on live follow-up: note', g().includes('still waiting for the agent'));
			await w('D');
			await w('y');
			ok('delete removes thread', !g().includes('follow-up: more?') && !g().includes('ans1'));
			m.r.unmount();
		}
		{
			// MCP off: note, draft kept
			const m = mkt();
			const {g, w, ws} = m;
			await tick();
			await m.add('q1');
			m.ans('ans1');
			await w('K');
			await w('a');
			await ws('draft');
			await w('\r');
			ok(
				'MCP off: note, box + draft kept',
				g().includes('MCP is off (M to start)') && g().includes('draft') && g().includes('enter send'),
			);
			m.r.unmount();
		}
		{
			// scroll inside a long thread
			const m = mkt();
			const {g, w} = m;
			await tick();
			await m.add('q1');
			m.ans(rows(60));
			await w('K');
			const cur = () => /↕ (\d+)-(\d+)\/(\d+)/.exec(g())?.slice(1).map(Number);
			const lines0 = g().split('\n').length;
			ok(
				'marker shows position, height const',
				cur()?.[0] === 1 && cur()![2] === 62 && g().split('\n').length === lines0,
			);
			ok(
				'long thread: cursor + hint visible',
				g().includes('[cursor') && g().includes('j/k scroll') && g().includes('? help'),
			);
			await w('j');
			ok('j scrolls one', cur()?.[0] === 2 && g().includes('▸'));
			await w('k');
			await w('k');
			ok('k clamps at top', cur()?.[0] === 1);
			await w('d');
			const half = cur()![0]! - 1;
			ok('d scrolls about half', half > 3 && half < 15);
			await w('u');
			ok('u back', cur()?.[0] === 1);
			await w('G');
			ok('G bottom', cur()?.[1] === 62 && g().includes('row-0060') && g().split('\n').length === lines0);
			await w('g');
			ok('g top', cur()?.[0] === 1 && g().includes('q1'));
			await w('l');
			ok('other motion unfocuses', !g().includes('▸') && !g().includes('↕'));
			m.r.unmount();
		}
		{
			// auto-follow tail while streaming
			const m = mkt();
			const {g, w} = m;
			await tick();
			await m.add('q1');
			m.ans(rows(60), 'streaming');
			await w('K');
			ok('streaming: tail shown', g().includes('row-0060') && !g().includes('row-0001'));
			m.ans(rows(70), 'streaming');
			await tick();
			ok('streaming: follows growth', g().includes('row-0070'));
			await w('k');
			ok('k leaves tail', !g().includes('row-0070'));
			m.ans(rows(80), 'streaming');
			await tick();
			ok('scrolled up: no follow', !g().includes('row-0080') && !g().includes('row-0070'));
			await w('G');
			ok('G re-follows', g().includes('row-0080'));
			m.ans(rows(90), 'streaming');
			await tick();
			ok('follows again', g().includes('row-0090'));
			m.r.unmount();
		}
		{
			// long thread of several turns keeps viewport constant
			const m = mkt();
			const {g, w} = m;
			await tick();
			await m.add('q1');
			const lines0 = g().split('\n').length;
			m.ans(rows(15, 'a'));
			m.ctl().followUp(m.id(), 'f2');
			m.ans(rows(15, 'b'));
			m.ctl().followUp(m.id(), 'f3');
			m.ans(rows(15, 'c'));
			await w('K');
			ok('multi-turn long: height const, marker', g().split('\n').length === lines0 && g().includes('↕'));
			await w('G');
			ok('multi-turn long: bottom', g().includes('c-0015') && g().includes('answer · ag · done'));
			m.r.unmount();
		}
		{
			const m = mkt({}, 40);
			const {g, w} = m;
			await tick();
			await m.add('q1');
			m.ans('a1');
			await w('K');
			ok(
				'narrow: hint drops parts, fits',
				g().includes('D delete') &&
					g()
						.split('\n')
						.every((l) => l.length <= 40),
			);
			m.r.unmount();
		}
	}
}

finish();
