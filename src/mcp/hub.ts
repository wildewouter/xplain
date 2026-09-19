export const MAX_TEXT = 20000;

export type HistoryEntry = {turn: number; message: string; answer?: string};
export type Question = {
	threadId: string;
	turn: number;
	message: string;
	context?: string;
	followUp?: boolean;
	history?: HistoryEntry[];
};

export const MAX_HISTORY = 5;
export const MAX_HISTORY_TEXT = 4000;
export type Annotation = {file: string; line: number; text: string; side?: 'old' | 'new'};
export type PollResult = Question | null | 'closed';

export type HubEvent =
	| {type: 'answer'; threadId: string; turn: number; text: string}
	| {type: 'annotate'; file: string; line: number; text: string; side?: 'old' | 'new'}
	| {type: 'delivered'; threadId: string; clientId: string}
	| {type: 'clients'}
	| {type: 'enqueued'; threadId: string}
	| {type: 'closed'};

export type ClientInfo = {id: string; name: string; version: string; lastSeen: number; polling: boolean};
export type HubStatus = {running: boolean; clients: ClientInfo[]; pending: number; delivered: number};

export type Timers = {
	now: () => number;
	setTimeout: (fn: () => void, ms: number) => unknown;
	clearTimeout: (h: unknown) => void;
};

const realTimers: Timers = {
	now: () => Date.now(),
	setTimeout: (fn, ms) => setTimeout(fn, ms),
	clearTimeout: (h) => clearTimeout(h as NodeJS.Timeout),
};

