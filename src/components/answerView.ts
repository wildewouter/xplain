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
export type BodyLine = {t: string; k: 'msg' | 'fu' | 'div' | 'ans'; err?: boolean};
export type TurnIn = {message: string; answer?: Answer};
export const BODY_CAP = 14; // unfocused: lines shown before "… +N more"

const isLive = (a: Answer) => a.status === 'pending' || a.status === 'streaming';

// message line of turn 1, then per turn: answer divider + wrapped answer; follow-ups get `follow-up:` lines
export function threadBody(turns: TurnIn[], msg0: string, width: number): BodyLine[] {
	const out: BodyLine[] = [];
	turns.forEach((tu, i) => {
		if (i === 0) out.push({t: msg0, k: 'msg'});
		else for (const l of wrapText('follow-up: ' + tu.message, width)) out.push({t: l, k: 'fu'});
		const a = tu.answer;
		if (!a) return;
		const shown =
			a.text || !isLive(a) ? a : {...a, text: a.status === 'pending' ? 'waiting for agent…' : 'agent working…'};
		const v = answerView(shown, width, true, Infinity);
		const body = a.status === 'error' ? (a.error ?? 'failed') : shown.text;
		const err = a.status === 'error';
		out.push({t: v.head, k: 'div', err});
		if (body) for (const l of wrapText(body, width)) out.push({t: l, k: 'ans', err});
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
