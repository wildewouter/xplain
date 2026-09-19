import {existsSync, mkdirSync, readFileSync, renameSync, unlinkSync, writeFileSync} from 'node:fs';
import {homedir} from 'node:os';
import {dirname, join} from 'node:path';
import {MODES, type Mode} from './diff/load.js';
import {isTheme, THEME_NAMES, type ThemeName} from './theme.js';

export {DEFAULTS, type Config} from './defaults.js';
import {DEFAULTS, type Config} from './defaults.js';

export const configPath = (flag?: string, env = process.env): string =>
	flag || env.XPLAIN_CONFIG || join(env.XDG_CONFIG_HOME || join(homedir(), '.config'), 'xplain', 'config.json');

export type Loaded = {config: Config; broken: boolean};

const warn = (m: string) => process.stderr.write(`xplain: config: ${m}\n`);
const isObj = (v: unknown): v is Record<string, unknown> => !!v && typeof v === 'object' && !Array.isArray(v);

export function loadConfig(path: string): Loaded {
	const config: Config = structuredClone(DEFAULTS);
	if (!existsSync(path)) return {config, broken: false};
	let raw: unknown;
	try {
		raw = JSON.parse(readFileSync(path, 'utf8'));
		if (!isObj(raw)) throw new Error('top level must be an object');
	} catch (e) {
		warn(`${path}: ${(e as Error).message}; using defaults, file will not be modified`);
		return {config, broken: true};
	}
	const r = raw as Record<string, unknown>;
	if ('theme' in r) {
		if (isTheme(r.theme)) config.theme = r.theme;
		else warn(`invalid theme ${JSON.stringify(r.theme)} (${THEME_NAMES.join('|')}); using ${DEFAULTS.theme}`);
	}
	if ('view' in r) {
		if (!isObj(r.view)) warn('view must be an object; using defaults');
		else {
			const v = r.view;
			if ('mode' in v) {
				if (MODES.includes(v.mode as Mode)) config.view.mode = v.mode as Mode;
				else warn(`invalid view.mode ${JSON.stringify(v.mode)} (${MODES.join('|')}); using ${DEFAULTS.view.mode}`);
			}
			for (const k of ['split', 'full'] as const) {
				if (!(k in v)) continue;
				if (typeof v[k] === 'boolean') config.view[k] = v[k];
				else warn(`invalid view.${k} ${JSON.stringify(v[k])} (boolean); using ${DEFAULTS.view[k]}`);
			}
		}
	}
	if ('app' in r) {
		if (!isObj(r.app)) warn('app must be an object; using defaults');
		else if ('confirmQuit' in r.app) {
			if (typeof r.app.confirmQuit === 'boolean') config.app.confirmQuit = r.app.confirmQuit;
			else
				warn(
					`invalid app.confirmQuit ${JSON.stringify(r.app.confirmQuit)} (boolean); using ${DEFAULTS.app.confirmQuit}`,
				);
		}
	}
	return {config, broken: false};
}

const merge = (a: Record<string, unknown>, b: Record<string, unknown>): Record<string, unknown> => {
	const o = {...a};
	for (const [k, v] of Object.entries(b)) o[k] = isObj(v) && isObj(o[k]) ? merge(o[k], v) : v;
	return o;
};

export type Patch = {theme?: ThemeName; view?: Partial<Config['view']>; app?: Partial<Config['app']>};

/** Returns error message, or undefined on success. */
export function saveConfig(path: string, patch: Patch): string | undefined {
	let cur: Record<string, unknown> = {version: 1};
	if (existsSync(path)) {
		try {
			const j: unknown = JSON.parse(readFileSync(path, 'utf8'));
			if (!isObj(j)) throw new Error('not an object');
			cur = j;
		} catch {
			return `config unreadable, not saved (${path})`;
		}
	}
	const tmp = `${path}.${process.pid}.tmp`;
	try {
		mkdirSync(dirname(path), {recursive: true});
		writeFileSync(tmp, JSON.stringify(merge(cur, patch), null, '\t') + '\n');
		renameSync(tmp, path);
	} catch (e) {
		try {
			unlinkSync(tmp);
		} catch {}
		return `config save failed: ${(e as Error).message}`;
	}
}

/** defaults < config < flags */
export function resolve(
	c: Config,
	f: {theme?: ThemeName; mode?: Mode; split?: boolean; full?: boolean; confirmQuit?: boolean},
) {
	return {
		theme: f.theme ?? c.theme,
		mode: f.mode ?? c.view.mode,
		split: f.split ?? c.view.split,
		full: f.full ?? c.view.full,
		confirmQuit: f.confirmQuit ?? c.app.confirmQuit,
	};
}
