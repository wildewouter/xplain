import type {Hub, Question} from './hub.js';

export type ToolDef = {name: string; description: string; inputSchema: Record<string, unknown>};
export type ToolResult = {content: {type: 'text'; text: string}[]; isError?: boolean};
export type ToolContext = {hub: Hub; clientId: string; signal?: AbortSignal; onDeliver?: (q: Question) => void};

export const DEFAULT_WAIT_S = 45;
export const MAX_WAIT_S = 120;
const AGAIN = 'Call next_question again immediately.';

export const TOOLS: ToolDef[] = [
	{
		name: 'next_question',
		description:
			'Long-poll for the next question a human asked in the xplain code-review UI. ' +
			'Returns {status:"question", thread_id, turn, follow_up, question}: answer it with the answer tool. ' +
			'If follow_up is true it continues an earlier thread: use `previous` (earlier questions and your answers) for reference and answer with the SAME thread_id. ' +
			'If status is "no_question_yet", call next_question again immediately. ' +
			'Keep looping: after every answer, call next_question again immediately, until status is "closed".',
		inputSchema: {
			type: 'object',
			properties: {
				wait_seconds: {
					type: 'integer',
					minimum: 1,
					maximum: MAX_WAIT_S,
					default: DEFAULT_WAIT_S,
					description: 'Max seconds to wait for a question (1-120).',
				},
			},
			additionalProperties: false,
		},
	},
	{
		name: 'answer',
		description:
			'Send your answer for a question received from next_question, using its thread_id (for a follow-up, the same thread_id as before). Plain text or markdown. ' +
			'Then call next_question again immediately.',
		inputSchema: {
			type: 'object',
			properties: {
				thread_id: {type: 'string', description: 'thread_id from next_question.'},
				text: {type: 'string', description: 'The answer text.'},
			},
			required: ['thread_id', 'text'],
			additionalProperties: false,
		},
	},
	{
		name: 'get_questions',
		description:
			'List questions (and follow-ups, marked follow_up) still waiting to be delivered, without consuming them. Does not replace the next_question loop.',
		inputSchema: {type: 'object', properties: {}, additionalProperties: false},
	},
	{
		name: 'annotate',
		description: 'Attach a note to a line of a file in the xplain diff view. Then continue the next_question loop.',
		inputSchema: {
			type: 'object',
			properties: {
				file: {type: 'string', description: 'File path as shown in the diff.'},
				line: {type: 'number', description: '1-based line number.'},
				text: {type: 'string', description: 'Annotation text.'},
				side: {type: 'string', enum: ['old', 'new'], description: 'Diff side; default new.'},
			},
			required: ['file', 'line', 'text'],
			additionalProperties: false,
		},
	},
];

const text = (o: unknown, isError = false): ToolResult => ({
	content: [{type: 'text', text: typeof o === 'string' ? o : JSON.stringify(o)}],
	...(isError ? {isError: true} : {}),
});

export function formatQuestion(q: Question): string {
	const msg = q.followUp
		? `Follow-up to your earlier answer (thread ${q.threadId}, turn ${q.turn}): ${q.message}`
		: q.message;
	return q.context ? `${msg}\n\n${q.context}` : msg;
}

export function clampWait(v: unknown): number {
	const n = typeof v === 'number' && Number.isFinite(v) ? Math.floor(v) : DEFAULT_WAIT_S;
	return Math.min(MAX_WAIT_S, Math.max(1, n));
}

export async function callTool(name: string, args: unknown, ctx: ToolContext): Promise<ToolResult> {
	const a = (args && typeof args === 'object' ? args : {}) as Record<string, unknown>;
	switch (name) {
		case 'next_question': {
			const r = await ctx.hub.poll(ctx.clientId, clampWait(a.wait_seconds) * 1000, ctx.signal);
			if (r === 'closed') return text({status: 'closed', note: 'xplain closed the session. Stop.'});
			if (r === null) {
				return text({status: 'no_question_yet', call_again: true, note: `No question yet. ${AGAIN}`});
			}
			ctx.onDeliver?.(r);
			return text({
				status: 'question',
				thread_id: r.threadId,
				turn: r.turn,
				follow_up: !!r.followUp,
				...(r.followUp
					? {previous: (r.history ?? []).map((h) => ({turn: h.turn, question: h.message, answer: h.answer ?? ''}))}
					: {}),
				question: formatQuestion(r),
			});
		}
		case 'answer': {
			if (typeof a.thread_id !== 'string' || typeof a.text !== 'string') {
				return text('thread_id and text (strings) are required', true);
			}
			if (!ctx.hub.answer(a.thread_id, a.text)) return text(`Unknown thread_id: ${a.thread_id.slice(0, 100)}`, true);
			return text({ok: true, note: `Answer delivered. ${AGAIN}`});
		}
		case 'get_questions':
			return text({
				questions: ctx.hub.pending().map((q) => ({
					thread_id: q.threadId,
					turn: q.turn,
					follow_up: !!q.followUp,
					preview: q.message.slice(0, 200),
				})),
			});
		case 'annotate': {
			if (typeof a.file !== 'string' || typeof a.text !== 'string' || typeof a.line !== 'number') {
				return text('file (string), line (number) and text (string) are required', true);
			}
			const side = a.side === 'old' || a.side === 'new' ? a.side : undefined;
			ctx.hub.annotate({file: a.file, line: a.line, text: a.text, ...(side ? {side} : {})});
			return text({ok: true});
		}
		default:
			return text(`Unknown tool: ${name.slice(0, 100)}`, true);
	}
}
