// Key notation: literal text (one key per character) plus <Name> tokens. `<lt>` is a literal `<`.
// Named: Esc Enter Tab S-Tab Up Down Left Right Home End PageUp PageDown Space BS Del, C-x (ctrl), A-x / M-x (alt).
export type Key = {label: string; bytes: (appCursor: boolean) => string};

const csi = (normal: string, app: string) => (appCursor: boolean) => (appCursor ? app : normal);
const fixed = (s: string) => () => s;

const NAMED: Record<string, (appCursor: boolean) => string> = {
	esc: fixed('\x1b'),
	escape: fixed('\x1b'),
	enter: fixed('\r'),
	cr: fixed('\r'),
	return: fixed('\r'),
	tab: fixed('\t'),
	's-tab': fixed('\x1b[Z'),
	space: fixed(' '),
	bs: fixed('\x7f'),
	backspace: fixed('\x7f'),
	del: fixed('\x1b[3~'),
	up: csi('\x1b[A', '\x1bOA'),
	down: csi('\x1b[B', '\x1bOB'),
	right: csi('\x1b[C', '\x1bOC'),
	left: csi('\x1b[D', '\x1bOD'),
	home: csi('\x1b[H', '\x1bOH'),
	end: csi('\x1b[F', '\x1bOF'),
	pageup: fixed('\x1b[5~'),
	pagedown: fixed('\x1b[6~'),
	lt: fixed('<'),
};

const ctrl = (c: string) => {
	const u = c.toUpperCase();
	if (c === ' ' || c === '@') return '\x00';
	if (u >= 'A' && u <= 'Z') return String.fromCharCode(u.charCodeAt(0) - 64);
	const m: Record<string, string> = {'[': '\x1b', '\\': '\x1c', ']': '\x1d', '^': '\x1e', _: '\x1f', '?': '\x7f'};
	return m[c];
};

function named(name: string): Key | undefined {
	const n = name.toLowerCase();
	if (NAMED[n]) return {label: `<${name}>`, bytes: NAMED[n]!};
	const m = /^([CAM])-(.+)$/i.exec(name);
	if (!m) return undefined;
	const inner = m[2]!.length === 1 ? fixed(m[2]!) : NAMED[m[2]!.toLowerCase()];
	if (!inner) return undefined;
	if (m[1]!.toUpperCase() === 'C') {
		const b = m[2]!.length === 1 ? ctrl(m[2]!) : undefined;
		return b === undefined ? undefined : {label: `<${name}>`, bytes: fixed(b)};
	}
	return {label: `<${name}>`, bytes: (a) => '\x1b' + inner(a)};
}

/** Split a key string into keys; throws on an unknown <Token>. */
export function parseKeys(s: string): Key[] {
	const out: Key[] = [];
	let i = 0;
	while (i < s.length) {
		const m = s[i] === '<' ? /^<([A-Za-z][\w-]*|[CAM]-.)>/.exec(s.slice(i)) : null;
		if (m) {
			const k = named(m[1]!);
			if (!k) throw new Error(`unknown key token ${m[0]} (use <lt> for a literal <)`);
			out.push(k);
			i += m[0].length;
			continue;
		}
		const ch = String.fromCodePoint(s.codePointAt(i)!);
		out.push({label: ch, bytes: fixed(ch)});
		i += ch.length;
	}
	return out;
}
