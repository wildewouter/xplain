import {mkdirSync} from 'node:fs';

// Runtime-neutral user messages: `<action> <target>: <reason>`. Runtime error texts are never shown verbatim;
// only the error code picks the reason word.
const REASONS: Record<string, string> = {
	ENOENT: 'not found',
	EACCES: 'permission denied',
	EPERM: 'permission denied',
	EISDIR: 'is a directory',
	ENOTDIR: 'not a directory',
};

export function reason(e: unknown): string {
	const code = (e as {code?: unknown} | null | undefined)?.code;
	return (typeof code === 'string' && REASONS[code]) || 'failed';
}

/** `<what>: <reason>`, e.g. failMsg('cannot read a.txt', err) -> `cannot read a.txt: not found`. */
export const failMsg = (what: string, e: unknown): string => `${what}: ${reason(e)}`;

/** mkdir -p. An existing non-directory on the path fails as ENOTDIR (runtimes report EEXIST for the last part). */
export function ensureDir(dir: string, mode?: number): void {
	try {
		mkdirSync(dir, {recursive: true, ...(mode !== undefined ? {mode} : {})});
	} catch (e) {
		throw (e as {code?: unknown})?.code === 'EEXIST'
			? Object.assign(new Error('not a directory'), {code: 'ENOTDIR'})
			: e;
	}
}
