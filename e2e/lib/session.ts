// One app process in a PTY, mirrored into a headless xterm. Barrier protocol: e2e/README.md.
import pty from '@lydell/node-pty';
import xterm from '@xterm/headless';
import {parseKeys} from './keys.js';

const {Terminal} = xterm;
type Term = InstanceType<typeof Terminal>;

export const BARRIERS = {idle: '\x1b[9999~', frame: '\x1b[9998~'} as const;
export type BarrierKind = keyof typeof BARRIERS;
const OSC_SYNC = 7770;
/** HTTP requests the runner has on the wire: `sent` = fully written to the app, `done` = answered or aborted by us. */
export type HttpCounts = () => {sent: number; done: number};

/** Rejects waits once the scenario guard expires (reports hangs; never used as a wait). */
export class Guard {
	expired = false;
	private fns = new Set<() => void>();
	private t: NodeJS.Timeout;
	constructor(readonly ms: number) {
		this.t = setTimeout(() => {
			this.expired = true;
			this.fns.forEach((f) => f());
		}, ms);
	}
	on(f: () => void) {
		this.fns.add(f);
		return () => void this.fns.delete(f);
	}
	clear() {
		clearTimeout(this.t);
	}
}

export class HangError extends Error {}
export class ExitError extends Error {}

export type Cell = {
	chars: string;
	fg: string | number;
	bg: string | number;
	bold: boolean;
	dim: boolean;
	italic: boolean;
	underline: boolean;
	inverse: boolean;
	strikethrough: boolean;
};

export class Session {
	readonly term: Term;
	private p: pty.IPty;
	exit?: {code: number; signal?: number};
	ready = false;
	replies = 0;
	sent = 0;
	/** request counters from the last reply (undefined: app reports none) */
	appHttp?: {received: number; done: number};
	protocolError?: string;
	clipboard: string[] = [];
	private subs = new Set<() => void>();
	private hold?: () => void;

