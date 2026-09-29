// Scenario files: load + strict validation. Format reference: e2e/README.md.
// Every object is checked against a schema: unknown keys, step types, `$` operators or wrong types are load errors.
import {readFileSync} from 'node:fs';
import {parse} from 'yaml';
import {parseKeys} from './keys.js';

export type ShimRule = {
	match?: string[];
	stdout?: string;
	stderr?: string;
	exit?: number;
	readStdin?: boolean;
	block?: boolean;
	passthrough?: boolean;
};
export type Step = Record<string, unknown>;
export type Scenario = {
	file: string;
	id: string;
	also: string[]; // extra spec feature ids this scenario covers (coverage only)
	title: string;
	args: string[];
	env: Record<string, string | null>;
	cols: number;
	rows: number;
	fixture: 'standard' | 'empty' | 'nogit';
	files: Record<string, unknown>;
	shims: Record<string, ShimRule[]>;
	tui: boolean;
	cwd?: string;
	startWithoutBarrier: boolean;
	captureStderr: boolean;
	steps: Step[];
};

// ---- schema ----
type S =
	| 'str'
	| 'int'
	| 'bool'
	| 'num'
	| 'any'
	| 're' // regex string
	| 'var' // variable name
	| 'json' // JSON subset expectation: any value, `$` operators checked
	| {enum: readonly (string | number)[]}
	| {list: S; min?: number}
	| {map: S} // string keys
	| {obj: Record<string, S>; req?: string[]; nonEmpty?: boolean; check?: (o: Record<string, unknown>) => string | void}
	| {or: S[]; name: string};

const strs: S = {or: ['str', {list: 'str'}], name: 'string or list of strings'};
const res: S = {or: ['re', {list: 're'}], name: 'regex or list of regexes'};
const TM_KEYS = {equals: 'str', contains: strs, notContains: strs, matches: res} as const;
const tmObj = (extra: Record<string, S> = {}, o: {req?: string[]} = {}): S => ({
	obj: {...TM_KEYS, ...extra},
	nonEmpty: true,
	...o,
});
const TM: S = {or: ['str', tmObj()], name: 'text matcher (string or {equals, contains, notContains, matches})'};
const oneOrList = (s: S, name: string): S => ({or: [s, {list: s, min: 1}], name});
const color: S = {or: ['str', 'int'], name: "color ('#rrggbb', palette index or 'default')"};
const octal: S = {or: ['str', 'int'], name: "octal mode string like '644'"};
const argv: S = {list: 'str'};
const ports: S = {or: ['str', 'int'], name: 'port'};

const HTTP_REQ: Record<string, S> = {
	tool: 'str',
	args: 'any',
	method: 'str',
	params: 'any',
	body: 'any',
	raw: 'str',
	bodyRepeat: 'int',
	headers: {map: 'str'},
	auth: {or: ['bool', 'str'], name: 'false or header value'},
	path: 'str',
	httpMethod: 'str',
	port: ports,
};
const httpCheck = (o: Record<string, unknown>) => {
	const n = ['tool', 'method', 'body', 'raw'].filter((k) => k in o).length;
	if (n !== 1) return 'needs exactly one of tool, method, body, raw';
	if ('bodyRepeat' in o && !('raw' in o)) return 'bodyRepeat needs raw';
	if ('args' in o && !('tool' in o)) return 'args needs tool';
	if ('params' in o && !('method' in o)) return 'params needs method';
	if (o.auth === true) return 'auth: true is the default; use false or a header value';
};
const HTTP_EXPECT: S = {
	obj: {status: 'int', body: 'json', headers: 'json', result: 'json', text: TM},
	nonEmpty: true,
};
const CAPTURE: S = {map: 'str'};
const SHIM_RULE: Record<string, S> = {
	match: argv,
	stdout: 'str',
	stderr: 'str',
	exit: 'int',
	readStdin: 'bool',
	block: 'bool',
	passthrough: 'bool',
};
const shimRuleCheck = (o: Record<string, unknown>) => {
	if (o.passthrough && ('stdout' in o || 'stderr' in o || 'exit' in o || 'readStdin' in o))
		return 'passthrough runs the real program: no stdout, stderr, exit, readStdin';
};

