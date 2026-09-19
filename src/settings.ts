import {MODES, type Mode} from './diff/load.js';
import {THEME_NAMES, type ThemeName} from './theme.js';
import type {Patch} from './config.js';

export type SettingsState = {theme: ThemeName; mode: Mode; split: boolean; full: boolean; confirmQuit: boolean};
// live-apply hooks provided by the app
export type Actions = {
	theme(v: ThemeName): void;
	mode(v: Mode): void;
	split(v: boolean): void;
	full(v: boolean): void;
	confirmQuit(v: boolean): void;
};
export type Setting = {
	path: string; // config key path
	label: string;
	choices: readonly string[];
	get(s: SettingsState): string;
	set(v: string, a: Actions): void; // apply live
	patch(v: string): Patch; // persist
};

const bool = (v: string) => v === 'on' || v === 'full';

export const SETTINGS: Setting[] = [
	{
		path: 'theme',
		label: 'theme',
		choices: THEME_NAMES,
		get: (s) => s.theme,
		set: (v, a) => a.theme(v as ThemeName),
		patch: (v) => ({theme: v as ThemeName}),
	},
	{
		path: 'view.mode',
		label: 'mode',
		choices: MODES,
		get: (s) => s.mode,
		set: (v, a) => a.mode(v as Mode),
		patch: (v) => ({view: {mode: v as Mode}}),
	},
	{
		path: 'view.split',
		label: 'split',
		choices: ['off', 'on'],
		get: (s) => (s.split ? 'on' : 'off'),
		set: (v, a) => a.split(bool(v)),
		patch: (v) => ({view: {split: bool(v)}}),
	},
	{
		path: 'view.full',
		label: 'view',
		choices: ['full', 'changes'],
		get: (s) => (s.full ? 'full' : 'changes'),
		set: (v, a) => a.full(bool(v)),
		patch: (v) => ({view: {full: bool(v)}}),
	},
	{
		path: 'app.confirmQuit',
		label: 'confirm quit',
		choices: ['off', 'on'],
		get: (s) => (s.confirmQuit ? 'on' : 'off'),
		set: (v, a) => a.confirmQuit(bool(v)),
		patch: (v) => ({app: {confirmQuit: bool(v)}}),
	},
];