const ANSI = /\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)|\x1b\[[0-?]*[ -/]*[@-~]|\x1b[@-Z\\-_]|\x1b/g;
// eslint-disable-next-line no-control-regex
const CTRL = /[\x00-\x08\x0b-\x1f\x7f-\x9f]/g;

/** Strip ANSI escapes and control chars (keep \n and \t), cap length. */
export function sanitize(s: unknown): string {
	const str = typeof s === 'string' ? s : String(s ?? '');
	return str.replace(ANSI, '').replace(CTRL, '').slice(0, MAX_TEXT);
}

function cleanQuestion(q: Question): Question {
	const out: Question = {...q, message: sanitize(q.message)};
	if (q.context !== undefined) out.context = sanitize(q.context);
	if (q.history) {
		out.history = q.history.slice(-MAX_HISTORY).map((h) => ({
			turn: h.turn,
			message: sanitize(h.message).slice(0, MAX_HISTORY_TEXT),
			...(h.answer !== undefined ? {answer: sanitize(h.answer).slice(0, MAX_HISTORY_TEXT)} : {}),
		}));
	}
	return out;
}

type Poller = {
	clientId: string;
	resolve: (r: PollResult) => void;
	timer: unknown;
	detach: () => void;
};

export type Hub = ReturnType<typeof createHub>;

export function createHub(opts: {timers?: Partial<Timers>} = {}) {
	const t: Timers = {...realTimers, ...opts.timers};
	const queue: Question[] = [];
	const pollers = new Map<string, Poller>();
	const clients = new Map<string, {name: string; version: string; lastSeen: number}>();
	const sticky = new Map<string, string>(); // threadId -> clientId
	const turns = new Map<string, number>(); // threadId -> last delivered turn
	const listeners = new Set<(e: HubEvent) => void>();
	let delivered = 0;
	let running = true;

	const emit = (e: HubEvent) => {
		for (const l of [...listeners]) {
			try {
				l(e);
			} catch {
				// listener errors must not break the hub
			}
		}
	};

	const touch = (id: string) => {
		const c = clients.get(id);
		if (c) c.lastSeen = t.now();
		else clients.set(id, {name: 'unknown', version: '', lastSeen: t.now()});
	};

	const finish = (p: Poller, r: PollResult) => {
		if (pollers.get(p.clientId) === p) pollers.delete(p.clientId);
		t.clearTimeout(p.timer);
		p.detach();
		p.resolve(r);
	};

	// Index of first queued question this client may take, or -1.
	const pick = (clientId: string): number => {
		for (let i = 0; i < queue.length; i++) {
			const s = sticky.get(queue[i]!.threadId);
			if (s === undefined || s === clientId || !pollers.has(s)) return i;
		}
		return -1;
	};

	const dispatch = () => {
		let progress = true;
		while (progress && queue.length > 0) {
			progress = false;
			for (const p of [...pollers.values()]) {
				const i = pick(p.clientId);
				if (i < 0) continue;
				const [q] = queue.splice(i, 1);
				deliver(p, q!);
				progress = true;
				break;
			}
		}
	};

	const deliver = (p: Poller, q: Question) => {
		delivered++;
		sticky.set(q.threadId, p.clientId);
		turns.set(q.threadId, q.turn);
		finish(p, q);
		emit({type: 'delivered', threadId: q.threadId, clientId: p.clientId});
		emit({type: 'clients'});
	};

	return {
		registerClient(id: string, name: string, version: string) {
			clients.set(id, {
				name: sanitize(name).slice(0, 200),
				version: sanitize(version).slice(0, 100),
				lastSeen: t.now(),
			});
			emit({type: 'clients'});
		},

		enqueue(q: Question) {
			if (!running) return;
			queue.push(cleanQuestion(q));
			emit({type: 'enqueued', threadId: q.threadId});
			dispatch();
		},

		/** Put a question that could not be sent back at the front of the queue. */
		requeue(q: Question) {
			if (!running) return;
			if (delivered > 0) delivered--;
			queue.unshift(cleanQuestion(q));
			dispatch();
		},

		poll(clientId: string, waitMs: number, signal?: AbortSignal): Promise<PollResult> {
			if (!running) return Promise.resolve('closed');
			if (signal?.aborted) return Promise.resolve(null);
			touch(clientId);
			const old = pollers.get(clientId);
			if (old) finish(old, null);
			return new Promise<PollResult>((resolve) => {
				const p: Poller = {clientId, resolve, timer: undefined, detach: () => {}};
				const onAbort = () => {
					if (pollers.get(clientId) === p) {
						finish(p, null);
						emit({type: 'clients'});
					}
				};
				signal?.addEventListener('abort', onAbort, {once: true});
				p.detach = () => signal?.removeEventListener('abort', onAbort);
				const i = pick(clientId);
				if (i >= 0) {
					const [q] = queue.splice(i, 1);
					pollers.set(clientId, p);
					deliver(p, q!);
					return;
				}
				p.timer = t.setTimeout(
					() => {
						if (pollers.get(clientId) === p) {
							touch(clientId);
							finish(p, null);
							emit({type: 'clients'});
						}
					},
					Math.max(0, waitMs),
				);
				pollers.set(clientId, p);
				emit({type: 'clients'});
			});
		},

		/** Returns false if the thread is unknown (never delivered). */
		answer(threadId: string, text: string): boolean {
			const turn = turns.get(threadId);
			if (turn === undefined) return false;
			emit({type: 'answer', threadId, turn, text: sanitize(text)});
			return true;
		},

		annotate(a: Annotation) {
			const line = Number.isFinite(a.line) ? Math.max(1, Math.floor(a.line)) : 1;
			emit({
				type: 'annotate',
				file: sanitize(a.file),
				line,
				text: sanitize(a.text),
				...(a.side ? {side: a.side} : {}),
			});
		},

		pending(): Question[] {
			return queue.map((q) => ({...q}));
		},

		status(): HubStatus {
			return {
				running,
				clients: [...clients].map(([id, c]) => ({id, ...c, polling: pollers.has(id)})),
				pending: queue.length,
				delivered,
			};
		},

		subscribe(listener: (e: HubEvent) => void): () => void {
			listeners.add(listener);
			return () => void listeners.delete(listener);
		},

		close() {
			if (!running) return;
			running = false;
			for (const p of [...pollers.values()]) finish(p, 'closed');
			emit({type: 'closed'});
			emit({type: 'clients'});
		},
	};
}