type StepDef = {s: S; extra?: Record<string, S>; tui?: boolean; nonTui?: boolean};
const BARRIER_KIND: S = {enum: ['idle', 'frame', 'none']};
export const STEP_DEFS: Record<string, StepDef> = {
	keys: {s: 'str', extra: {wait: BARRIER_KIND}, tui: true},
	paste: {s: 'str', extra: {wait: BARRIER_KIND}, tui: true},
	barrier: {s: {enum: ['idle', 'frame']}, tui: true},
	expectScreen: {
		s: tmObj({
			line: oneOrList(tmObj({row: 'int'}, {req: ['row']}), 'line matcher or list'),
			region: oneOrList(tmObj({rows: {list: 'int', min: 2}, cols: {list: 'int', min: 2}}), 'region matcher or list'),
			cursor: {obj: {row: 'int', col: 'int', visible: 'bool'}, nonEmpty: true},
		}),
		tui: true,
	},
	expectCell: {
		s: {
			obj: {
				at: {
					obj: {text: 'str', nth: 'int', offset: 'int', row: 'int', col: 'int'},
					check: (o) => {
						if ('text' in o) {
							if ('col' in o) return 'use text or col, not both';
						} else if (!('row' in o && 'col' in o)) return 'needs {text} or {row, col}';
						else if ('nth' in o || 'offset' in o) return 'nth/offset need text';
					},
				},
				fg: color,
				bg: color,
				bold: 'bool',
				dim: 'bool',
				italic: 'bool',
				underline: 'bool',
				inverse: 'bool',
				strikethrough: 'bool',
				chars: 'str',
			},
			req: ['at'],
			check: (o) => (Object.keys(o).length < 2 ? 'nothing to check besides at' : undefined),
		},
		tui: true,
	},
	expectGolden: {s: 'str', tui: true},
	expectOsc52: {s: {or: ['str', tmObj({count: 'int'})], name: 'text matcher with optional count'}, tui: true},
	captureScreen: {s: {obj: {matches: 're', row: 'int', var: 'var'}, req: ['matches']}, tui: true},
	expectFile: {
		s: {
			obj: {
				path: 'str',
				glob: 'str',
				exists: 'bool',
				count: 'int',
				mode: octal,
				name: TM,
				capturePath: 'var',
				json: 'json',
				...TM_KEYS,
			},
			check: (o) => {
				if ('path' in o === 'glob' in o) return 'needs exactly one of path, glob';
				if ('count' in o && !('glob' in o)) return 'count needs glob';
				if ('count' in o && 'exists' in o) return 'use count or exists, not both';
				if ('capturePath' in o && 'count' in o && o.count !== 1) return 'capturePath needs exactly one match';
				if (o.exists === false && Object.keys(o).some((k) => !['path', 'glob', 'exists'].includes(k)))
					return 'exists: false takes no other checks';
			},
		},
	},
	writeFile: {
		s: {
			obj: {
				path: 'str',
				content: 'any',
				mode: octal,
				append: 'bool',
				lines: {obj: {from: 'int', to: 'int', text: 'str'}, req: ['from', 'to']},
				size: 'int',
				fill: 'str',
				lineLength: 'int',
				bytes: {
					obj: {hex: 'str', base64: 'str'},
					check: (o) => (Object.keys(o).length !== 1 ? 'needs hex or base64' : undefined),
				},
				nulAt: {or: ['int', {list: 'int'}], name: 'offset or list of offsets'},
			},
			req: ['path'],
			check: (o) => {
				const gens = ['content', 'lines', 'size', 'bytes'].filter((k) => k in o);
				if (gens.length > 1) return `use one of content, lines, size, bytes (got ${gens.join(', ')})`;
				if (('fill' in o || 'lineLength' in o) && !('size' in o)) return 'fill/lineLength need size';
				if (o.lineLength !== undefined && Number(o.lineLength) < 1) return 'lineLength must be >= 1';
				if (o.fill === '') return 'fill must not be empty';
			},
		},
	},
	removeFile: {s: 'str'},
	copyFile: {s: {obj: {from: 'str', to: 'str', mode: octal}, req: ['from', 'to']}},
	mkdir: {s: {or: ['str', {obj: {path: 'str', mode: octal}, req: ['path']}], name: 'path or {path, mode}'}},
	chmod: {s: {obj: {path: 'str', mode: octal}, req: ['path', 'mode']}},
	symlink: {s: {obj: {target: 'str', path: 'str'}, req: ['target', 'path']}},
	git: {
		s: {
			or: [
				'str',
				{list: 'str', min: 1},
				{obj: {args: {or: ['str', {list: 'str', min: 1}], name: 'args'}, code: 'int'}, req: ['args']},
			],
			name: 'argv string, argv list or {args, code}',
		},
	},
	http: {s: {obj: HTTP_REQ, check: httpCheck}, extra: {expect: HTTP_EXPECT, capture: CAPTURE}, tui: true},
	httpStart: {s: {obj: {id: 'str', ...HTTP_REQ}, req: ['id'], check: httpCheck}, tui: true},
	httpAwait: {
		s: {or: ['str', {obj: {id: 'str'}, req: ['id']}], name: 'id'},
		extra: {expect: HTTP_EXPECT, capture: CAPTURE},
		tui: true,
	},
	httpAbort: {s: 'str', tui: true},
	expectRefused: {s: {or: ['str', 'int', {obj: {port: ports}}], name: 'port or {port}'}, tui: true},
	holdPort: {
		s: {
			obj: {port: ports, var: 'var'},
			check: (o) => ('port' in o === 'var' in o ? 'needs exactly one of port, var' : undefined),
		},
		tui: true,
	},
	shim: {
		s: {
			obj: {name: 'str', rules: {list: {obj: SHIM_RULE, check: shimRuleCheck}}, remove: 'bool', ...SHIM_RULE},
			req: ['name'],
			check: (o) => {
				const rule = Object.keys(o).filter((k) => k in SHIM_RULE);
				if ('rules' in o && rule.length) return `use rules or rule fields (${rule.join(', ')}), not both`;
				if (o.remove && Object.keys(o).length > 2) return 'remove takes only name';
				return shimRuleCheck(o);
			},
		},
	},
	release: {s: 'str', tui: true},
	expectShimCall: {
		s: {
			obj: {
				name: 'str',
				args: argv,
				argsPrefix: argv,
				argsContain: argv,
				count: 'int',
				stdin: TM,
				cwd: 'str',
				nth: 'int',
				sequence: {list: argv, min: 1},
			},
			req: ['name'],
			check: (o) => ('sequence' in o && Object.keys(o).length > 2 ? 'sequence takes only name' : undefined),
		},
	},
	expectExit: {s: {or: ['int', {obj: {code: 'int'}}], name: 'exit code or {code}'}},
	expectStdout: {s: TM, nonTui: true},
	expectStderr: {s: TM},
};
export const STEPS = Object.keys(STEP_DEFS);
const TOP: Record<string, S> = {
	id: 'str',
	also: {list: 'str', min: 1},
	title: 'str',
	args: {list: 'str'},
	env: {map: {or: ['str', {enum: [null as unknown as string]}], name: 'string or null'}},
	size: 'str',
	fixture: {enum: ['standard', 'empty', 'nogit']},
	files: {map: 'any'},
	shims: {
		or: [
			{list: 'str'},
			{
				map: {
					or: [
						{enum: [null as unknown as string]},
						{obj: SHIM_RULE, check: shimRuleCheck},
						{list: {obj: SHIM_RULE, check: shimRuleCheck}},
					],
					name: 'rule or list of rules',
				},
			},
		],
		name: 'list of names or map name -> rules',
	},
	tui: 'bool',
	cwd: 'str',
	startWithoutBarrier: 'bool',
	captureStderr: 'bool',
	steps: {list: 'any'},
};
const OPS: Record<string, S> = {$contains: 'any', $matches: 're', $exists: 'bool', $len: 'int'};

