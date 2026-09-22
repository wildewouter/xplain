import {execFile} from 'node:child_process';
import {lstat} from 'node:fs/promises';
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
	return parseDiff(raw).map((f) => {
		const to = clean(f.to);
		const from = clean(f.from);
		const binary = /^Binary files|^GIT binary patch/m.test(
			f.chunks.length ? '' : (raw.split(`diff --git`).find((s) => s.includes(f.to ?? f.from ?? '\0')) ?? ''),
		);
		const renamed = from && to && from !== to;
		return {
			path: to ?? from ?? '?',
			from: renamed ? from : undefined,
			adds: f.additions,
			dels: f.deletions,
			binary,
			note: binary
				? 'Binary file'
				: !f.chunks.length
					? renamed
						? 'Renamed, no content changes'
						: 'No textual changes'
					: undefined,
			hunks: f.chunks.map((c) => ({
				header: c.content,
				lines: c.changes.map((ch) =>
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

const MAX_UNTRACKED = 1024 * 1024;

function git(args: string[], cwd?: string, okCodes: number[] = [0]): Promise<string> {
	return new Promise((resolve, reject) =>
		execFile('git', args, {cwd, maxBuffer: 256 * 1024 * 1024, timeout: 60000}, (err, out, stderr) => {
			const code = (err as {code?: number} | null)?.code;
			err && !(typeof code === 'number' && okCodes.includes(code))
				? reject(new Error(stderr || err.message))
				: resolve(out);
		}),
	);
}

// Untracked (not ignored) regular files as all-added diffs. Never touches index/worktree.
async function untrackedDiff(cwd: string | undefined, full: boolean): Promise<string> {
	const names = (await git(['ls-files', '--others', '--exclude-standard', '-z'], cwd)).split('\0').filter(Boolean);
	const parts: string[] = [];
	for (let i = 0; i < names.length; i += 16)
		parts.push(
			...(await Promise.all(
				names.slice(i, i + 16).map(async (n) => {
					try {
						const st = await lstat(cwd ? `${cwd}/${n}` : n);
						if (!st.isFile() || st.size > MAX_UNTRACKED) return '';
						return await git(
							[
								'diff',
								'--no-index',
								'--no-color',
								'--no-ext-diff',
								...(full ? ['-U1000000'] : []),
								'--',
								'/dev/null',
								n,
							],
							cwd,
							[0, 1],
						);
					} catch {
						return '';
					}
				}),
			)),
		);
	return parts.join('');
}

// Rule: mode picks base args (all=HEAD, staged=--cached, unstaged=none). Extra git args
// are appended after --cached/none; for `all` they replace HEAD (legacy behavior).
// `all` with no extra args also includes untracked files (as fully added).
export async function loadDiff(mode: Mode, args: string[], cwd?: string, full = true): Promise<DiffFile[]> {
	const base = mode === 'staged' ? ['--cached'] : mode === 'unstaged' ? [] : args.length ? [] : ['HEAD'];
	const gitArgs = ['diff', '--no-color', '--no-ext-diff', ...(full ? ['-U1000000'] : []), ...base, ...args];
	const [out, extra] = await Promise.all([
		git(gitArgs, cwd),
		mode === 'all' && !args.length ? untrackedDiff(cwd, full) : '',
	]);
	return parse(out + extra);
}

export function listFiles(cwd?: string): Promise<string[]> {
	return new Promise((resolve, reject) =>
		execFile(
			'git',
			['ls-files', '--cached', '--others', '--exclude-standard'],
			{cwd, maxBuffer: 256 * 1024 * 1024},
			(err, out, stderr) =>
				err ? reject(new Error(stderr || err.message)) : resolve([...new Set(out.split('\n').filter(Boolean))].sort()),
		),
	);
}
