import {highlight} from 'cli-highlight';

const langs: Record<string, string> = {
	ts: 'typescript', tsx: 'typescript', mts: 'typescript', cts: 'typescript',
	js: 'javascript', jsx: 'javascript', mjs: 'javascript', cjs: 'javascript',
	json: 'json', md: 'markdown', css: 'css', html: 'xml', xml: 'xml', yml: 'yaml', yaml: 'yaml',
	sh: 'bash', bash: 'bash', zsh: 'bash', py: 'python', go: 'go', rs: 'rust', java: 'java',
	c: 'c', h: 'c', cpp: 'cpp', rb: 'ruby', sql: 'sql', toml: 'ini',
};

export const langFor = (path: string) => langs[path.split('.').pop()?.toLowerCase() ?? ''];

const cache = new Map<string, string>();

export function hl(text: string, path: string): string {
	const lang = langFor(path);
	if (!lang || !text.trim()) return text;
	const key = lang + '\0' + text;
	let r = cache.get(key);
	if (r === undefined) {
		try {
			r = highlight(text, {language: lang, ignoreIllegals: true});
		} catch {
			r = text;
		}
		cache.set(key, r);
	}
	return r;
}
