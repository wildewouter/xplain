// Headless glue between the ask controller, the MCP hub/server and agent integrations. No react/ink.
import {execFile} from 'node:child_process';
import {homedir} from 'node:os';
import {isLiveStatus, buildContext, type AskController, type Question} from '../ask/index.js';
import {createIntegrations, type AgentIntegration, type McpEndpoint, type Runner} from '../integrations/index.js';
import {osc52Copy} from '../clipboard.js';
import {createHub, type Hub} from './hub.js';
import {createMcpServer, type McpServer} from './server.js';
import {loadOrCreateToken} from './token.js';

export type BridgeClient = {id: string; name: string; version: string; polling: boolean};
export type BridgeIntegration = {
	id: string;
	label: string;
	canRegister: boolean;
	needsRestart: boolean;
	pollSeconds: number;
	registered?: boolean;
	stale?: boolean;
	busy?: boolean;
	note?: string;
};
export type BridgeState = {
	running: boolean;
	starting: boolean;
	error?: string;
	url?: string;
	port?: number;
	tokenMasked?: string;
	clients: BridgeClient[];
	pending: number;
	delivered: number;
	integrations: BridgeIntegration[];
};

export type BridgeDeps = {
	controller: AskController;
	stateDir?: string;
	port?: number;
	cwd?: string;
	integrations?: AgentIntegration[];
	createHub?: () => Hub;
	createServer?: (o: {hub: Hub; token: string; port?: number; dir?: string}) => McpServer;
	loadToken?: (dir?: string) => {token: string};
	clipboard?: (text: string) => void;
};

export type McpBridge = ReturnType<typeof createMcpBridge>;

const defaultRun: Runner = (cmd, args, opts) =>
	new Promise((resolve, reject) => {
		execFile(
			cmd,
			args,
			{cwd: opts?.cwd, env: {...process.env, ...opts?.env}, timeout: 20000, maxBuffer: 1 << 20},
			(err, stdout, stderr) => {
				if (err && typeof (err as {code?: unknown}).code !== 'number') return reject(err);
				resolve({code: err ? ((err as {code?: number}).code ?? 1) : 0, stdout: String(stdout), stderr: String(stderr)});
			},
		);
	});