const typeOf = (v: unknown) => (v === null ? 'null' : Array.isArray(v) ? 'list' : typeof v);
const noVars = (r: string) => r.replace(/\$\{\w+\}/g, 'V');

/** Returns an error message (with a path) or undefined. */
function check(v: unknown, s: S, at: string): string | undefined {
	const bad = (m: string) => `${at || 'value'}: ${m}`;
	if (typeof s === 'string') {
		switch (s) {
			case 'any':
				return;
			case 'str':
				return typeof v === 'string' ? undefined : bad(`expected string, got ${typeOf(v)}`);
			case 'var':
				return typeof v === 'string' && /^\w+$/.test(v) ? undefined : bad('expected variable name (\\w+)');
			case 'int':
				return Number.isInteger(v) ? undefined : bad(`expected integer, got ${typeOf(v)} ${JSON.stringify(v)}`);
			case 'num':
				return typeof v === 'number' ? undefined : bad(`expected number, got ${typeOf(v)}`);
			case 'bool':
				return typeof v === 'boolean' ? undefined : bad(`expected true/false, got ${typeOf(v)}`);
			case 're':
				if (typeof v !== 'string') return bad(`expected regex string, got ${typeOf(v)}`);
				try {
					new RegExp(noVars(v), 'm');
				} catch (e) {
					return bad(`bad regex: ${(e as Error).message}`);
				}
				return;
			case 'json':
				return checkJson(v, at);
		}
	}
	if ('enum' in s)
		return s.enum.includes(v as string)
			? undefined
			: bad(`expected one of ${s.enum.join(', ')}, got ${JSON.stringify(v)}`);
	if ('list' in s) {
		if (!Array.isArray(v)) return bad(`expected list, got ${typeOf(v)}`);
		if (s.min !== undefined && v.length < s.min) return bad(`needs at least ${s.min} items`);
		for (let i = 0; i < v.length; i++) {
			const e = check(v[i], s.list, `${at}[${i}]`);
			if (e) return e;
		}
		return;
	}
	if ('map' in s) {
		if (!v || typeof v !== 'object' || Array.isArray(v)) return bad(`expected mapping, got ${typeOf(v)}`);
		for (const [k, x] of Object.entries(v)) {
			const e = check(x, s.map, `${at}.${k}`);
			if (e) return e;
		}
		return;
	}
	if ('obj' in s) {
		if (!v || typeof v !== 'object' || Array.isArray(v)) return bad(`expected mapping, got ${typeOf(v)}`);
		const o = v as Record<string, unknown>;
		for (const k of Object.keys(o))
			if (!(k in s.obj)) return bad(`unknown key ${JSON.stringify(k)} (allowed: ${Object.keys(s.obj).join(', ')})`);
		for (const k of s.req ?? []) if (!(k in o)) return bad(`missing required key ${k}`);
		if (s.nonEmpty && !Object.keys(o).some((k) => !(s.req ?? []).includes(k)))
			return bad('empty matcher checks nothing');
		for (const [k, x] of Object.entries(o)) {
			const e = check(x, s.obj[k]!, `${at}.${k}`);
			if (e) return e;
		}
		const c = s.check?.(o);
		return c ? bad(c) : undefined;
	}
	// or: first alternative of the same shape wins, so errors point inside it
	const errs = s.or.map((a) => check(v, a, at));
	if (errs.some((e) => e === undefined)) return;
	const shape = (a: S) =>
		typeof a === 'string'
			? (
					{str: 'string', re: 'string', var: 'string', int: 'number', num: 'number', bool: 'boolean'} as Record<
						string,
						string
					>
				)[a]
			: 'list' in a
				? 'list'
				: 'obj' in a || 'map' in a
					? 'object'
					: undefined;
	const t = typeOf(v);
	const same = s.or.findIndex((a) => shape(a) === t);
	return same >= 0 && errs[same] && !errs[same]!.endsWith(`got ${t}`)
		? errs[same]
		: bad(`expected ${s.name}, got ${t}`);
}

