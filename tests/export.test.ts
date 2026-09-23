import {renderReviewMarkdown, exportName, type Question} from '../src/ask/index.js';
import {threadBody} from '../src/components/answerView.js';

let fail = 0;
const ok = (n: string, c: boolean) => {
	console.log(c ? 'PASS' : 'FAIL', n);
	if (!c) fail++;
};
const meta = {cwd: '/r', mode: 'all', args: ['HEAD~1'], date: new Date('2026-01-02T03:04:05Z')};
const qs: Question[] = [
	{file: 'b.ts', index: 3, side: 'new', line: 9, text: 'const x = `a`', message: 'why?', context: ['l1', 'l2']},
	{
		file: 'a.ts',
		index: 5,
		line: 20,
		text: 'sel',
		message: 'second',
		startLine: 20,
		endLine: 22,
		startCol: 2,
		endCol: 8,
		turns: [
			{message: 'second', answer: {status: 'done', text: 'because\n```js\nx\n```', tools: 0, agent: 'bot'}},
			{message: 'and then?', answer: {status: 'error', text: '', error: 'boom', tools: 0, agent: 'bot'}},
		],
		answer: {status: 'error', text: '', error: 'boom', tools: 0, agent: 'bot'},
	},
	{
		file: 'a.ts',
		index: 1,
		side: 'old',
		line: 2,
		text: 'old',
		message: 'first',
		answer: {status: 'cancelled', text: '', tools: 0},
	},
	{file: 'c.ts', index: 1, line: 1, text: 't', message: 'note', origin: 'agent'},
];
const md = renderReviewMarkdown(qs, meta);
ok(
	'title + meta',
	md.startsWith('# xplain review') && md.includes('`/r`') && md.includes('`all HEAD~1`') && md.includes('2026-01-02'),
);
ok(
	'files sorted',
	md.indexOf('## `a.ts`') < md.indexOf('## `b.ts`') && md.indexOf('## `b.ts`') < md.indexOf('## `c.ts`'),
);
ok('lines sorted in file', md.indexOf('old side, line 2') < md.indexOf('selection lines 20-22'));
ok('selection cols', md.includes('lines 20-22, cols 2-8') && md.includes('Selected text:'));
ok('context block', md.includes('Context:') && md.includes('l1\nl2'));
ok(
	'follow-up + answers in order',
	md.indexOf('**Comment:**') >= 0 && md.indexOf('**Follow-up 1:**') > md.indexOf('Answer (bot) - done'),
);
ok('error status + text', md.includes('Answer (bot) - error') && md.includes('boom'));
ok('states', md.includes('state: cancelled') && md.includes('state: saved') && md.includes('state: error'));
ok('agent origin', md.includes('origin: agent') && md.includes('origin: human') && md.includes('**Note:**'));
ok(
	'backtick in content gets longer fence',
	md.includes('````\nconst x = `a`\n````') === false && md.includes('```\nconst x = `a`\n```'),
);
const md2 = renderReviewMarkdown([{file: 'f', index: 0, line: 1, text: '```\nx\n```', message: 'm'}], meta);
ok('triple backticks get 4-fence', md2.includes('````\n```\nx\n```\n````'));
ok('empty', renderReviewMarkdown([], meta).includes('No comments.'));
ok('name', exportName(new Date(2026, 0, 2, 3, 4, 5)) === 'xplain-review-20260102-030405.md');
// several answers on one turn: all kept, in order
const pa = (text: string) => ({status: 'done' as const, text, tools: 0});
const multi: Question = {
	file: 'm',
	index: 0,
	text: 'x',
	message: 'go',
	turns: [{message: 'go', prior: [pa('starting work')], answer: pa('all done')}],
};
const md3 = renderReviewMarkdown([multi], meta);
ok(
	'prior answers exported in order',
	md3.indexOf('starting work') >= 0 && md3.indexOf('starting work') < md3.indexOf('all done'),
);
const tb = threadBody(multi.turns!, 'go', 40);
ok(
	'threadBody shows every answer',
	tb.filter((l) => l.k === 'div').length === 2 &&
		tb.findIndex((l) => l.t === 'starting work') < tb.findIndex((l) => l.t === 'all done'),
);
process.exit(fail ? 1 : 0);
