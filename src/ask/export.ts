// Headless markdown export of comments and their threads. No UI imports.
import type {Answer, Question, Turn} from './types.js';

export type ExportMeta = {cwd: string; mode: string; args?: string[]; date?: Date};

const turnsOf = (q: Question): Turn[] => q.turns ?? [{message: q.message, ...(q.answer ? {answer: q.answer} : {})}];

/** Fence longer than any backtick run in `s` (min 3). */
const fence = (s: string) => {
	const runs = s.match(/`+/g) ?? [];
	return '`'.repeat(Math.max(3, ...runs.map((r) => r.length + 1)));
};
const block = (s: string, lang = '') => {
	const f = fence(s);
	return `${f}${lang}\n${s}\n${f}`;
};
const quote = (s: string) =>
	s
		.split('\n')
		.map((l) => (l ? `> ${l}` : '>'))
		.join('\n');
const inline = (s: string) => {
	const f = fence(s).slice(2); // one backtick more than the longest run
	return `${f}${s.startsWith('`') || s.endsWith('`') ? ` ${s} ` : s}${f}`;
};

export const stamp = (d: Date) => {
	const p = (n: number) => String(n).padStart(2, '0');
	return `${d.getFullYear()}${p(d.getMonth() + 1)}${p(d.getDate())}-${p(d.getHours())}${p(d.getMinutes())}${p(d.getSeconds())}`;
};
export const exportName = (d: Date) => `xplain-review-${stamp(d)}.md`;

const stateOf = (q: Question) => {
	const a = q.answer;
	if (!a) return 'saved';
	return a.status === 'done' ? 'answered' : a.status;
};
const where = (q: Question) => {
	const side = q.side ?? 'new';
	if (q.startLine !== undefined && q.endLine !== undefined) {
		const cols = q.startCol !== undefined && q.endCol !== undefined ? `, cols ${q.startCol}-${q.endCol}` : '';
		const r = q.startLine === q.endLine ? `line ${q.startLine}` : `lines ${q.startLine}-${q.endLine}`;
		return `${side} side, selection ${r}${cols}`;
	}
	return q.line !== undefined ? `${side} side, line ${q.line}` : `${side} side, row ${q.index + 1}`;
};
const answerHead = (a: Answer) => `Answer${a.agent ? ` (${a.agent})` : ''} - ${a.status}`;

const renderComment = (q: Question, n: number) => {
	const out: string[] = [`### ${n}. ${where(q)}`, ''];
	out.push(`- origin: ${q.origin ?? 'human'}`, `- state: ${stateOf(q)}`, '');
	if (q.text) out.push(q.startLine !== undefined ? 'Selected text:' : 'Line:', '', block(q.text), '');
	if (q.context?.length) out.push('Context:', '', block(q.context.join('\n')), '');
	turnsOf(q).forEach((t, i) => {
		out.push(
			i === 0 ? `**${q.origin === 'agent' ? 'Note' : 'Comment'}:**` : `**Follow-up ${i}:**`,
			'',
			quote(t.message),
			'',
		);
		if (t.answer) {
			const a = t.answer;
			out.push(`**${answerHead(a)}**`, '');
			if (a.text) out.push(quote(a.text), '');
			if (a.error) out.push(`Error: ${inline(a.error)}`, '');
		}
	});
	return out.join('\n');
};

export function renderReviewMarkdown(comments: Question[], meta: ExportMeta): string {
	const byFile = new Map<string, Question[]>();
	for (const q of comments) byFile.set(q.file, [...(byFile.get(q.file) ?? []), q]);
	const key = (q: Question) => q.startLine ?? q.line ?? q.index;
	const out: string[] = [
		'# xplain review',
		'',
		`- repo: ${inline(meta.cwd)}`,
		`- diff: ${inline([meta.mode, ...(meta.args ?? [])].join(' '))}`,
		`- date: ${(meta.date ?? new Date()).toISOString()}`,
		`- comments: ${comments.length}`,
		'',
	];
	if (!comments.length) out.push('No comments.', '');
	let n = 0;
	for (const file of [...byFile.keys()].sort()) {
		out.push(`## ${inline(file)}`, '');
		for (const q of byFile.get(file)!.sort((a, b) => key(a) - key(b))) out.push(renderComment(q, ++n), '');
	}
	return (
		out
			.join('\n')
			.replace(/\n{3,}/g, '\n\n')
			.trimEnd() + '\n'
	);
}
