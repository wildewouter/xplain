import type {Answer, AnswerStatus} from '../ask/types.js';

// hard wrap to width, breaking at spaces when possible; keeps blank lines
export function wrapText(text: string, width: number): string[] {
	const w = Math.max(1, width);
	const out: string[] = [];
	for (const raw of text.replace(/\r/g, '').replace(/\t/g, '  ').split('\n')) {
		let l = raw;
		if (!l) {
			out.push('');
			continue;
		}
		while (l.length > w) {
			let k = l.lastIndexOf(' ', w);
			if (k <= 0) k = w;
			out.push(l.slice(0, k).trimEnd());
			l = l.slice(k).trimStart();
		}
		out.push(l);
	}
	while (out.length > 1 && out[out.length - 1] === '') out.pop();
	return out;
}

export const ANS_MAX = 12;
export const ANS_MAX_FOCUS = 30;

const LABEL: Record<AnswerStatus, string> = {
	pending: 'waiting…',
	streaming: 'streaming…',
	done: 'done',
	error: 'error',
	cancelled: 'cancelled',
};

export type AnswerView = {head: string; lines: string[]; more: number; status: AnswerStatus};
// answer as shown in a sent box: wrapped + capped
export function answerView(a: Answer, width: number, focused: boolean, room = Infinity): AnswerView {
	const body = a.status === 'error' ? `${a.error ?? 'failed'}` : a.text;
	const all = body ? wrapText(body, width) : [];
	const cap = Math.max(1, Math.min(focused ? ANS_MAX_FOCUS : ANS_MAX, room));
	return {
		head: `answer · ${a.agent ?? 'agent'} · ${LABEL[a.status]}`,
		lines: all.slice(0, cap),
		more: Math.max(0, all.length - cap),
		status: a.status,
	};
}

// ---- thread body: all turns flattened to display lines ----
export type BodyLine = {
	t: string;
	k: 'msg' | 'fu' | 'div' | 'ans' | 'btn' | 'code';
	err?: boolean;
	live?: 'pending' | 'streaming';
	lang?: string; // btn/code: fence language
	blk?: number; // btn/code: code block index within the thread
	code?: string; // btn: raw block text (copied)
};
export type TurnIn = {message: string; answer?: Answer; prior?: Answer[]};
export const BODY_CAP = 14; // unfocused: lines shown before "… +N more"

export const COPY_BTN = '[ copy ]';
export const CODE_GUTTER = 2; // "│ " before each code line
const OPEN = /^\s*(`{3,}|~{3,})\s*([^\s`]*)/;

// prose wrapped as usual; each fenced block becomes a copy button line + hard-cut code lines (fences not shown)
export function richLines(text: string, width: number, k: 'msg' | 'ans', blk = {n: 0}): BodyLine[] {
	const out: BodyLine[] = [];
	const prose: string[] = [];
	const flush = () => {
		if (prose.length) for (const t of wrapText(prose.join('\n'), width)) out.push({t, k});
		prose.length = 0;
	};
	const src = text.replace(/\r/g, '').split('\n');
	for (let i = 0; i < src.length; i++) {
		const m = OPEN.exec(src[i]!);
		if (!m) {
			prose.push(src[i]!);
			continue;
		}
		flush();
		const fence = m[1]!;
		const close = new RegExp(`^\\s*${fence[0]}{${fence.length},}\\s*$`);
		const code: string[] = [];
		while (++i < src.length && !close.test(src[i]!)) code.push(src[i]!);
		const lang = m[2] || undefined;
		const n = blk.n++;
		out.push({t: COPY_BTN + (lang ? ' ' + lang : ''), k: 'btn', blk: n, code: code.join('\n'), ...(lang && {lang})});
		const cw = Math.max(1, width - CODE_GUTTER);
		for (const raw of code) {
			const l = raw.replace(/\t/g, '  ');
			for (let j = 0; j === 0 || j < l.length; j += cw)
				out.push({t: l.slice(j, j + cw), k: 'code', blk: n, ...(lang && {lang})});
		}
	}
	flush();
	return out;
}

const isLive = (a: Answer) => a.status === 'pending' || a.status === 'streaming';

// message line of turn 1 (multi-line agent notes get wrapped + code blocks), then per turn: answer divider + wrapped answer (each prior answer too); follow-ups get `follow-up:` lines
export function threadBody(turns: TurnIn[], msg0: string, width: number): BodyLine[] {
	const out: BodyLine[] = [];
	const blk = {n: 0};
	turns.forEach((tu, i) => {
		if (i === 0) {
			if (msg0.includes('\n')) out.push(...richLines(msg0, width, 'msg', blk));
			else out.push({t: msg0, k: 'msg'});
		} else for (const l of wrapText('follow-up: ' + tu.message, width)) out.push({t: l, k: 'fu'});
		for (const a of [...(tu.prior ?? []), ...(tu.answer ? [tu.answer] : [])]) {
			const shown =
				a.text || !isLive(a) ? a : {...a, text: a.status === 'pending' ? 'waiting for agent…' : 'agent working…'};
			const v = answerView(shown, width, true, Infinity);
			const body = a.status === 'error' ? (a.error ?? 'failed') : shown.text;
			const err = a.status === 'error';
			out.push({t: v.head, k: 'div', err});
			const live = !a.text && isLive(a) ? (a.status as 'pending' | 'streaming') : undefined;
			if (!body) continue;
			if (err || live) for (const l of wrapText(body, width)) out.push({t: l, k: 'ans', err, ...(live && {live})});
			else out.push(...richLines(body, width, 'ans', blk));
		}
	});
	return out;
}

export type BodyWin = {
	shown: BodyLine[];
	more: number;
	total: number;
	from: number;
	to: number;
	maxOff: number;
	off: number;
};
// unfocused: head + "… +N more"; focused: window of v lines at off (clamped)
export function windowBody(lines: BodyLine[], focused: boolean, v: number, off: number): BodyWin {
	const total = lines.length;
	if (!focused) {
		const n = Math.min(total, BODY_CAP);
		return {shown: lines.slice(0, n), more: total - n, total, from: 1, to: n, maxOff: 0, off: 0};
	}
	const vv = Math.max(1, v);
	const maxOff = Math.max(0, total - vv);
	const o = Math.min(maxOff, Math.max(0, off));
	const shown = lines.slice(o, o + vv);
	return {shown, more: 0, total, from: o + 1, to: o + shown.length, maxOff, off: o};
}