export function createMcpBridge(deps: BridgeDeps) {
	const {controller} = deps;
	const integrations =
		deps.integrations ?? createIntegrations({run: defaultRun, cwd: deps.cwd ?? process.cwd(), home: homedir()});
	const clip = deps.clipboard ?? ((t: string) => osc52Copy(t));
	let state: BridgeState = {
		running: false,
		starting: false,
		clients: [],
		pending: 0,
		delivered: 0,
		integrations: integrations.map((i) => ({
			id: i.id,
			label: i.label,
			canRegister: i.canRegister,
			needsRestart: i.needsRestart,
			pollSeconds: i.pollSeconds,
		})),
	};
	const listeners = new Set<() => void>();
	const changeListeners = new Set<(paths: string[]) => void>();
	let hub: Hub | null = null;
	let server: McpServer | null = null;
	let unsub: (() => void) | null = null;
	let ep: McpEndpoint | null = null;
	let disposed = false;
	const agentOf = new Map<string, string>();

	const set = (p: Partial<BridgeState>) => {
		state = {...state, ...p};
		listeners.forEach((l) => l());
	};
	const setInt = (id: string, p: Partial<BridgeIntegration>) =>
		set({integrations: state.integrations.map((i) => (i.id === id ? {...i, ...p} : i))});
	const maskTok = (s: string) => (ep?.token ? s.split(ep.token).join('***') : s);
	const clientName = (clientId: string) => {
		const c = hub?.status().clients.find((x) => x.id === clientId);
		return c?.name && c.name !== 'unknown' ? c.name : undefined;
	};
	const syncHub = () => {
		if (!hub) return;
		const s = hub.status();
		set({
			clients: s.clients.map((c) => ({id: c.id, name: c.name, version: c.version, polling: c.polling})),
			pending: s.pending,
			delivered: s.delivered,
		});
	};
	const cancelLive = () => {
		for (const q of controller.getState().questions)
			if (q.id && isLiveStatus(controller.answer(q.id)))
				controller.setAnswer(q.id, {status: 'cancelled', text: 'MCP stopped'}); // latest turn
	};

	const onEvent = (e: import('./hub.js').HubEvent) => {
		if (e.type === 'delivered') {
			const agent = clientName(e.clientId);
			if (agent) agentOf.set(e.threadId, agent);
			if (controller.isLive(e.threadId)) controller.setAnswer(e.threadId, {status: 'streaming', text: '', agent});
		} else if (e.type === 'answer') {
			if (controller.turns(e.threadId).length >= e.turn)
				controller.setAnswer(e.threadId, {status: 'done', text: e.text, agent: agentOf.get(e.threadId)}, e.turn);
		} else if (e.type === 'files_changed') {
			changeListeners.forEach((l) => l(e.paths));
		} else if (e.type === 'annotate') {
			controller.add({
				file: e.file,
				index: 0,
				line: e.line,
				side: e.side ?? 'new',
				text: '',
				message: e.text,
				origin: 'agent',
			});
		}
		syncHub();
	};

	const teardown = async () => {
		const h = hub;
		const s = server;
		hub = null;
		server = null;
		ep = null;
		unsub?.();
		unsub = null;
		cancelLive();
		h?.close();
		try {
			await s?.stop();
		} catch {
			// already stopped
		}
	};

	const api = {
		getState: () => state,
		subscribe(l: () => void) {
			listeners.add(l);
			return () => void listeners.delete(l);
		},

		/** Subscribe to agent "files changed" signals. */
		onFilesChanged(l: (paths: string[]) => void) {
			changeListeners.add(l);
			return () => void changeListeners.delete(l);
		},

		async start(): Promise<void> {
			if (disposed || state.running || state.starting) return;
			set({starting: true, error: undefined});
			const h = (deps.createHub ?? createHub)();
			try {
				const token = (deps.loadToken ?? loadOrCreateToken)(deps.stateDir).token;
				const s = (deps.createServer ?? createMcpServer)({hub: h, token, port: deps.port, dir: deps.stateDir});
				const {url, port} = await s.start();
				if (disposed) {
					h.close();
					await s.stop();
					return;
				}
				hub = h;
				server = s;
				ep = {url, token, port};
				unsub = h.subscribe(onEvent);
				set({running: true, starting: false, url, port, tokenMasked: token.slice(0, 3) + '…'});
				syncHub();
				void api.refreshRegistration();
			} catch (e) {
				h.close();
				const code = (e as {code?: string})?.code;
				const msg = code === 'EADDRINUSE' ? `port ${deps.port ?? 47615} in use` : String((e as Error)?.message ?? e);
				set({running: false, starting: false, error: msg});
			}
		},

		async stop(): Promise<void> {
			if (!state.running && !hub) return;
			await teardown();
			set({running: false, url: undefined, port: undefined, tokenMasked: undefined, clients: [], pending: 0});
		},

		dispose() {
			if (disposed) return;
			disposed = true;
			void teardown();
			state = {...state, running: false, starting: false, clients: [], pending: 0};
			listeners.clear();
			changeListeners.clear();
		},

		/** Send a stored comment to a connected agent. False when MCP is off. */
		ask(q: Question): boolean {
			if (!hub || !state.running || !q.id) return false;
			controller.setAnswer(q.id, {status: 'pending', text: ''}); // before enqueue: delivery may be synchronous
			hub.enqueue({threadId: q.id, turn: 1, message: q.message, context: buildContext(q)});
			syncHub();
			return true;
		},

		/** Send a follow-up turn on an answered thread. False when MCP is off or the thread refuses. */
		followUp(id: string, message: string): boolean {
			if (!hub || !state.running) return false;
			const history = controller
				.turns(id)
				.map((t, i) => ({turn: i + 1, message: t.message, ...(t.answer ? {answer: t.answer.text} : {})}));
			const turn = controller.followUp(id, message);
			if (turn === undefined) return false;
			controller.setAnswer(id, {status: 'pending', text: ''}, turn); // before enqueue: delivery may be synchronous
			hub.enqueue({threadId: id, turn, message, followUp: true, history});
			syncHub();
			return true;
		},

		async refreshRegistration(): Promise<void> {
			const e = ep;
			if (!e) return;
			await Promise.all(
				integrations.map(async (i) => {
					if (!i.isRegistered) return;
					try {
						const r = await i.isRegistered(e);
						setInt(i.id, {registered: r.registered, stale: r.stale ?? false, note: undefined});
					} catch (err) {
						setInt(i.id, {note: maskTok(String((err as Error)?.message ?? err))});
					}
				}),
			);
		},

		async register(id: string): Promise<void> {
			const i = integrations.find((x) => x.id === id);
			if (!i) return;
			if (!i.canRegister || !i.register) return setInt(id, {note: 'copy-paste only'});
			if (!ep) return setInt(id, {note: 'start MCP first'});
			setInt(id, {busy: true, note: undefined});
			let note: string;
			try {
				const r = await i.register(ep);
				note = maskTok(r.message);
				if (r.ok && r.needsRestart) note += '; restart the agent session, then paste the watch prompt';
			} catch (err) {
				note = maskTok(String((err as Error)?.message ?? err));
			}
			setInt(id, {busy: false, note});
			await api.refreshRegistration();
			if (state.integrations.find((x) => x.id === id)?.note === undefined) setInt(id, {note});
		},

		async unregister(id: string): Promise<void> {
			const i = integrations.find((x) => x.id === id);
			if (!i) return;
			if (!i.unregister) return setInt(id, {note: 'copy-paste only'});
			setInt(id, {busy: true, note: undefined});
			let note: string;
			try {
				note = maskTok((await i.unregister()).message);
			} catch (err) {
				note = maskTok(String((err as Error)?.message ?? err));
			}
			setInt(id, {busy: false, note});
			await api.refreshRegistration();
			if (state.integrations.find((x) => x.id === id)?.note === undefined) setInt(id, {note});
		},

		/** Register command with the live endpoint; null when MCP is off. masked hides the token. */
		commandText(id: string, masked = false): string | null {
			const i = integrations.find((x) => x.id === id);
			if (!i || !ep) return null;
			const t = i.registerCommand(ep);
			return masked ? maskTok(t) : t;
		},
		watchText(id: string, masked = false): string | null {
			const i = integrations.find((x) => x.id === id);
			if (!i || !ep) return null;
			const t = i.watchPrompt(ep);
			return masked ? maskTok(t) : t;
		},
		copy(text: string) {
			clip(text);
		},
	};
	return api;
}
