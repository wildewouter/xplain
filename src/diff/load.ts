import {execFile} from 'node:child_process';
import parseDiff from 'parse-diff';

export type Line = {type: 'add' | 'del' | 'normal'; oldNo?: number; newNo?: number; text: string};
export type Hunk = {header: string; lines: Line[]};
export type DiffFile = {
	path: string;
	from?: string;
	adds: number;
	dels: number;
	binary: boolean;
	note?: string;
	hunks: Hunk[];
};

const clean = (p?: string) => (p && p !== '/dev/null' ? p.replace(/^[ab]\//, '') : undefined);

export function parse(raw: string): DiffFile[] {
	return parseDiff(raw).map(f => {
		const to = clean(f.to);
		const from = clean(f.from);
		const binary = /^Binary files|^GIT binary patch/m.test(f.chunks.length ? '' : raw.split(`diff --git`).find(s => s.includes(f.to ?? f.from ?? '\0')) ?? '');
		const renamed = from && to && from !== to;
		return {
			path: to ?? from ?? '?',
			from: renamed ? from : undefined,
			adds: f.additions,
			dels: f.deletions,
			binary,
			note: binary ? 'Binary file' : !f.chunks.length ? (renamed ? 'Renamed, no content changes' : 'No textual changes') : undefined,
			hunks: f.chunks.map(c => ({
				header: c.content,
				lines: c.changes.map(ch =>
					ch.type === 'add'
						? {type: 'add', newNo: ch.ln, text: ch.content.slice(1)}
						: ch.type === 'del'
							? {type: 'del', oldNo: ch.ln, text: ch.content.slice(1)}
							: {type: 'normal', oldNo: ch.ln1, newNo: ch.ln2, text: ch.content.slice(1)},
				),
			})),
		} as DiffFile;
	});
}

export const MODES = ['all', 'staged', 'unstaged'] as const;
export type Mode = (typeof MODES)[number];

// Rule: mode picks base args (all=HEAD, staged=--cached, unstaged=none). Extra git args
// are appended after --cached/none; for `all` they replace HEAD (legacy behavior).
export function loadDiff(mode: Mode, args: string[], cwd?: string): Promise<DiffFile[]> {
	const base = mode === 'staged' ? ['--cached'] : mode === 'unstaged' ? [] : args.length ? [] : ['HEAD'];
	const gitArgs = ['diff', '--no-color', '--no-ext-diff', ...base, ...args];
	return new Promise((resolve, reject) =>
		execFile('git', gitArgs, {cwd, maxBuffer: 256 * 1024 * 1024}, (err, out, stderr) =>
			err ? reject(new Error(stderr || err.message)) : resolve(parse(out)),
		),
	);
}
