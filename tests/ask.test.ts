import {askHint} from '../src/components/AskBox.js';
import {buildPrompt} from '../src/ask/prompt.js';
import {wrapText, answerView} from '../src/components/answerView.js';

let fail = 0;
const ok = (n: string, c: boolean) => {
	console.log(c ? 'PASS' : 'FAIL', n);
	if (!c) fail++;
};

const p1 = buildPrompt({file: 'src/a.ts', index: 3, side: 'new', line: 12, text: 'const x = 1;', message: 'why x?'});
ok('prompt file', p1.includes('File: src/a.ts'));
ok('prompt single line', /^Lines: 12$/m.test(p1));
ok('prompt cursor text', p1.includes('Code at cursor line:') && p1.includes('const x = 1;'));
ok('prompt message first, verbatim', p1.startsWith('why x?\n') && !p1.includes('Question:'));
ok('prompt: no instruction text', !/Read-only|Explain why|Be concise/.test(p1));
ok('prompt no context block', !p1.includes('Surrounding context'));

const p2 = buildPrompt({
	file: 'b.ts',
	index: 1,
	side: 'old',
	line: 5,
	startLine: 5,
	endLine: 7,
	text: 'a\nb\nc',
	message: 'm',
	context: ['before', 'a', 'b', 'c', 'after'],
});
ok('prompt range', /^Lines: 5-7$/m.test(p2));
ok('prompt old side', p2.includes('Side: old'));
ok('prompt selection', p2.includes('Selected code:') && p2.includes('a\nb\nc'));
ok('prompt context', p2.includes('Surrounding context:') && p2.includes('before') && p2.includes('after'));
ok('prompt no line', !/^Lines:/m.test(buildPrompt({file: 'x', index: 0, text: '', message: 'm'})));

ok('wrap words', wrapText('aaa bbb ccc', 7).join('|') === 'aaa bbb|ccc');
ok('wrap hard', wrapText('abcdefgh', 3).join('|') === 'abc|def|gh');
ok('wrap newline+blank', wrapText('a\n\nb', 5).join('|') === 'a||b');
ok(
	'wrap all <= width',
	wrapText('x'.repeat(50) + ' yy ' + 'z'.repeat(20), 9).every((l) => l.length <= 9),
);

const long = Array.from({length: 40}, (_, i) => `l${i}`).join('\n');
const v = answerView({status: 'streaming', text: long, tools: 0, agent: 'claude'}, 20, false);
ok(
	'view cap 12 + more + streaming header',
	v.lines.length === 12 && v.more === 28 && v.head === 'answer · claude · streaming…',
);
const vf = answerView({status: 'done', text: long, tools: 0, agent: 'claude'}, 20, true);
ok('view focused cap 30', vf.lines.length === 30 && vf.more === 10 && vf.head === 'answer · claude · done');
const ve = answerView({status: 'error', text: '', error: 'boom', tools: 0, agent: 'claude'}, 20, false);
ok('view error', ve.lines[0] === 'boom' && ve.head.endsWith('error'));
ok('view pending empty', answerView({status: 'pending', text: '', tools: 0}, 20, false).lines.length === 0);

ok('hint edit no chip', askHint(60) === 'enter send  esc cancel');

const pq = {
	file: 'f.ts',
	index: 1,
	side: 'new' as const,
	line: 5,
	text: 'x',
	message: 'm',
	context: ['c3'],
	wide: ['c3', 'w15'],
};
const pf = buildPrompt(pq);
ok('prompt: near ctx used, wide ctx excluded', pf.includes('c3') && !pf.includes('w15'));

process.exit(fail ? 1 : 0);
