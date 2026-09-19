import {waitFor, settle, keyAndWait, makePress} from './helpers.js';

let fail = 0;
const ok = (n: string, c: boolean) => {
	console.log(c ? 'PASS' : 'FAIL', n);
	if (!c) fail++;
};

let t = Date.now();
ok('waitFor true resolves true', (await waitFor(() => true)) === true);
ok('waitFor true is fast', Date.now() - t < 20);

t = Date.now();
ok('waitFor times out false', (await waitFor(() => false, {timeout: 60, interval: 5})) === false);
const el = Date.now() - t;
ok('waitFor timeout respected', el >= 55 && el < 300);

let n = 0;
const at: number[] = [];
t = Date.now();
await waitFor(() => (at.push(Date.now()), ++n >= 4), {interval: 20});
ok('waitFor interval respected', n === 4 && Date.now() - t >= 55 && at[1]! - at[0]! >= 15);

ok(
	'waitFor throwing pred does not throw',
	(await waitFor(
		() => {
			throw new Error('x');
		},
		{timeout: 20},
	)) === false,
);
let flip = false;
setTimeout(() => (flip = true), 30);
ok('waitFor picks up later change', (await waitFor(() => flip)) === true);
ok('waitFor async pred', (await waitFor(async () => true)) === true);

t = Date.now();
await settle(30);
ok('settle sleeps', Date.now() - t >= 25);

const writes: string[] = [];
const stdin = {write: (s: string) => writes.push(s)};
ok('keyAndWait no pred', (await keyAndWait(stdin, 'a')) === true && writes[0] === 'a');
ok('keyAndWait pred false', (await keyAndWait(stdin, 'b', () => false, {timeout: 20})) === false);

let frame = 'one';
const press = makePress({write: (s: string) => setTimeout(() => (frame = `f:${s}`), 10)}, () => frame);
ok('press until string', (await press('x', 'f:x')) === true);
ok('press until regexp', (await press('y', /f:y/)) === true);
ok('press until fn', (await press('z', (f) => f === 'f:z')) === true);
ok('press no until detects change', (await press('w')) === true && frame === 'f:w');
ok('press until fail', (await press('q', 'nope')) === false);

if (fail) process.exit(1);
