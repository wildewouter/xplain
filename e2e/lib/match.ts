// Text matchers and JSON subset matching. Each returns an error string, or undefined when it matches.
const show = (v: unknown) => JSON.stringify(v);
const list = <T>(v: T | T[] | undefined): T[] => (v === undefined ? [] : Array.isArray(v) ? v : [v]);

export type TextMatch =
	| string
	| {equals?: string; contains?: string | string[]; notContains?: string | string[]; matches?: string | string[]};

/** Plain string = contains. */
export function matchText(actual: string, m: TextMatch, what = 'text'): string | undefined {
	const o = typeof m === 'string' ? {contains: m} : m;
	if (o.equals !== undefined && actual !== o.equals) return `${what}: expected to equal ${show(o.equals)}`;
	for (const c of list(o.contains)) if (!actual.includes(c)) return `${what}: expected to contain ${show(c)}`;
	for (const c of list(o.notContains)) if (actual.includes(c)) return `${what}: expected not to contain ${show(c)}`;
	for (const r of list(o.matches)) if (!new RegExp(r, 'm').test(actual)) return `${what}: expected to match /${r}/m`;
	return undefined;
}

/**
 * Deep subset: objects need listed keys only, arrays match element-wise with equal length, primitives strictly.
 * Operators (object whose keys all start with $): $contains (string or array element subset), $matches, $exists, $len.
 */
export function subset(actual: unknown, exp: unknown, path = '$'): string | undefined {
	if (exp && typeof exp === 'object' && !Array.isArray(exp)) {
		const keys = Object.keys(exp);
		if (keys.length && keys.every((k) => k.startsWith('$'))) {
			const o = exp as Record<string, unknown>;
			if ('$exists' in o && (actual !== undefined) !== o.$exists)
				return `${path}: expected ${o.$exists ? 'present' : 'absent'}`;
			if ('$contains' in o) {
				if (typeof actual === 'string') {
					if (!actual.includes(String(o.$contains))) return `${path}: ${show(actual)} lacks ${show(o.$contains)}`;
				} else if (Array.isArray(actual)) {
					if (!actual.some((x) => subset(x, o.$contains) === undefined))
						return `${path}: no element matches ${show(o.$contains)}`;
				} else return `${path}: $contains needs string or array, got ${show(actual)}`;
			}
			if ('$matches' in o && (typeof actual !== 'string' || !new RegExp(String(o.$matches), 'm').test(actual)))
				return `${path}: ${show(actual)} does not match /${o.$matches}/`;
			if ('$len' in o && (actual as {length?: unknown})?.length !== o.$len)
				return `${path}: length ${(actual as {length?: unknown})?.length} != ${o.$len}`;
			return undefined;
		}
		if (!actual || typeof actual !== 'object' || Array.isArray(actual))
			return `${path}: expected object, got ${show(actual)}`;
		for (const k of keys) {
			const e = subset((actual as Record<string, unknown>)[k], (exp as Record<string, unknown>)[k], `${path}.${k}`);
			if (e) return e;
		}
		return undefined;
	}
	if (Array.isArray(exp)) {
		if (!Array.isArray(actual) || actual.length !== exp.length)
			return `${path}: expected array of ${exp.length}, got ${show(actual)}`;
		for (let i = 0; i < exp.length; i++) {
			const e = subset(actual[i], exp[i], `${path}[${i}]`);
			if (e) return e;
		}
		return undefined;
	}
	return actual === exp ? undefined : `${path}: expected ${show(exp)}, got ${show(actual)}`;
}

/** Dotted path lookup (a.b.0.c). */
export function pick(o: unknown, path: string): unknown {
	return path
		.split('.')
		.filter(Boolean)
		.reduce<unknown>((v, k) => (v && typeof v === 'object' ? (v as Record<string, unknown>)[k] : undefined), o);
}
