import {rmSync, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, writeFileSync} from 'node:fs';
import {join} from 'node:path';
import {DEFAULTS, configPath, loadConfig, resolve, saveConfig} from '../src/config.js';

let fail = 0;
const ok = (n: string, c: boolean) => {
	console.log(c ? 'PASS' : 'FAIL', n);
	if (!c) fail++;
};
const base = join(process.env.CLAUDE_JOB_DIR ?? process.env.TMPDIR ?? '/tmp', 'tmp');
mkdirSync(base, {recursive: true});
const made: string[] = [];
const tmp = () => {
	const d = mkdtempSync(join(base, 'cfg-'));
	made.push(d);
	return d;
};
const capture = <T>(fn: () => T): [T, string] => {
	const orig = process.stderr.write.bind(process.stderr);
	let err = '';
	process.stderr.write = ((s: string) => ((err += s), true)) as typeof process.stderr.write;
	try {
		return [fn(), err];
	} finally {
		process.stderr.write = orig;
	}
};

{
	const p = join(tmp(), 'x', 'config.json');
	const {config, broken} = loadConfig(p);
	ok('missing -> defaults', JSON.stringify(config) === JSON.stringify(DEFAULTS) && !broken);
	ok('missing -> not created', !existsSync(p) && !existsSync(join(p, '..')));
}
{
	const d = tmp();
	const p = join(d, 'a', 'b', 'config.json');
	ok('save ok', saveConfig(p, {theme: 'light'}) === undefined);
	ok('save creates dir+file', JSON.parse(readFileSync(p, 'utf8')).theme === 'light');
	ok('atomic: no temp left', readdirSync(join(d, 'a', 'b')).join() === 'config.json');
}
{
	const p = join(tmp(), 'config.json');
	writeFileSync(
		p,
		JSON.stringify({version: 1, theme: 'dull', foo: {bar: 1}, view: {split: true, extra: 'x'}, agent: {a: 1}}),
	);
	saveConfig(p, {theme: 'vibrant'});
	const j = JSON.parse(readFileSync(p, 'utf8'));
	ok('unknown keys preserved', j.foo.bar === 1 && j.view.extra === 'x' && j.view.split === true && j.agent.a === 1);
	ok('patch applied', j.theme === 'vibrant');
}
{
	const p = join(tmp(), 'config.json');
	writeFileSync(p, JSON.stringify({theme: 'nope', view: {mode: 'staged', split: 'yes'}}));
	const [{config}, err] = capture(() => loadConfig(p));
	ok('invalid theme -> default + warning', config.theme === DEFAULTS.theme && err.includes('invalid theme'));
	ok(
		'per-field: valid kept, bad defaulted',
		config.view.mode === 'staged' && config.view.split === false && err.includes('view.split'),
	);
}
{
	const p = join(tmp(), 'config.json');
	const bad = '{"theme": "light",';
	writeFileSync(p, bad);
	const [{config, broken}, err] = capture(() => loadConfig(p));
	ok('malformed -> defaults + warn + broken', broken && config.theme === DEFAULTS.theme && err.includes('config'));
	const e = saveConfig(p, {theme: 'dull'});
	ok('malformed -> save refused', typeof e === 'string');
	ok('malformed -> file unchanged', readFileSync(p, 'utf8') === bad && readdirSync(join(p, '..')).length === 1);
}
{
	const c = {...DEFAULTS, theme: 'light' as const, view: {...DEFAULTS.view, split: true}};
	ok('config used when no flag', resolve(c, {}).theme === 'light' && resolve(c, {}).split === true);
	ok(
		'flag beats config',
		resolve(c, {theme: 'dull', split: false}).theme === 'dull' && !resolve(c, {split: false}).split,
	);
}
{
	const d = tmp();
	const p = join(d, 'config.json');
	writeFileSync(p, JSON.stringify({app: {confirmQuit: false, extra: 1}}));
	ok('confirmQuit loaded', loadConfig(p).config.app.confirmQuit === false);
	saveConfig(p, {app: {confirmQuit: true}});
	const j = JSON.parse(readFileSync(p, 'utf8'));
	ok('confirmQuit merge keeps siblings', j.app.confirmQuit === true && j.app.extra === 1);
	const p2 = join(d, 'bad.json');
	writeFileSync(p2, JSON.stringify({app: {confirmQuit: 'nope'}}));
	const [{config}, err] = capture(() => loadConfig(p2));
	ok('invalid confirmQuit ignored + warning', config.app.confirmQuit === true && err.includes('app.confirmQuit'));
	const c = {...DEFAULTS, app: {confirmQuit: false}};
	ok('resolve: config used', resolve(c, {}).confirmQuit === false && resolve(DEFAULTS, {}).confirmQuit === true);
	ok('resolve: flag beats config', resolve(c, {confirmQuit: true}).confirmQuit === true);
}
{
	const d = tmp();
	const p = join(d, 'config.json');
	ok('mcp autostart default off', DEFAULTS.mcp.autostart === false && resolve(DEFAULTS, {}).mcpAutostart === false);
	writeFileSync(p, JSON.stringify({app: {confirmQuit: false}, mcp: {autostart: true, extra: 1}}));
	const l = loadConfig(p).config;
	ok('mcp autostart on round-trips', l.mcp.autostart === true && resolve(l, {}).mcpAutostart === true);
	saveConfig(p, {mcp: {autostart: false}});
	const j = JSON.parse(readFileSync(p, 'utf8'));
	ok(
		'mcp autostart off saved, merge keeps others',
		j.mcp.autostart === false && j.mcp.extra === 1 && j.app.confirmQuit === false,
	);
	ok('mcp autostart off resolves', resolve(loadConfig(p).config, {}).mcpAutostart === false);
	const p2 = join(d, 'bad.json');
	writeFileSync(p2, JSON.stringify({mcp: {autostart: 'yes'}}));
	const [{config}, err] = capture(() => loadConfig(p2));
	ok('invalid mcp.autostart ignored + warning', config.mcp.autostart === false && err.includes('mcp.autostart'));
}
{
	ok('flag path wins', configPath('/f', {XPLAIN_CONFIG: '/e', XDG_CONFIG_HOME: '/x'}) === '/f');
	ok('env path', configPath(undefined, {XPLAIN_CONFIG: '/e', XDG_CONFIG_HOME: '/x'}) === '/e');
	ok('xdg path', configPath(undefined, {XDG_CONFIG_HOME: '/x'}) === '/x/xplain/config.json');
}
for (const d of made) rmSync(d, {recursive: true, force: true});
process.exit(fail ? 1 : 0);
