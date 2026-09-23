// Headless comment/thread store. No UI imports.
import type {Answer, Question, Turn} from './types.js';

export * from './types.js';

export type AskState = {
	questions: Question[];
	answers: Record<string, Answer>;
};

export const isLiveStatus = (a?: Answer) => a?.status === 'pending' || a?.status === 'streaming';

const turnsOf = (q: Question): Turn[] => q.turns ?? [{message: q.message, ...(q.answer ? {answer: q.answer} : {})}];

const followable = (q: Question) => {
	const t = turnsOf(q);
	const last = t[t.length - 1]!;
	if (!last.answer) return q.origin === 'agent' && t.length === 1;
	return !isLiveStatus(last.answer);
};

export type AskController = ReturnType<typeof createAskController>;

export function createAskController() {
	let state: AskState = {questions: [], answers: {}};
	const listeners = new Set<() => void>();
	let nextId = 1;
	let disposed = false;

	const set = (p: Partial<AskState>) => {
		state = {...state, ...p};
		listeners.forEach((l) => l());
	};
	const mapQ = (id: string, q: Question) => set({questions: state.questions.map((x) => (x.id === id ? q : x))});

	return {
		getState: () => state,
		subscribe(l: () => void) {
			listeners.add(l);
			return () => void listeners.delete(l);
		},
		answer: (id?: string): Answer | undefined => (id ? state.answers[id] : undefined),
		isLive: (id?: string) => isLiveStatus(id ? state.answers[id] : undefined),
		add(q0: Question): Question {
			const q = {...q0, id: q0.id ?? `q${nextId++}`};
			q.turns ??= turnsOf(q0);
			set({questions: [...state.questions, q]});
			return q;
		},
		edit(id: string, message: string): Question | undefined {
			const q = state.questions.find((x) => x.id === id);
			if (!q || turnsOf(q).length > 1) return undefined;
			const q1 = {...q, message, turns: [{...turnsOf(q)[0]!, message}]};
			mapQ(id, q1);
			return q1;
		},
		turns: (id?: string): Turn[] => {
			const q = state.questions.find((x) => x.id === id);
			return q ? turnsOf(q) : [];
		},
		latestTurn: (id?: string): {turn: number; message: string; answer?: Answer} | undefined => {
			const q = state.questions.find((x) => x.id === id);
			if (!q) return undefined;
			const t = turnsOf(q);
			return {turn: t.length, ...t[t.length - 1]!};
		},
		// can a follow-up turn be appended? an agent note's own text counts as the answered first turn
		canFollowUp: (id?: string): boolean => {
			const q = state.questions.find((x) => x.id === id);
			return !!q && followable(q);
		},
		// append a follow-up turn; 1-based turn number, undefined when refused
		followUp(id: string, message: string): number | undefined {
			const q = state.questions.find((x) => x.id === id);
			if (!q || !followable(q)) return undefined;
			const t = turnsOf(q);
			const turns = [...t, {message}];
			const {answer: _a, ...rest} = q;
			const {[id]: _b, ...answers} = state.answers;
			set({answers, questions: state.questions.map((x) => (x.id === id ? {...rest, turns} : x))});
			return turns.length;
		},
		// reply for a stored comment (default: latest turn); undefined when unknown
		setAnswer(
			id: string,
			a: {status: Answer['status']; text: string; error?: string; agent?: string},
			turn?: number,
		): Question | undefined {
			return put(id, a, turn, false);
		},
		// like setAnswer, but a turn that already has a done answer keeps it and gains a new one
		addAnswer(
			id: string,
			a: {status: Answer['status']; text: string; error?: string; agent?: string},
			turn?: number,
		): Question | undefined {
			return put(id, a, turn, true);
		},
		remove(id: string): Question | undefined {
			const q = state.questions.find((x) => x.id === id);
			const {[id]: _, ...answers} = state.answers;
			set({answers, questions: state.questions.filter((x) => x.id !== id)});
			return q;
		},
		dispose() {
			if (disposed) return;
			disposed = true;
			listeners.clear();
		},
	};

	function put(
		id: string,
		a: {status: Answer['status']; text: string; error?: string; agent?: string},
		turn: number | undefined,
		append: boolean,
	): Question | undefined {
		const q = state.questions.find((x) => x.id === id);
		if (!q) return undefined;
		const t = turnsOf(q);
		const n = turn ?? t.length;
		if (n < 1 || n > t.length) return undefined;
		const ans: Answer = {tools: 0, ...a};
		const keep = (x: Turn) => (append && x.answer?.status === 'done' ? {prior: [...(x.prior ?? []), x.answer]} : {});
		const turns = t.map((x, i) => (i === n - 1 ? {...x, ...keep(x), answer: ans} : x));
		const latest = turns[turns.length - 1]!.answer;
		const q1: Question = {...q, turns};
		if (latest) q1.answer = latest;
		else delete q1.answer;
		const {[id]: _, ...answers} = state.answers;
		set({
			answers: latest ? {...answers, [id]: latest} : answers,
			questions: state.questions.map((x) => (x.id === id ? q1 : x)),
		});
		return q1;
	}
}