function checkJson(v: unknown, at: string): string | undefined {
	if (Array.isArray(v)) {
		for (let i = 0; i < v.length; i++) {
			const e = checkJson(v[i], `${at}[${i}]`);
			if (e) return e;
		}
		return;
	}
	if (!v || typeof v !== 'object') return;
	const keys = Object.keys(v);
	const ops = keys.filter((k) => k.startsWith('$'));
	if (ops.length && ops.length !== keys.length) return `${at}: mixes $ operators with keys (${keys.join(', ')})`;
	if (ops.length) {
		for (const k of ops) {
			if (!(k in OPS)) return `${at}: unknown operator ${k} (allowed: ${Object.keys(OPS).join(', ')})`;
			const e =
				k === '$contains'
					? checkJson((v as Record<string, unknown>)[k], `${at}.${k}`)
					: check((v as Record<string, unknown>)[k], OPS[k]!, `${at}.${k}`);
			if (e) return e;
		}
		return;
	}
	for (const k of keys) {
		const e = checkJson((v as Record<string, unknown>)[k], `${at}.${k}`);
		if (e) return e;
	}
	return;
}

// ---- git argv strings ----
/** Split a git argument string into argv like a shell would for plain words and quotes; shell syntax is an error. */
export function splitArgs(s: string): string[] {
	const out: string[] = [];
	let cur: string | undefined;
	for (let i = 0; i < s.length; i++) {
		const c = s[i]!;
		if (/\s/.test(c)) {
			if (cur !== undefined) out.push(cur);
			cur = undefined;
		} else if (c === "'") {
			const j = s.indexOf("'", i + 1);
			if (j < 0) throw new Error('unterminated single quote');
			cur = (cur ?? '') + s.slice(i + 1, j);
			i = j;
		} else if (c === '"') {
			let t = '';
			let j = i + 1;
			for (; j < s.length && s[j] !== '"'; j++) {
				if (s[j] === '\\' && (s[j + 1] === '"' || s[j + 1] === '\\')) j++;
				else if (s[j] === '`' || (s[j] === '$' && s[j + 1] === '(')) throw new Error(`shell substitution in "${s}"`);
				t += s[j];
			}
			if (j >= s.length) throw new Error('unterminated double quote');
			cur = (cur ?? '') + t;
			i = j;
		} else if (c === '\\') {
			cur = (cur ?? '') + (s[i + 1] ?? '');
			i++;
		} else if (';&|<>`()'.includes(c) || (c === '$' && s[i + 1] === '(')) {
			throw new Error(`shell operator ${JSON.stringify(c)} not allowed (git steps run git directly, no shell)`);
		} else cur = (cur ?? '') + c;
	}
	if (cur !== undefined) out.push(cur);
	return out;
}

