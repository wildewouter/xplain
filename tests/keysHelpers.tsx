import {makePress, settle} from './helpers.js';
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
	const base = join(process.env.CLAUDE_JOB_DIR ?? '.', 'tmp');
	mkdirSync(base, {recursive: true});
	const d = mkdtempSync(join(base, prefix + '-'));
	dirs.push(d);
	return d;
};
export const finish = (): never => {
	for (const d of dirs) rmSync(d, {recursive: true, force: true});
	process.exit(fail ? 1 : 0);
};

type R = {stdin: {write: (s: string) => unknown}; lastFrame: () => string | undefined};
/** press(key, until?) for an ink-testing-library instance; without `until`, waits briefly for a frame change. */
export const keyPress = (r: R) => {
	const press = makePress(r.stdin, r.lastFrame, {changeTimeout: 40});
	return async (k: string): Promise<void> => {
		await press(k);
		await settle(15);
	};
};
