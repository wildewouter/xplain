import {createContext, useContext} from 'react';
import chalk from 'chalk';
import {DEFAULTS} from './defaults.js';

export type Theme = {
	syntax: Record<string, string>; // cli-highlight token -> color
	bold: boolean; // bold syntax tokens
	addBg: string;
	delBg: string;
	addMark: string;
	delMark: string;
	gutter: string;
	hunk: string;
	mode: string; // [mode] [full] tags
	view: string; // [split] tag
	file: string; // path in header
	adds: string;
	dels: string;
	dim: string; // footer, rules, notes
	accent: string; // help groups, M status
	modalBorder: string;
	modalBg: string;
	modalFg: string; // default text color inside modals
	selBg: string;
	selFg: string;
	curBg: string; // cursor-mode row highlight
	visBg: string; // visual selection chars
	visFg: string;
};

const syn = (o: Record<string, string>, ...same: [string, string[]][]) => {
	for (const [c, ks] of same) for (const k of ks) o[k] = c;
	return o;
};

export const THEMES = {
	vibrant: {
		syntax: syn(
			{},
			['#ff5fd7', ['keyword', 'selector-tag', 'doctag']],
			['#87ff5f', ['string', 'regexp', 'addition']],
			['#ffaf00', ['number', 'literal', 'symbol', 'attr', 'attribute']],
			['#00d7ff', ['built_in', 'type', 'class', 'title', 'name', 'tag', 'selector-class', 'selector-id']],
			['#5fafff', ['function', 'params', 'variable', 'template-variable', 'bullet', 'link']],
			['#7a7a9a', ['comment', 'quote', 'meta', 'deletion']],
		),
		bold: false,
		addBg: '#1f4d2b',
		delBg: '#5a1f26',
		addMark: 'greenBright',
		delMark: 'redBright',
		gutter: 'gray',
		hunk: 'cyan',
		mode: 'yellow',
		view: 'magenta',
		file: 'cyan',
		adds: 'green',
		dels: 'red',
		dim: 'gray',
		accent: 'cyan',
		modalBorder: 'cyan',
		modalBg: 'black',
		modalFg: '#e4e4e4',
		selBg: 'cyan',
		selFg: 'black',
		curBg: '#33336b',
		visBg: '#875f00',
		visFg: '#ffffff',
	},
	dull: {
		syntax: syn(
			{},
			['#a8899c', ['keyword', 'selector-tag', 'doctag']],
			['#8fa38a', ['string', 'regexp', 'addition']],
			['#b39f80', ['number', 'literal', 'symbol', 'attr', 'attribute']],
			['#7f9fa8', ['built_in', 'type', 'class', 'title', 'name', 'tag', 'selector-class', 'selector-id']],
			['#8a9bb0', ['function', 'params', 'variable', 'template-variable', 'bullet', 'link']],
			['#5f6368', ['comment', 'quote', 'meta', 'deletion']],
		),
		bold: false,
		addBg: '#26332a',
		delBg: '#382528',
		addMark: '#7f9c7f',
		delMark: '#a87f7f',
		gutter: '#5f6368',
		hunk: '#7f9fa8',
		mode: '#b39f80',
		view: '#a8899c',
		file: '#9aa5b1',
		adds: '#7f9c7f',
		dels: '#a87f7f',
		dim: '#5f6368',
		accent: '#7f9fa8',
		modalBorder: '#5f6368',
		modalBg: '#1c1c1c',
		modalFg: '#c0c0c0',
		selBg: '#3a3f47',
		selFg: '#d0d0d0',
		curBg: '#3f3f5f',
		visBg: '#6b5a2e',
		visFg: '#f0f0f0',
	},
	contrast: {
		syntax: syn(
			{},
			['#ff00ff', ['keyword', 'selector-tag', 'doctag']],
			['#00b800', ['string', 'regexp', 'addition']],
			['#ff8700', ['number', 'literal', 'symbol', 'attr', 'attribute']],
			['#0087ff', ['built_in', 'type', 'class', 'title', 'name', 'tag', 'selector-class', 'selector-id']],
			['#00afaf', ['function', 'params', 'variable', 'template-variable', 'bullet', 'link']],
			['#808080', ['comment', 'quote', 'meta', 'deletion']],
		),
		bold: true,
		addBg: '#005f00',
		delBg: '#870000',
		addMark: '#ffffff',
		delMark: '#ffffff',
		gutter: '#808080',
		hunk: '#ffffff',
		mode: '#ffff00',
		view: '#ff00ff',
		file: '#00ffff',
		adds: '#00ff00',
		dels: '#ff0000',
		dim: '#808080',
		accent: '#ffff00',
		modalBorder: '#ffffff',
		modalBg: '#000000',
		modalFg: '#ffffff',
		selBg: '#ffff00',
		selFg: '#000000',
		curBg: '#3a3aa8',
		visBg: '#af5f00',
		visFg: '#ffffff',
	},
	colorblind: {
		syntax: syn(
			{},
			['#b48ead', ['keyword', 'selector-tag', 'doctag']],
			['#f0c674', ['string', 'regexp', 'addition']],
			['#ff9f43', ['number', 'literal', 'symbol', 'attr', 'attribute']],
			['#56b6f7', ['built_in', 'type', 'class', 'title', 'name', 'tag', 'selector-class', 'selector-id']],
			['#8ab4f8', ['function', 'params', 'variable', 'template-variable', 'bullet', 'link']],
			['#8a8a8a', ['comment', 'quote', 'meta', 'deletion']],
		),
		bold: false,
		addBg: '#12345a',
		delBg: '#5a3410',
		addMark: '#5fafff',
		delMark: '#ffaf3f',
		gutter: '#808080',
		hunk: '#5fafff',
		mode: '#f0c674',
		view: '#b48ead',
		file: '#56b6f7',
		adds: '#5fafff',
		dels: '#ffaf3f',
		dim: '#8a8a8a',
		accent: '#56b6f7',
		modalBorder: '#56b6f7',
		modalBg: '#000000',
		modalFg: '#e0e0e0',
		selBg: '#56b6f7',
		selFg: '#000000',
		curBg: '#5a5a5a',
		visBg: '#b8a000',
		visFg: '#000000',
	},
	light: {
		syntax: syn(
			{},
			['#8f1f8f', ['keyword', 'selector-tag', 'doctag']],
			['#0a6b1f', ['string', 'regexp', 'addition']],
			['#a34a00', ['number', 'literal', 'symbol', 'attr', 'attribute']],
			['#00609c', ['built_in', 'type', 'class', 'title', 'name', 'tag', 'selector-class', 'selector-id']],
			['#1f3f9f', ['function', 'params', 'variable', 'template-variable', 'bullet', 'link']],
			['#6a6a6a', ['comment', 'quote', 'meta', 'deletion']],
		),
		bold: false,
		addBg: '#d4f0d4',
		delBg: '#f8d4d4',
		addMark: '#0a6b1f',
		delMark: '#a01010',
		gutter: '#6a6a6a',
		hunk: '#00609c',
		mode: '#8a5a00',
		view: '#8f1f8f',
		file: '#00609c',
		adds: '#0a6b1f',
		dels: '#a01010',
		dim: '#6e6e6e',
		accent: '#00609c',
		modalBorder: '#303030',
		modalBg: '#f4f4f4',
		modalFg: '#202020',
		selBg: '#bcd8ff',
		selFg: '#101010',
		curBg: '#ffe9a0',
		visBg: '#7fb2ff',
		visFg: '#000000',
	},
	solarized: {
		syntax: syn(
			{},
			['#859900', ['keyword', 'selector-tag', 'doctag']],
			['#2aa198', ['string', 'regexp', 'addition']],
			['#d33682', ['number', 'literal', 'symbol', 'attr', 'attribute']],
			['#b58900', ['built_in', 'type', 'class', 'title', 'name', 'tag', 'selector-class', 'selector-id']],
			['#268bd2', ['function', 'params', 'variable', 'template-variable', 'bullet', 'link']],
			['#586e75', ['comment', 'quote', 'meta', 'deletion']],
		),
		bold: false,
		addBg: '#0b3b1f',
		delBg: '#4a1a1f',
		addMark: '#859900',
		delMark: '#dc322f',
		gutter: '#586e75',
		hunk: '#2aa198',
		mode: '#b58900',
		view: '#6c71c4',
		file: '#268bd2',
		adds: '#859900',
		dels: '#dc322f',
		dim: '#586e75',
		accent: '#cb4b16',
		modalBorder: '#268bd2',
		modalBg: '#002b36',
		modalFg: '#93a1a1',
		selBg: '#073642',
		selFg: '#93a1a1',
		curBg: '#22586b',
		visBg: '#6b4f00',
		visFg: '#fdf6e3',
	},
} satisfies Record<string, Theme>;

export type ThemeName = keyof typeof THEMES;
// default theme first, then the rest in definition order
export const THEME_NAMES = [
	DEFAULTS.theme,
	...(Object.keys(THEMES) as ThemeName[]).filter((n) => n !== DEFAULTS.theme),
] as ThemeName[];
export const isTheme = (v: unknown): v is ThemeName => typeof v === 'string' && v in THEMES;

export const ThemeContext = createContext<Theme>(THEMES[DEFAULTS.theme]);
export const useTheme = () => useContext(ThemeContext);

// cli-highlight theme: token -> ansi fn
export const highlightTheme = (t: Theme) =>
	Object.fromEntries(Object.entries(t.syntax).map(([k, c]) => [k, t.bold ? chalk.hex(c).bold : chalk.hex(c)]));
