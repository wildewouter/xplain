import {highlight, supportsLanguage} from 'cli-highlight';
import {highlightTheme, THEMES, type ThemeName} from './theme.js';

const langs: Record<string, string> = {
	ts: 'typescript',
	tsx: 'typescript',
	mts: 'typescript',
	cts: 'typescript',
	js: 'javascript',
	jsx: 'javascript',
	mjs: 'javascript',
	cjs: 'javascript',
	json: 'json',
	md: 'markdown',
	css: 'css',
	html: 'xml',
	xml: 'xml',
	yml: 'yaml',
	yaml: 'yaml',
	sh: 'bash',
	bash: 'bash',
	zsh: 'bash',
	py: 'python',
	go: 'go',
	rs: 'rust',
	java: 'java',
	c: 'c',
	h: 'c',
	cpp: 'cpp',
	rb: 'ruby',
	sql: 'sql',
	toml: 'ini',
};

export const langFor = (path: string) => langs[path.split('.').pop()?.toLowerCase() ?? ''];
// markdown fence info (```ts, ```python, ```shell) -> highlight language; undefined when unknown
const fenceAlias: Record<string, string> = {shell: 'bash', console: 'bash', golang: 'go', 'c++': 'cpp', yml: 'yaml'};
export const langForFence = (name?: string): string | undefined => {
	const n = name?.toLowerCase();
	if (!n) return undefined;
	const l = langs[n] ?? fenceAlias[n] ?? n;
	return supportsLanguage(l) ? l : undefined;
};

const themes = Object.fromEntries(Object.entries(THEMES).map(([k, t]) => [k, highlightTheme(t)]));
const cache = new Map<string, string>();

export const hl = (text: string, path: string, theme: ThemeName): string => hlLang(text, langFor(path), theme);
// highlight a code block line by its fence language name
export const hlFence = (text: string, fence: string | undefined, theme: ThemeName): string =>
	hlLang(text, langForFence(fence), theme);

function hlLang(text: string, lang: string | undefined, theme: ThemeName): string {
	if (!lang || !text.trim()) return text;
	const key = theme + '\0' + lang + '\0' + text;
	let r = cache.get(key);
	if (r === undefined) {
		try {
			r = highlight(text, {language: lang, ignoreIllegals: true, theme: themes[theme]});
		} catch {
			r = text;
		}
		cache.set(key, r);
	}
	return r;
}
