import {render} from 'ink';
import App from './app.js';
import {MODES, type Mode} from './diff/load.js';

const usage = `usage: xplain [--cwd dir] [--mode all|staged|unstaged | --staged | --unstaged] [git diff args...]
  --mode <m>   all (git diff HEAD, default), staged (--cached), unstaged
  --staged     same as --mode staged
  --unstaged   same as --mode unstaged
  -h, --help   show this help
extra git args replace HEAD in "all" mode, and are appended in the other modes.
keys: m cycles mode, q quits`;

const fail = (msg: string): never => {
	process.stderr.write(`xplain: ${msg}\n${usage}\n`);
	process.exit(1);
};

const argv = process.argv.slice(2);
let cwd: string | undefined;
let mode: Mode = 'all';
const rest: string[] = [];
const setMode = (v: string | undefined) => {
	if (!MODES.includes(v as Mode)) fail(`invalid mode: ${v ?? '(missing)'}`);
	mode = v as Mode;
};
for (let i = 0; i < argv.length; i++) {
	const a = argv[i]!;
	if (a === '-h' || a === '--help') {
		process.stdout.write(usage + '\n');
		process.exit(0);
	} else if (a === '--cwd') cwd = argv[++i] ?? fail('--cwd needs a value');
	else if (a === '--mode') setMode(argv[++i]);
	else if (a.startsWith('--mode=')) setMode(a.slice(7));
	else if (a === '--staged') mode = 'staged';
	else if (a === '--unstaged') mode = 'unstaged';
	else rest.push(a);
}

render(<App args={rest} cwd={cwd} mode={mode} />);
