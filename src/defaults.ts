import type {Mode} from './diff/load.js';
import type {ThemeName} from './theme.js';

export type Config = {
	version: 1;
	theme: ThemeName;
	view: {mode: Mode; split: boolean; full: boolean};
	agent: Record<string, unknown>; // reserved
	keys: Record<string, unknown>; // reserved
};

export const DEFAULTS: Config = {
	version: 1,
	theme: 'solarized',
	view: {mode: 'all', split: false, full: true},
	agent: {},
	keys: {},
};
