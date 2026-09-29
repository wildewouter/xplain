import {makePress, settle, waitFor} from './helpers.js';
import {mkdirSync, mkdtempSync, rmSync} from 'node:fs';
import {join} from 'node:path';

export const cwd = process.argv[2]!;
export const tick = () => new Promise((r) => setTimeout(r, 80));
let fail = 0;
export const ok = (n: string, c: boolean) => {
	console.log(c ? 'PASS' : 'FAIL', n);
	if (!c) fail++;
};
const dirs: string[] = [];
/** Unique temp dir (removed by finish()). */
export const tmpDir = (prefix: string): string => {
	const base = join(process.env.TMPDIR ?? '.', 'tmp');
	mkdirSync(base, {recursive: true});
	const d = mkdtempSync(join(base, prefix + '-'));
	dirs.push(d);
	return d;
};
export const finish = (): never => {
	for (const d of dirs) rmSync(d, {recursive: true, force: true});
	process.exit(fail ? 1 : 0);
};

export type R = {stdin: {write: (s: string) => unknown}; lastFrame: () => string | undefined};
/** press(key, until?) for an ink-testing-library instance; without `until`, waits briefly for a frame change. */
export const keyPress = (r: R) => {
	const press = makePress(r.stdin, r.lastFrame, {changeTimeout: 40});
	return async (k: string): Promise<void> => {
		await press(k);
		await settle(15);
	};
};

const turn = () => new Promise<void>((r) => setImmediate(r));
/**
 * Wait (max `timeout` ms) until the frame matches p and still does once rendering is calm (no new frame for two
 * loop turns). Async loads commit new rows first; cursor/scroll reset effects (and onCursor) land turns later,
 * and a key sent in between is overridden by them. Wall-clock sleeps are not enough under CPU load.
 */
export const until = async (
	r: Pick<R, 'lastFrame'> & {frames?: readonly string[]},
	p: string | RegExp | ((f: string) => boolean),
	timeout = 3000,
): Promise<boolean> => {
	const f = () => r.lastFrame() ?? '';
	const m = () => {
		const s = f();
		return typeof p === 'string' ? s.includes(p) : p instanceof RegExp ? p.test(s) : p(s);
	};
	const mark = () => `${r.frames?.length ?? 0}\0${f()}`;
	const end = Date.now() + timeout;
	while (await waitFor(m, {timeout: Math.max(0, end - Date.now())})) {
		for (let calm = 0, k = mark(); calm < 2 && Date.now() < end;) {
			await turn();
			const k2 = mark();
			calm = k2 === k ? calm + 1 : 0;
			k = k2;
		}
		if (m()) return true;
	}
	return false;
};
/** F, type q, Enter once the (async) file list has hits, then wait for the (async) browse view. */
export const browse = async (r: R, q: string): Promise<boolean> => {
	const w = keyPress(r);
	await w('F');
	await w(q);
	await until(r, /Search \([1-9]\d*\//);
	await w('\r');
	return until(r, '[browse]');
};
/** Wait for the initial (async) diff load: file counter or the no-changes screen. */
export const booted = (r: Pick<R, 'lastFrame'>): Promise<boolean> => until(r, /\[\d+\/\d+\]|No changes/);
