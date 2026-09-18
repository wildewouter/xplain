import {render} from 'ink';
import App from './app.js';
import {MODES, type Mode} from './diff/load.js';
import {THEME_NAMES, isTheme, type ThemeName} from './theme.js';
import {configPath, loadConfig, resolve} from './config.js';

const usage = `usage: xplain [--cwd dir] [--config file] [--mode all|staged|unstaged | --staged | --unstaged] [git diff args...]
  --mode <m>   all (git diff HEAD, default), staged (--cached), unstaged
  --staged     same as --mode staged
  --unstaged   same as --mode unstaged
  --split      start in side-by-side view (s toggles)
  --changes-only  start with git hunks only, not the full file (c toggles)
  --theme <t>  vibrant, dull, contrast, colorblind, light, solarized (default) (t cycles)
  --config <f> config file (default $XPLAIN_CONFIG or ~/.config/xplain/config.json)
  -h, --help   show this help
xplain config path  print the resolved config path
extra git args replace HEAD in "all" mode, and are appended in the other modes.
keys: ? help, s split/unified, c full/changes, ]/[ next/prev change, m cycles mode, t cycles theme, C config, q quits`;

const fail = (msg: string): never => {
	process.stderr.write(`xplain: ${msg}\n${usage}\n`);
	process.exit(1);
};

const argv = process.argv.slice(2);
let cwd: string | undefined;
let mode: Mode | undefined;
let split: boolean | undefined;
let full: boolean | undefined;
let theme: ThemeName | undefined;
let cfgFlag: string | undefined;
const rest: string[] = [];
const setMode = (v: string | undefined) => {
	if (!MODES.includes(v as Mode)) fail(`invalid mode: ${v ?? '(missing)'}`);
	mode = v as Mode;
};
const setTheme = (v: string | undefined) => {
	if (!isTheme(v)) fail(`invalid theme: ${v ?? '(missing)'} (${THEME_NAMES.join('|')})`);
	theme = v as ThemeName;
};
for (let i = 0; i < argv.length; i++) {
	const a = argv[i]!;
	if (a === '-h' || a === '--help') {
		process.stdout.write(usage + '\n');
		process.exit(0);
	} else if (a === '--cwd') cwd = argv[++i] ?? fail('--cwd needs a value');
	else if (a === '--config') cfgFlag = argv[++i] ?? fail('--config needs a value');
	else if (a.startsWith('--config=')) cfgFlag = a.slice(9);
	else if (a === '--mode') setMode(argv[++i]);
	else if (a.startsWith('--mode=')) setMode(a.slice(7));
	else if (a === '--theme') setTheme(argv[++i]);
	else if (a.startsWith('--theme=')) setTheme(a.slice(8));
	else if (a === '--split') split = true;
	else if (a === '--changes-only') full = false;
	else if (a === '--staged') mode = 'staged';
	else if (a === '--unstaged') mode = 'unstaged';
	else rest.push(a);
}

const path = configPath(cfgFlag);
if (rest[0] === 'config' && rest[1] === 'path' && rest.length === 2) {
	process.stdout.write(path + '\n');
	process.exit(0);
}
const v = resolve(loadConfig(path).config, {theme, mode, split, full});
render(<App args={rest} cwd={cwd} {...v} configPath={path} />, {alternateScreen: true});
