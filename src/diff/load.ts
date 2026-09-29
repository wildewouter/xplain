import {execFile} from 'node:child_process';
import {lstat, stat} from 'node:fs/promises';
import parseDiff from 'parse-diff';
import {failMsg} from '../errors.js';

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

const C_ESC: Record<string, number> = {a: 7, b: 8, t: 9, n: 10, v: 11, f: 12, r: 13, '"': 34, '\\': 92};

/** Decode a git C-quoted string starting at s[i] === '"'. Returns the text and the index after the closing quote. */
function readQuoted(s: string, i: number): [string, number] {
	const parts: Buffer[] = [];
	let j = i + 1;
	while (j < s.length && s[j] !== '"') {
		if (s[j] === '\\' && j + 1 < s.length) {
			const oct = /^[0-7]{3}/.exec(s.slice(j + 1, j + 4));
			const c = s[j + 1]!;
			parts.push(Buffer.from(oct ? [parseInt(oct[0], 8)] : C_ESC[c] !== undefined ? [C_ESC[c]] : Buffer.from(c)));
			j += oct ? 4 : 2;
			continue;
		}
		const ch = String.fromCodePoint(s.codePointAt(j)!);
		parts.push(Buffer.from(ch));
		j += ch.length;
	}
	return [Buffer.concat(parts).toString('utf8'), j + 1];
}

// One path argument of a header line (`--- a/x`, `rename from x`): C-quoted or raw (git appends a TAB to names with spaces).
const pathArg = (s: string) => (s.startsWith('"') ? readQuoted(s, 0)[0] : s.replace(/\t$/, ''));

// Git's own prefix, stripped once: `a/` old side, `b/` new side. `/dev/null` means no file on that side.
const strip = (p: string | undefined, prefix: string): string | null | undefined =>
	p === undefined ? undefined : p === '/dev/null' ? null : p.startsWith(prefix) ? p.slice(prefix.length) : p;

// `diff --git <a/old> <b/new>`: both sides, possibly C-quoted. Unquoted names with spaces are ambiguous, so the
// common case (same path both sides) is split in the middle.
function gitHeader(rest: string): [string | undefined, string | undefined] {
	if (rest.startsWith('"')) {
		const [a, k] = readQuoted(rest, 0);
		return [a, pathArg(rest.slice(k + 1))];
	}
	const q = rest.lastIndexOf(' "');
	if (q > 0 && rest.endsWith('"')) return [rest.slice(0, q), readQuoted(rest, q + 1)[0]];
	const n = (rest.length - 5) / 2;
	if (Number.isInteger(n) && n > 0 && rest.slice(2, 2 + n) === rest.slice(5 + n) && rest[2 + n] === ' ')
		return [rest.slice(0, 2 + n), rest.slice(3 + n)];
	const m = rest.indexOf(' b/');
	return m > 0 ? [rest.slice(0, m), rest.slice(m + 1)] : [rest, rest];
}

type Paths = {from?: string; to?: string; binary: boolean; renamed: boolean};

// Paths and binary flag of one file section, from its header lines only (before the first hunk).
function header(section: string): Paths {
	let from: string | null | undefined;
	let to: string | null | undefined;
	let rnFrom: string | undefined;
	let rnTo: string | undefined;
	let hdr: [string | undefined, string | undefined] = [undefined, undefined];
	let added = false;
	let deleted = false;
	let binary = false;
	for (const line of section.split('\n')) {
		if (line.startsWith('@@')) break;
		if (line.startsWith('diff --git ')) hdr = gitHeader(line.slice(11));
		else if (line.startsWith('diff --cc ') || line.startsWith('diff --combined '))
			hdr = [undefined, pathArg(line.slice(line.indexOf(' ', 5) + 1))];
		else if (line.startsWith('--- ')) from = strip(pathArg(line.slice(4)), 'a/');
		else if (line.startsWith('+++ ')) to = strip(pathArg(line.slice(4)), 'b/');
		else if (/^(rename|copy) from /.test(line)) rnFrom = pathArg(line.slice(line.indexOf(' from ') + 6));
		else if (/^(rename|copy) to /.test(line)) rnTo = pathArg(line.slice(line.indexOf(' to ') + 4));
		else if (line.startsWith('new file mode ')) added = true;
		else if (line.startsWith('deleted file mode ')) deleted = true;
		else if (/^Binary files .* differ$/.test(line) || line === 'GIT binary patch') binary = true;
	}
	const hFrom = strip(hdr[0], 'a/') ?? undefined;
	const hTo = strip(hdr[1], 'b/') ?? undefined;
	const oldP = rnFrom ?? (from === null ? undefined : (from ?? (added ? undefined : hFrom)));
	const newP = rnTo ?? (to === null ? undefined : (to ?? (deleted ? undefined : hTo)));
	return {
		from: oldP,
		to: newP,
		binary,
		renamed: oldP !== undefined && newP !== undefined && oldP !== newP,
	};
}

