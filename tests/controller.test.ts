import {createAskController, type Question} from '../src/ask/index.js';

let fail = 0;
const ok = (n: string, c: boolean) => {
	console.log(c ? 'PASS' : 'FAIL', n);
	if (!c) fail++;
};

const Q = (message = 'why?'): Question => ({file: 'a.ts', index: 0, text: 'x', message, side: 'new', line: 1});

{
	// add/edit/remove
	const ctl = createAskController();
	let notes = 0;
	const un = ctl.subscribe(() => notes++);
	const s0 = ctl.getState();
	const q = ctl.add(Q());
	const q2 = ctl.add(Q('b'));
	ok('add assigns sequential ids', q.id === 'q1' && q2.id === 'q2');
	ok(
		'snapshots immutable',
		ctl.getState() !== s0 && s0.questions.length === 0 && ctl.getState().questions.length === 2,
	);
	ok(
		'edit updates message',
		ctl.edit('q1', 'new msg')?.message === 'new msg' && ctl.getState().questions[0]?.message === 'new msg',
	);
	ok('edit unknown', ctl.edit('zz', 'x') === undefined);
	ok('saved-only has no answer', ctl.answer('q1') === undefined && !ctl.isLive('q1'));
	const n1 = notes;
	ok('subscribe notified', n1 >= 3);

	// setAnswer
	const q1 = ctl.setAnswer('q1', {status: 'done', text: 'because'});
	ok('setAnswer returns question with answer', q1?.answer?.text === 'because' && q1.answer.status === 'done');
	ok('setAnswer published', ctl.getState().answers['q1']?.text === 'because' && ctl.answer('q1')?.tools === 0);
	ok('setAnswer stored on question', ctl.getState().questions[0]?.answer?.status === 'done');
	ok('setAnswer keeps other comments untouched', ctl.getState().questions[1]?.answer === undefined);
	ok('setAnswer unknown', ctl.setAnswer('zz', {status: 'done', text: 'x'}) === undefined);
	ctl.setAnswer('q2', {status: 'error', text: '', error: 'boom'});
	ok('setAnswer error', ctl.answer('q2')?.error === 'boom');
	ctl.setAnswer('q2', {status: 'streaming', text: 'a'});
	ok('isLive while streaming', ctl.isLive('q2'));
	ctl.setAnswer('q2', {status: 'done', text: 'ab'});
	ok('replace answer, not live', ctl.answer('q2')?.text === 'ab' && !ctl.isLive('q2'));

	// remove
	const removed = ctl.remove('q1');
	ok(
		'remove drops question + answer',
		removed?.id === 'q1' && ctl.getState().questions.length === 1 && !('q1' in ctl.getState().answers),
	);
	ok('remove unknown', ctl.remove('zz') === undefined);
	const n2 = notes;
	un();
	ctl.add(Q('c'));
	ok('unsubscribe stops notifications', notes === n2);
	ctl.dispose();
	ctl.dispose(); // idempotent: must not throw
}

{
	// turns / follow-ups
	const ctl = createAskController();
	const q = ctl.add(Q('first'));
	const id = q.id!;
	ok('add sets one turn', q.turns?.length === 1 && q.turns[0]!.message === 'first');
	ok('followUp refused unanswered', ctl.followUp(id, 'f') === undefined && ctl.turns(id).length === 1);
	ctl.setAnswer(id, {status: 'pending', text: ''});
	ok('followUp refused while live', ctl.followUp(id, 'f') === undefined);
	ctl.setAnswer(id, {status: 'done', text: 'A1'});
	ok('turn 1 answer stored', ctl.turns(id)[0]!.answer?.text === 'A1');
	ok('followUp unknown', ctl.followUp('zz', 'f') === undefined);
	const s0 = ctl.getState();
	ok('followUp returns turn number', ctl.followUp(id, 'second') === 2);
	ok('immutable on followUp', s0.questions[0]!.turns!.length === 1 && ctl.getState().questions[0]!.turns!.length === 2);
	const q2 = ctl.getState().questions[0]!;
	ok('message stays first', q2.message === 'first');
	ok('top-level answer cleared for new turn', q2.answer === undefined && ctl.answer(id) === undefined);
	ok('latestTurn', ctl.latestTurn(id)?.turn === 2 && ctl.latestTurn(id)?.message === 'second');
	ok('edit guard with >1 turns', ctl.edit(id, 'x') === undefined && ctl.getState().questions[0]!.message === 'first');
	ctl.setAnswer(id, {status: 'streaming', text: ''});
	ok('followUp refused while turn 2 live', ctl.followUp(id, 'f') === undefined && ctl.isLive(id));
	ctl.setAnswer(id, {status: 'done', text: 'A2'});
	ok('mirror = latest', ctl.answer(id)?.text === 'A2' && ctl.getState().questions[0]!.answer?.text === 'A2');
	ctl.setAnswer(id, {status: 'done', text: 'A1b'}, 1);
	ok(
		'setAnswer explicit turn keeps mirror on latest',
		ctl.turns(id)[0]!.answer?.text === 'A1b' && ctl.answer(id)?.text === 'A2',
	);
	ok('setAnswer bad turn', ctl.setAnswer(id, {status: 'done', text: 'x'}, 3) === undefined);
	ok('followUp turn 3', ctl.followUp(id, 'third') === 3);
	ctl.setAnswer(id, {status: 'done', text: 'A3b'}, 2);
	ok('mirror empty while latest unanswered', ctl.answer(id) === undefined);
	ok('remove thread', ctl.remove(id)?.id === id && ctl.turns(id).length === 0);
	// single-turn edit keeps turns in sync
	const e = ctl.add(Q('one'));
	ok('edit single turn', ctl.edit(e.id!, 'two')?.turns?.[0]!.message === 'two');
	// agent annotations refuse follow-ups
	const ag = ctl.add({...Q('note'), origin: 'agent'});
	ctl.setAnswer(ag.id!, {status: 'done', text: 'x'});
	ok('agent origin refuses followUp', ctl.followUp(ag.id!, 'f') === undefined);
	ctl.dispose();
}

console.log(fail ? `${fail} FAILED` : 'ALL PASS');
process.exit(fail ? 1 : 0);
