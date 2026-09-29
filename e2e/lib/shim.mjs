// Fake external CLI. Invoked as: node shim.mjs <shimDir> <name> [argv...]
// Config from <shimDir>/<name>.json ({rules, ctl}); first rule whose `match` is an argv prefix wins (none: exit 0,
// no output). Each call appends {name, argv, cwd, stdin?} as a JSON line to <shimDir>/calls.log, then:
// `block`: waits until the runner (TCP 127.0.0.1:<ctl>) says go; `passthrough`: runs the real program (`exec`).
import {spawnSync} from 'node:child_process';
import {appendFileSync, readFileSync} from 'node:fs';
import {connect} from 'node:net';

const [dir, name, ...argv] = process.argv.slice(2);
let cfg = {rules: []};
try {
	cfg = JSON.parse(readFileSync(`${dir}/${name}.json`, 'utf8'));
} catch {}
const rule = cfg.rules.find((r) => !r.match || r.match.every((a, i) => argv[i] === a)) ?? {};
const stdin = rule.readStdin ? readFileSync(0, 'utf8') : undefined;
appendFileSync(
	`${dir}/calls.log`,
	JSON.stringify({name, argv, cwd: process.cwd(), ...(stdin !== undefined ? {stdin} : {})}) + '\n',
);
if (rule.block)
	await new Promise((resolve) => {
		let go = false;
		const s = connect(cfg.ctl, '127.0.0.1', () => s.write(`${name}\n`));
		s.on('data', () => ((go = true), s.destroy(), resolve()));
		s.on('error', () => {}); // close follows
		s.on('close', () => go || process.exit(97)); // runner gone
	});
if (rule.passthrough) {
	const r = spawnSync(rule.exec, argv, {stdio: 'inherit'});
	process.exit(r.status ?? 1);
}
if (rule.stdout) process.stdout.write(rule.stdout);
if (rule.stderr) process.stderr.write(rule.stderr);
process.exitCode = rule.exit ?? 0;