	constructor(
		cmd: string,
		args: string[],
		o: {cwd: string; env: Record<string, string>; cols: number; rows: number; stderrFile?: string},
		private guard: Guard,
		private http: HttpCounts = () => ({sent: 0, done: 0}),
	) {
		this.term = new Terminal({cols: o.cols, rows: o.rows, scrollback: 0, allowProposedApi: true});
		// async handler (returns a Promise) pauses the parser; supported at runtime, missing from the headless typings
		this.term.parser.registerOscHandler(OSC_SYNC, ((data: string) => {
			if (data === 'ready') this.ready = true;
			else {
				// idle;<n> (legacy) or (idle|frame);<n>;<received>;<done>
				const m = /^(idle|frame);(\d+)(?:;(\d+);(\d+))?$/.exec(data);
				const n = m ? Number(m[2]) : NaN;
				const want = this.kinds[this.replies];
				if (!m || n !== this.replies + 1 || m[1] !== want)
					this.protocolError ??= `bad sync reply OSC ${OSC_SYNC};${data} (expected ${want ?? 'none'};${this.replies + 1})`;
				if (n > this.replies) this.replies = n;
				if (m?.[3] !== undefined) this.appHttp = {received: Number(m[3]), done: Number(m[4])};
				// freeze parsing right at the idle point: later output (spinner frames etc.) waits until the next input
				this.notify();
				return new Promise<boolean>((r) => (this.hold = () => r(true)));
			}
			this.notify();
			return true;
		}) as unknown as (data: string) => boolean);
		this.term.parser.registerOscHandler(52, (data) => {
			const b64 = data.slice(data.indexOf(';') + 1);
			if (b64 !== '?') this.clipboard.push(Buffer.from(b64, 'base64').toString('utf8'));
			this.notify();
			return false; // let xterm handle it too
		});
		// stty before exec: no echo / no line buffering, so input sent before the app sets raw mode is neither echoed nor held.
		// Absolute path: a scenario `env.PATH` may leave out /bin
		const q = (x: string) => `'${x.replace(/'/g, `'\\''`)}'`;
		const script = `/bin/stty -echo -icanon min 1 time 0 2>/dev/null; printf '\\033]${OSC_SYNC};ready\\007'; exec ${cmd} "$@"${o.stderrFile ? ` 2>${q(o.stderrFile)}` : ''}`;
		this.p = pty.spawn('/bin/sh', ['-c', script, 'xplain', ...args], {
			name: o.env.TERM ?? 'xterm-256color',
			cols: o.cols,
			rows: o.rows,
			cwd: o.cwd,
			env: o.env,
		});
		this.p.onData((d) => this.term.write(d, () => this.notify()));
		this.p.onExit(({exitCode, signal}) => {
			// let xterm finish parsing what was already received
			this.term.write('', () => {
				this.exit = {code: exitCode, ...(signal ? {signal} : {})};
				this.notify();
			});
		});
	}

	private notify() {
		for (const f of [...this.subs]) f();
	}

	/** Resolve once cond() is truthy; 'exit' if the process exits first and exitOk; rejects on exit or guard expiry. */
	wait(cond: () => boolean, what: string, exitOk = false): Promise<'ok' | 'exit'> {
		return new Promise((resolve, reject) => {
			let offGuard = () => {};
			const done = () => {
				this.subs.delete(check);
				offGuard();
			};
			const check = () => {
				if (this.protocolError) {
					done();
					reject(new Error(this.protocolError));
				} else if (cond()) {
					done();
					resolve('ok');
				} else if (this.exit) {
					done();
					if (exitOk) resolve('exit');
					else reject(new ExitError(`process exited (code ${this.exit.code}) while waiting: ${what}`));
				}
			};
			offGuard = this.guard.on(() => {
				done();
				reject(new HangError(`hang after ${this.guard.ms}ms: ${what}`));
			});
			this.subs.add(check);
			if (this.guard.expired) {
				done();
				reject(new HangError(`hang after ${this.guard.ms}ms: ${what}`));
			} else check();
		});
	}

	waitReady() {
		return this.wait(() => this.ready, 'no ready marker from launcher');
	}

	/** kind of every barrier sent, in order */
	private kinds: BarrierKind[] = [];
	/**
	 * Send a barrier (after `prefix` in the same write) and wait for its reply. While the app reports fewer HTTP
	 * requests received / finished than the runner has sent / finished, send another one (event driven, no sleeps).
	 */
	async barrier(exitOk = false, kind: BarrierKind = 'idle', prefix = ''): Promise<'ok' | 'exit'> {
		for (;;) {
			const n = ++this.sent;
			this.kinds.push(kind);
			this.write(prefix + BARRIERS[kind]);
			prefix = '';
			const r = await this.wait(() => this.replies >= n, `no ${kind} barrier reply #${n}`, exitOk);
			if (r === 'exit') return r;
			const h = this.http();
			if (!this.appHttp || (this.appHttp.received >= h.sent && this.appHttp.done >= h.done)) return 'ok';
		}
	}
	/** Write raw bytes without a barrier. */
	send(bytes: string) {
		this.write(bytes);
	}
	/** Resume parsing output held since the last idle reply. */
	release() {
		const h = this.hold;
		this.hold = undefined;
		h?.();
	}
	private write(s: string) {
		this.release();
		if (this.exit) return;
		try {
			this.p.write(s);
		} catch {
			// pty gone: the wait reports the exit
		}
	}

	/** Type keys one at a time, each followed by a barrier of `kind` ('none': no barrier, no wait). */
	async keys(s: string, exitOkAtEnd = false, kind: BarrierKind | 'none' = 'idle'): Promise<'ok' | 'exit'> {
		const ks = parseKeys(s);
		for (let i = 0; i < ks.length; i++) {
			const last = i === ks.length - 1;
			if (this.exit) {
				if (last && exitOkAtEnd) return 'exit';
				throw new ExitError(`process exited (code ${this.exit.code}) before key ${ks[i]!.label}`);
			}
			const bytes = ks[i]!.bytes(this.term.modes.applicationCursorKeysMode);
			if (kind === 'none') {
				this.write(bytes);
				continue;
			}
			const r = await this.barrier(last && exitOkAtEnd, kind, bytes);
			if (r === 'exit') return r;
		}
		return 'ok';
	}

	waitExit() {
		this.release();
		return this.wait(() => !!this.exit, 'no exit');
	}

	kill() {
		this.release();
		if (this.exit) return;
		try {
			this.p.kill('SIGKILL');
		} catch {}
	}

	// ---- screen ----
	get rows() {
		return this.term.rows;
	}
	get cols() {
		return this.term.cols;
	}
	private line(y: number) {
		const b = this.term.buffer.active;
		return b.getLine(b.viewportY + y);
	}
	/** Row text (right-trimmed) plus char-index -> column map. */
	rowInfo(y: number): {text: string; colOf: number[]} {
		const l = this.line(y);
		let text = '';
		const colOf: number[] = [];
		if (!l) return {text, colOf};
		for (let x = 0; x < this.term.cols; x++) {
			const c = l.getCell(x);
			if (!c || c.getWidth() === 0) continue;
			const ch = c.getChars() || ' ';
			for (let k = 0; k < ch.length; k++) colOf.push(x);
			text += ch;
		}
		const t = text.replace(/\s+$/, '');
		return {text: t, colOf: colOf.slice(0, t.length)};
	}
	lines(): string[] {
		return Array.from({length: this.term.rows}, (_, y) => this.rowInfo(y).text);
	}
	screen(): string {
		return this.lines().join('\n');
	}
	cell(row: number, col: number): Cell | undefined {
		const c = this.line(row)?.getCell(col);
		if (!c) return undefined;
		const color = (rgb: boolean, pal: boolean, v: number) =>
			rgb ? '#' + v.toString(16).padStart(6, '0') : pal ? v : 'default';
		return {
			chars: c.getChars(),
			fg: color(c.isFgRGB(), c.isFgPalette(), c.getFgColor()),
			bg: color(c.isBgRGB(), c.isBgPalette(), c.getBgColor()),
			bold: !!c.isBold(),
			dim: !!c.isDim(),
			italic: !!c.isItalic(),
			underline: !!c.isUnderline(),
			inverse: !!c.isInverse(),
			strikethrough: !!c.isStrikethrough(),
		};
	}
	cursor() {
		const b = this.term.buffer.active;
		const core = (this.term as unknown as {_core?: {coreService?: {isCursorHidden?: boolean}}})._core;
		return {row: b.cursorY, col: b.cursorX, visible: !core?.coreService?.isCursorHidden};
	}
}
