// e2e runner: npx tsx e2e/run.ts [--repeat N] [--jobs N] [--list] [filter...]
// filter: spec id (exact or prefix, e.g. F-NAV), or a substring / glob of the scenario path.
import {readdirSync} from 'node:fs';
import {availableParallelism} from 'node:os';
import {join, relative} from 'node:path';
import {loadScenario, type Scenario} from './lib/scenario.js';
import {DEFAULT_BIN, ROOT, runScenario, type Result} from './lib/runner.js';

// node-pty logs EIO when a key/barrier write races the app's exit; the exit itself is reported by the scenario
const cerr = console.error;
console.error = (...a: unknown[]) => {
	if (a[0] === 'Unhandled pty write error' && (a[1] as {code?: string})?.code === 'EIO') return;
	cerr(...a);
};

const argv = process.argv.slice(2);
const opt = (name: string, def: number) => {
	const i = argv.indexOf(name);
	if (i < 0) return def;
	const v = Number(argv[i + 1]);
	argv.splice(i, 2);
	return v;
};
const repeat = opt('--repeat', Number(process.env.E2E_REPEAT ?? 1));
const jobs = opt('--jobs', Number(process.env.E2E_JOBS ?? Math.max(2, availableParallelism())));
const list = argv.includes('--list');
const filters = argv.filter((a) => a !== '--list');
const bin = process.env.XPLAIN_BIN || DEFAULT_BIN;
const timeoutMs = Number(process.env.E2E_TIMEOUT ?? 30000);
const keep = process.env.E2E_KEEP === '1';

const dir = join(ROOT, 'e2e', 'scenarios');
const files = (readdirSync(dir, {recursive: true}) as string[])
	.filter((f) => /\.ya?ml$/.test(f))
	.map((f) => join(dir, f))
	.sort();
const glob = (g: string) =>
	new RegExp(
		'^' +
			g
				.replace(/[.+^${}()|[\]\\]/g, '\\$&')
				.replace(/\*\*/g, '\0')
				.replace(/\*/g, '[^/]*')
				.replace(/\0/g, '.*')
				.replace(/\?/g, '.') +
			'$',
	);

type Job = {file: string; sc?: Scenario; loadErr?: string};
const all: Job[] = files.map((file) => {
	try {
		return {file, sc: loadScenario(file)};
	} catch (e) {
		return {file, loadErr: (e as Error).message};
	}
});
const rel = (f: string) => relative(ROOT, f);
const picked = all.filter(
	(j) =>
		!filters.length ||
		filters.some((f) => {
			const id = j.sc?.id ?? '';
			return (
				id === f ||
				(id.startsWith(f) && /^[A-Z]/.test(f)) ||
				rel(j.file).includes(f) ||
				glob(f).test(rel(j.file)) ||
				glob(f).test(id)
			);
		}),
);
if (list) {
	for (const j of picked) console.log(`${j.sc?.id ?? '?'}\t${rel(j.file)}\t${j.sc?.title ?? j.loadErr}`);
	process.exit(0);
}
if (!picked.length) {
	console.error(`no scenarios match ${filters.join(' ')}`);
	process.exit(2);
}

const queue = picked.flatMap((j) => Array.from({length: repeat}, () => j));
const results: {job: Job; r: Result}[] = [];
const t0 = Date.now();
let next = 0;
const worker = async () => {
	while (next < queue.length) {
		const job = queue[next++]!;
		const r: Result = job.sc
			? await runScenario(job.sc, {bin, timeoutMs, keep})
			: {ok: false, ms: 0, error: `LOAD: ${job.loadErr}`};
		results.push({job, r});
		const id = job.sc?.id ?? '?';
		if (r.ok) console.log(`PASS ${id} ${rel(job.file)} ${r.ms}ms`);
		else {
			console.log(`FAIL ${id} ${rel(job.file)} ${r.ms}ms${r.step !== undefined ? ` step ${r.step}` : ''}: ${r.error}`);
			if (r.stepText) console.log(`  step: ${r.stepText}`);
			if (r.dump) console.log(r.dump);
		}
	}
};
await Promise.all(Array.from({length: Math.min(jobs, queue.length)}, worker));

const byId = new Map<string, {pass: number; fail: number}>();
for (const {job, r} of results) {
	const k = job.sc?.id ?? rel(job.file);
	const v = byId.get(k) ?? {pass: 0, fail: 0};
	r.ok ? v.pass++ : v.fail++;
	byId.set(k, v);
}
const failed = results.filter((x) => !x.r.ok).length;
console.log(`\n--- by spec id ---`);
for (const [k, v] of [...byId].sort()) console.log(`${v.fail ? 'FAIL' : 'PASS'} ${k} ${v.pass}/${v.pass + v.fail}`);
console.log(
	`\n${results.length - failed} passed, ${failed} failed, ${results.length} runs in ${Date.now() - t0}ms (bin: ${bin})`,
);
process.exit(failed ? 1 : 0);
