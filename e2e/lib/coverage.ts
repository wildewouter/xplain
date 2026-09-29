// Spec coverage: every in-scope feature of the SPEC.md coverage index needs a scenario, every scenario id
// (and `also` id) must be an in-scope feature.
import {readFileSync} from 'node:fs';

export type SpecIndex = {inScope: Set<string>; outScope: Map<string, string>}; // out: id -> Test column

export function parseSpecIndex(file: string): SpecIndex {
	const text = readFileSync(file, 'utf8');
	const start = text.search(/^## Coverage index\s*$/m);
	if (start < 0) throw new Error(`${file}: no "## Coverage index" section`);
	const inScope = new Set<string>();
	const outScope = new Map<string, string>();
	for (const line of text.slice(start).split('\n').slice(1)) {
		if (/^#{1,2} /.test(line)) break;
		const row = /^\|(.*)\|\s*$/.exec(line);
		const cells = row ? row[1]!.split('|').map((c) => c.trim()) : [];
		if (cells.length < 3 || !/^F-[A-Z]+-\d+$/.test(cells[0]!)) continue;
		const [id, , test] = cells as [string, string, string];
		if (inScope.has(id) || outScope.has(id)) throw new Error(`${file}: coverage index lists ${id} twice`);
		if (test === 'yes') inScope.add(id);
		else if (/^no \(.+\)$/.test(test)) outScope.set(id, test);
		else throw new Error(`${file}: ${id}: Test must be yes or no (<why>), got "${test}"`);
	}
	if (!inScope.size) throw new Error(`${file}: coverage index has no in-scope features`);
	return {inScope, outScope};
}

export type Covered = {file: string; id: string; also: string[]};

/** Report lines and whether coverage is complete. */
export function checkCoverage(spec: SpecIndex, scenarios: Covered[]): {ok: boolean; lines: string[]} {
	const count = new Map([...spec.inScope].map((id) => [id, 0]));
	const bad: string[] = [];
	for (const sc of scenarios)
		for (const [i, id] of [sc.id, ...sc.also].entries()) {
			const field = i ? 'also' : 'id';
			if (count.has(id)) count.set(id, count.get(id)! + 1);
			else if (spec.outScope.has(id))
				bad.push(`FAIL ${sc.file}: ${field} ${id} is out of scope (${spec.outScope.get(id)})`);
			else bad.push(`FAIL ${sc.file}: ${field} ${id} is not in the spec coverage index`);
		}
	const gaps = [...count].filter(([, n]) => !n).map(([id]) => id);
	const covered = spec.inScope.size - gaps.length;
	const lines = [
		...gaps.map((id) => `FAIL ${id}: no scenario`),
		...bad,
		`${covered}/${spec.inScope.size} in-scope features covered, ${gaps.length} gaps, ${bad.length} bad ids, ` +
			`${scenarios.length} scenarios (${spec.outScope.size} out of scope)`,
	];
	return {ok: !gaps.length && !bad.length, lines};
}