export function parse(raw: string): DiffFile[] {
	// One section per file: content lines always start with ' ', '+', '-' or '\\', so `diff ` only starts a file.
	const starts = [...raw.matchAll(/^diff /gm)].map((m) => m.index);
	return starts.map((at, i) => {
		const section = raw.slice(at, starts[i + 1] ?? raw.length);
		const f = parseDiff(section)[0];
		const h = header(section);
		const chunks = f?.chunks ?? [];
		return {
			path: h.to ?? h.from ?? '?',
			from: h.renamed ? h.from : undefined,
			adds: f?.additions ?? 0,
			dels: f?.deletions ?? 0,
			binary: h.binary,
			note: h.binary
				? 'Binary file'
				: !chunks.length
					? h.renamed
						? 'Renamed, no content changes'
						: 'No textual changes'
					: undefined,
			hunks: chunks.map((c) => ({
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

// Runtime-neutral: git stderr verbatim; git never ran: `cannot run git: <reason>`; else `git failed`.
const gitErr = (err: Error & {code?: unknown; syscall?: unknown}, stderr: string): Error =>
	new Error(
		stderr ||
			(typeof err.code === 'string' && typeof err.syscall === 'string' && err.syscall.startsWith('spawn')
				? failMsg('cannot run git', err)
				: typeof err.code === 'number'
					? `git failed (exit ${err.code})`
					: 'git failed'),
	);

function git(args: string[], cwd?: string, okCodes: number[] = [0]): Promise<string> {
	return new Promise((resolve, reject) =>
		execFile('git', args, {cwd, maxBuffer: 256 * 1024 * 1024, timeout: 60000}, (err, out, stderr) => {
			const code = (err as {code?: number} | null)?.code;
			err && !(typeof code === 'number' && okCodes.includes(code)) ? reject(gitErr(err, stderr)) : resolve(out);
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

// `--cwd` must be an existing directory before git is spawned (runtimes word a bad spawn cwd differently).
async function checkDir(dir: string): Promise<void> {
	let e: unknown;
	try {
		if (!(await stat(dir)).isDirectory()) e = {code: 'ENOTDIR'};
	} catch (err) {
		e = err;
	}
	if (e) throw new Error(failMsg(`cannot open directory ${dir}`, e));
}

// Rule: mode picks base args (all=HEAD, staged=--cached, unstaged=none). Extra git args
// are appended after --cached/none; for `all` they replace HEAD (legacy behavior).
// `all` with no extra args also includes untracked files (as fully added).
export async function loadDiff(mode: Mode, args: string[], cwd?: string, full = true): Promise<DiffFile[]> {
	const base = mode === 'staged' ? ['--cached'] : mode === 'unstaged' ? [] : args.length ? [] : ['HEAD'];
	const gitArgs = ['diff', '--no-color', '--no-ext-diff', ...(full ? ['-U1000000'] : []), ...base, ...args];
	if (cwd !== undefined) await checkDir(cwd);
	// Both settle before deciding: a failure always reports `git diff`'s own error (never the untracked listing's),
	// whichever finishes first. A failing listing alone just means no untracked files.
	const [out, extra] = await Promise.allSettled([
		git(gitArgs, cwd),
		mode === 'all' && !args.length ? untrackedDiff(cwd, full) : '',
	]);
	if (out.status === 'rejected') throw out.reason;
	return parse(out.value + (extra.status === 'fulfilled' ? extra.value : ''));
}

export function listFiles(cwd?: string): Promise<string[]> {
	return new Promise((resolve, reject) =>
		execFile(
			'git',
			['ls-files', '--cached', '--others', '--exclude-standard'],
			{cwd, maxBuffer: 256 * 1024 * 1024},
			(err, out, stderr) =>
				err ? reject(gitErr(err, stderr)) : resolve([...new Set(out.split('\n').filter(Boolean))].sort()),
		),
	);
}
