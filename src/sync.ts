// Test seam for black-box e2e runs, active only with XPLAIN_SYNC=1 (contract: e2e/README.md, spec Test seams).
// Input barriers are never keys. Idle barrier ESC[9999~ is answered on stdout with OSC
// `ESC]7770;idle;<n>;<reqs>;<done>BEL` once all input before it is handled, tracked async work has settled and the
// resulting frame is written; frame barrier ESC[9998~ (`frame;...`) does not wait for tracked work. <reqs>/<done>:
// MCP HTTP requests received / finished so far. Off: all no-ops.
import {PassThrough} from 'node:stream';

export const SYNC = process.env.XPLAIN_SYNC === '1';
const BARRIERS = {'\x1b[9999~': 'idle', '\x1b[9998~': 'frame'} as const;
const BLEN = 7; // every barrier is 7 bytes
const KITTY_ESC = '\x1b[27u'; // unambiguous Escape key (a lone ESC waits for an escape-sequence timeout)

let pending = 0;
let idle: (() => void)[] = [];
let httpReceived = 0;
let httpDone = 0;

/** Counts one MCP HTTP request as received (call when the request arrives); returns its idempotent done callback. */
export function httpRequest(): () => void {
	httpReceived++;
	let d = false;
	return () => {
		if (!d) httpDone++;
		d = true;
	};
}

/** Counts p as pending work until it settles, plus one macrotask so its handlers' state updates land first. */
export function track<T>(p: Promise<T>): Promise<T> {
	if (!SYNC) return p;
	pending++;
	const done = () =>
		void setImmediate(() => {
			if (--pending > 0) return;
			const w = idle;
			idle = [];
			w.forEach((f) => f());
		});
	p.then(done, done);
	return p;
}
export const tracked =
	<A extends unknown[], R>(f: (...a: A) => Promise<R>) =>
	(...a: A): Promise<R> =>
		track(f(...a));

type Ink = {waitUntilRenderFlush(): Promise<void>; waitUntilExit(): Promise<unknown>};

// index where a possible barrier prefix starts at the end of s (s.length if none)
const holdFrom = (s: string) => {
	for (let k = Math.max(0, s.length - BLEN + 1); k < s.length; k++)
		if (Object.keys(BARRIERS).some((b) => b.startsWith(s.slice(k)))) return k;
	return s.length;
};
// first barrier in s: [index, kind]
const findBarrier = (s: string): [number, 'idle' | 'frame'] | undefined => {
	let best: [number, 'idle' | 'frame'] | undefined;
	for (const [b, kind] of Object.entries(BARRIERS)) {
		const i = s.indexOf(b);
		if (i >= 0 && (!best || i < best[0])) best = [i, kind];
	}
	return best;
};
const tick = () => new Promise<void>((r) => setImmediate(r));

/** Sync mode: render options (filtered stdin, render counter) and attach(ink) to start answering barriers. */
export function createSync() {
	if (!SYNC) return undefined;
	const src = process.stdin;
	const input = Object.assign(new PassThrough(), {
		isTTY: src.isTTY,
		setRawMode(m: boolean) {
			src.setRawMode?.(m);
			return input;
		},
		ref() {
			src.ref();
			return input;
		},
		unref() {
			src.unref();
			return input;
		},
	});
	let renders = 0;
	let ink: Ink | undefined;
	let closed = false;
	let busy = false;
	let buf = '';
	let n = 0;
	// frame written, nothing pending: two calm rounds in a row (timer-driven renders like spinners only retry a round)
	// frame barrier: same, but tracked work may stay pending
	const settle = async (waitPending: boolean) => {
		for (let calm = 0; calm < 2 && !closed;) {
			if (input.readableLength > 0) {
				await tick();
				calm = 0;
			} else if (waitPending && pending > 0) {
				await new Promise<void>((r) => idle.push(r));
				calm = 0;
			} else {
				const r0 = renders;
				await ink!.waitUntilRenderFlush();
				await tick();
				calm = renders === r0 && (!waitPending || pending === 0) && input.readableLength === 0 ? calm + 1 : 0;
			}
		}
	};
	const pump = async () => {
		if (busy || !ink) return;
		busy = true;
		try {
			while (!closed) {
				const found = findBarrier(buf);
				if (!found) {
					const k = holdFrom(buf);
					if (k > 0) input.write(buf.slice(0, k));
					buf = buf.slice(k);
					return;
				}
				const [i, kind] = found;
				let seg = buf.slice(0, i);
				buf = buf.slice(i + BLEN);
				if (seg.endsWith('\x1b')) seg = seg.slice(0, -1) + KITTY_ESC; // a barrier ends a pending ESC
				if (seg) input.write(seg);
				await settle(kind === 'idle');
				if (!closed) process.stdout.write(`\x1b]7770;${kind};${++n};${httpReceived};${httpDone}\x07`);
			}
		} finally {
			busy = false;
		}
	};
	const onData = (d: string) => {
		buf += d;
		void pump();
	};
	src.setEncoding('utf8');
	src.on('data', onData);
	return {
		options: {stdin: input as unknown as NodeJS.ReadStream, onRender: () => void renders++},
		attach(i: Ink) {
			ink = i;
			const close = () => {
				closed = true;
				src.off('data', onData);
				src.pause();
				src.unref();
			};
			void i.waitUntilExit().then(close, close);
			void pump();
		},
	};
}
