import type {Question} from './types.js';

// file/side/lines/code + surrounding context block (no question text)
export function buildContext(q: Question): string {
	const ctx = q.context;
	const range =
		q.startLine !== undefined && q.endLine !== undefined && q.startLine !== q.endLine
			? `${q.startLine}-${q.endLine}`
			: q.startLine !== undefined || q.line !== undefined
				? String(q.startLine ?? q.line)
				: undefined;
	const sel = q.startLine !== undefined;
	return [
		`File: ${q.file}`,
		`Side: ${q.side === 'old' ? 'old (before the change)' : 'new (after the change)'}`,
		range ? `Lines: ${range}` : undefined,
		'',
		sel ? 'Selected code:' : 'Code at cursor line:',
		'```',
		q.text,
		'```',
		...(ctx?.length ? ['', 'Surrounding context:', '```', ...ctx, '```'] : []),
	]
		.filter((l) => l !== undefined)
		.join('\n');
}

export function buildPrompt(q: Question): string {
	return [q.message, '', '---', buildContext(q)].join('\n');
}