export const stepKind = (s: Step) => STEPS.find((k) => k in s)!;
const BUILTIN_VARS = ['TMP', 'REPO', 'HOME', 'CONFIG', 'STATE', 'SHIMS', 'PORT', 'ROOT'];

export function loadScenario(file: string): Scenario {
	const raw = parse(readFileSync(file, 'utf8')) as Record<string, unknown>;
	const bad = (m: string): never => {
		throw new Error(`${file}: ${m}`);
	};
	if (!raw || typeof raw !== 'object' || Array.isArray(raw)) bad('not a mapping');
	const te = check(raw, {obj: TOP, req: ['id', 'steps']}, 'top level');
	if (te) bad(te);
	if (!raw.id) bad('id must not be empty');
	const also = ((raw.also ?? []) as string[]).map((a) => a.trim());
	also.forEach((a, i) => {
		if (!a) bad(`also[${i}] must not be empty`);
		if (a === raw.id || also.indexOf(a) !== i) bad(`also[${i}]: duplicate id ${a}`);
	});
	const size = String(raw.size ?? '120x40');
	const m = /^(\d+)x(\d+)$/.exec(size) ?? bad(`size must be COLSxROWS, got ${size}`);
	const fixture = (raw.fixture ?? 'standard') as Scenario['fixture'];
	const shims: Record<string, ShimRule[]> = {};
	if (Array.isArray(raw.shims)) for (const n of raw.shims) shims[String(n)] = [];
	else if (raw.shims) for (const [n, r] of Object.entries(raw.shims)) shims[n] = r ? (Array.isArray(r) ? r : [r]) : [];
	const tui = raw.tui !== false;
	const captureStderr = raw.captureStderr === true;
	if (!tui && captureStderr) bad('captureStderr is for TUI scenarios (tui: false always captures stderr)');
	if (!tui && raw.startWithoutBarrier) bad('startWithoutBarrier needs a TUI scenario');
	const steps = raw.steps as Step[];
	const cols = Number(m[1]);
	const rows = Number(m[2]);
	// rows/cols outside the screen would make notContains checks vacuous
	const rowOk = (r: number) => r >= -rows && r < rows;
	const knownShims = new Set(Object.keys(shims));
	// variables: builtins, then whatever steps capture (in order)
	const vars = new Set(BUILTIN_VARS);
	const useVars = (v: unknown, at: string) => {
		const walk = (x: unknown) => {
			if (typeof x === 'string')
				for (const mm of x.matchAll(/\$\{(\w+)\}/g)) if (!vars.has(mm[1]!)) bad(`${at}: unknown variable \${${mm[1]}}`);
			if (x && typeof x === 'object') for (const [k, y] of Object.entries(x)) (walk(k), walk(y));
		};
		walk(v);
	};
	useVars({args: raw.args, env: raw.env, files: raw.files, shims: raw.shims, cwd: raw.cwd}, 'top level');
	steps.forEach((s, i) => {
		const kinds = s && typeof s === 'object' && !Array.isArray(s) ? STEPS.filter((k) => k in s) : [];
		const at = `step ${i + 1}${kinds.length === 1 ? ` (${kinds[0]})` : ''}`;
		if (!s || typeof s !== 'object' || Array.isArray(s)) bad(`${at}: not a mapping`);
		if (kinds.length !== 1)
			bad(
				`${at}: needs exactly one step type (got ${Object.keys(s).join(', ') || 'nothing'}); step types: ${STEPS.join(', ')}`,
			);
		const k = kinds[0]!;
		const def = STEP_DEFS[k]!;
		const extra: Record<string, S> = {note: 'str', ...def.extra};
		for (const key of Object.keys(s))
			if (key !== k && !(key in extra))
				bad(`${at}: unknown key ${JSON.stringify(key)} (allowed beside ${k}: ${Object.keys(extra).join(', ')})`);
		const e = check(s[k], def.s, k);
		if (e) bad(`${at}: ${e}`);
		for (const key of Object.keys(extra)) {
			if (!(key in s)) continue;
			const e2 = check(s[key], extra[key]!, key);
			if (e2) bad(`${at}: ${e2}`);
		}
		if (!tui && def.tui) bad(`${at}: ${k} needs a TUI scenario`);
		if (tui && def.nonTui) bad(`${at}: ${k} needs tui: false`);
		if (tui && k === 'expectStderr' && !captureStderr)
			bad(`${at}: expectStderr in a TUI scenario needs captureStderr: true`);
		if (k === 'keys') {
			try {
				parseKeys(s.keys as string);
			} catch (err) {
				bad(`${at}: ${(err as Error).message}`);
			}
		}
		if (k === 'git') {
			const g = s.git as unknown;
			const a =
				typeof g === 'string'
					? g
					: !Array.isArray(g) && typeof (g as {args: unknown}).args === 'string'
						? (g as {args: string}).args
						: undefined;
			if (a !== undefined)
				try {
					if (!splitArgs(a).length) bad(`${at}: empty git args`);
				} catch (err) {
					bad(`${at}: ${(err as Error).message}`);
				}
		}
		useVars(k === 'keys' || k === 'paste' ? {} : {v: s[k], expect: s.expect}, at);
		if (k === 'expectScreen' || k === 'expectCell') {
			const v = s[k] as {line?: unknown; region?: unknown; at?: {row?: number; col?: number}};
			for (const l of ([] as {row: number}[]).concat((v.line ?? []) as {row: number}[]))
				if (!rowOk(l.row)) bad(`${at}: line row ${l.row} outside the ${cols}x${rows} screen`);
			for (const g of ([] as {rows?: number[]; cols?: number[]}[]).concat((v.region ?? []) as {rows?: number[]}[])) {
				if ((g.rows && g.rows.length !== 2) || (g.cols && g.cols.length !== 2))
					bad(`${at}: region rows/cols need [from, to]`);
				if (g.rows && !g.rows.every(rowOk)) bad(`${at}: region rows ${JSON.stringify(g.rows)} outside the screen`);
				if (g.cols && !g.cols.every((c) => c >= 0 && c < cols))
					bad(`${at}: region cols ${JSON.stringify(g.cols)} outside the screen`);
			}
			if (v.at?.row !== undefined && !rowOk(v.at.row)) bad(`${at}: at.row ${v.at.row} outside the screen`);
			if (v.at?.col !== undefined && !(v.at.col >= 0 && v.at.col < cols))
				bad(`${at}: at.col ${v.at.col} outside the screen`);
		}
		if (k === 'captureScreen') {
			const r = (s[k] as {row?: number}).row;
			if (r !== undefined && !rowOk(r)) bad(`${at}: row ${r} outside the screen`);
		}
		if (k === 'shim') knownShims.add((s[k] as {name: string}).name);
		if (
			(k === 'expectShimCall' || k === 'release') &&
			!knownShims.has(k === 'release' ? String(s[k]) : (s[k] as {name: string}).name)
		)
			bad(
				`${at}: no shim named ${JSON.stringify(k === 'release' ? s[k] : (s[k] as {name: string}).name)} (declare it in shims or a shim step)`,
			);
		// variables defined by this step
		for (const name of Object.keys((s.capture ?? {}) as object)) vars.add(name);
		if (k === 'captureScreen') {
			const c = s[k] as {matches: string; var?: string};
			const named = [...noVars(c.matches).matchAll(/\(\?<(\w+)>/g)].map((x) => x[1]!);
			if (!c.var && !named.length) bad(`${at}: captureScreen needs var or named groups (?<NAME>...)`);
			for (const n of named) vars.add(n);
			if (c.var) vars.add(c.var);
		}
		if (k === 'expectFile' && (s[k] as {capturePath?: string}).capturePath)
			vars.add((s[k] as {capturePath: string}).capturePath);
		if (k === 'holdPort' && (s[k] as {var?: string}).var) vars.add((s[k] as {var: string}).var);
	});
	return {
		file,
		id: raw.id as string,
		also,
		title: String(raw.title ?? ''),
		args: ((raw.args ?? []) as unknown[]).map(String),
		env: (raw.env ?? {}) as Record<string, string | null>,
		cols,
		rows,
		fixture,
		files: (raw.files ?? {}) as Record<string, unknown>,
		shims,
		tui,
		cwd: raw.cwd as string | undefined,
		startWithoutBarrier: raw.startWithoutBarrier === true,
		captureStderr,
		steps,
	};
}
