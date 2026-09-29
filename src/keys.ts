export type HelpCtx =
	| 'cursor'
	| 'visual'
	| 'comment'
	| 'editor'
	| 'find'
	| 'goto'
	| 'picker'
	| 'search'
	| 'mcp'
	| 'config'
	| 'dialog'
	| 'browse';
// group, keys, description, contexts where the key applies.
// Keys that work the same everywhere are listed only in 'cursor'; other contexts list what differs.
// vim: standard vim motion, hidden from help unless motions are shown
// bar: footer item documenting this key; when the context's footer always shows it, help drops the entry
// or, with rest, shows only the part the footer leaves out
export type Key = {
	g: string;
	k: string;
	d: string;
	c: HelpCtx[];
	vim?: boolean;
	bar?: BarId;
	rest?: {k: string; d: string};
};
export const KEYS: Key[] = [
	{g: 'Move', k: 'h/j/k/l', d: 'char / line', c: ['cursor', 'browse'], vim: true, bar: 'move'},
	{g: 'Move', k: 'w/b/e', d: 'word fwd/back/end', c: ['cursor', 'browse'], vim: true},
	{g: 'Move', k: '0/^/$', d: 'start/nonblank/end', c: ['cursor', 'browse'], vim: true},
	{g: 'Move', k: 'd/u', d: 'half page down/up', c: ['cursor', 'browse'], vim: true},
	{g: 'Move', k: 'PgDn/PgUp', d: 'page down/up (space: down)', c: ['cursor', 'browse'], vim: true},
	{g: 'Move', k: 'g/G', d: 'first / last line', c: ['cursor', 'browse'], vim: true},
	{g: 'Move', k: '1-9', d: 'count (5j, 12G)', c: ['cursor', 'browse'], vim: true},
	{g: 'Find', k: ']/[', d: 'next/prev change', c: ['cursor']},
	{g: 'Find', k: '/ n/N', d: 'find, next/prev', c: ['cursor']},
	{g: 'Find', k: ':', d: 'go to line', c: ['cursor']},
	{g: 'Find', k: 'type', d: 'search text', c: ['find']},
	{g: 'Find', k: 'backspace', d: 'delete char', c: ['find']},
	{g: 'Find', k: 'Enter', d: 'jump to match', c: ['find']},
	{g: 'Find', k: 'esc', d: 'cancel', c: ['find']},
	{g: 'Go to line', k: 'type', d: 'line number', c: ['goto']},
	{g: 'Go to line', k: 'backspace', d: 'delete char', c: ['goto']},
	{g: 'Go to line', k: 'Enter', d: 'go to line', c: ['goto']},
	{g: 'Go to line', k: 'esc', d: 'cancel', c: ['goto']},
	{g: 'Find', k: 'tab/S-tab', d: 'next / prev file', c: ['cursor']},
	{g: 'Find', k: 'f/F', d: 'file picker / search', c: ['cursor']},
	{g: 'File viewer', k: 'esc', d: 'back to diff', c: ['browse']},
	{g: 'File viewer', k: 's/c/m/f', d: 'diff-only, no-op', c: ['browse']},
	{g: 'Comments', k: 'v/V', d: 'select chars/lines', c: ['cursor', 'browse']},
	{
		g: 'Comments',
		k: 'Enter/a',
		d: 'comment on line',
		c: ['cursor', 'browse'],
		bar: 'ask',
		rest: {k: 'a', d: 'comment on line'},
	},
	{g: 'General', k: 's/c/m', d: 'split, full, staged', c: ['cursor']},
	{g: 'General', k: 't/r', d: 'theme / reload', c: ['cursor']},
	{g: 'General', k: 'p', d: 'old/new pane', c: ['cursor'], bar: 'pane'},
	{g: 'Move', k: 'hjkl…', d: 'extend selection', c: ['visual'], vim: true, bar: 'move'},
	{g: 'Selection', k: 'v/V', d: 'chars/lines, end', c: ['visual'], bar: 'end', rest: {k: 'V', d: 'lines, end'}},
	{g: 'Selection', k: 'Enter/a', d: 'comment on it', c: ['visual'], bar: 'ask', rest: {k: 'a', d: 'comment on it'}},
	{g: 'Selection', k: 'esc', d: 'end selection', c: ['visual'], bar: 'end'},
	{
		g: 'Comments',
		k: 'J/K',
		d: 'next/prev in file',
		c: ['cursor', 'browse', 'visual', 'comment'],
		bar: 'comments',
	},
	{g: 'Comments', k: ')/(', d: 'numbered, any file', c: ['cursor', 'browse', 'visual', 'comment']},
	{g: 'Comments', k: 'E', d: 'export comments', c: ['cursor']},
	{
		g: 'Move',
		k: 'j/k d/u',
		d: 'scroll thread',
		c: ['comment'],
		vim: true,
		bar: 'scroll',
		rest: {k: 'd/u', d: 'scroll thread'},
	},
	{g: 'Move', k: 'g/G', d: 'thread top/bottom', c: ['comment'], vim: true},
	{
		g: 'Comments',
		k: 'e/Enter',
		d: 'edit (no replies)',
		c: ['comment'],
		bar: 'edit',
		rest: {k: 'Enter', d: 'edit (no replies)'},
	},
	{g: 'Comments', k: 'D', d: 'delete (y/n)', c: ['comment'], bar: 'delete'},
	{g: 'Comments', k: 'a/A', d: 'ask: this / all', c: ['comment'], bar: 'follow', rest: {k: 'A', d: 'ask: all'}},
	{g: 'Comments', k: 'up/down', d: 'pick block to copy', c: ['comment']},
	{g: 'Comments', k: 'hjkl…', d: 'motion unfocuses', c: ['comment']},
	{g: 'Comments', k: 'esc', d: 'unpick / unfocus', c: ['comment'], bar: 'back'},
	{g: 'Editor', k: 'type', d: 'comment text', c: ['editor']},
	{g: 'Editor', k: 'Enter', d: 'send', c: ['editor'], bar: 'send'},
	{g: 'Editor', k: 'tab', d: 'save / ask agent', c: ['editor'], bar: 'save'},
	{g: 'Editor', k: 'left/right', d: 'move cursor', c: ['editor']},
	{g: 'Editor', k: 'backspace', d: 'delete char', c: ['editor']},
	{g: 'Editor', k: 'esc', d: 'cancel', c: ['editor'], bar: 'cancel'},
	{g: 'Move', k: 'j/k', d: 'move', c: ['picker'], vim: true},
	{g: 'Move', k: 'd/u', d: 'half page down/up', c: ['picker'], vim: true},
	{g: 'File picker', k: 'Enter', d: 'open file', c: ['picker']},
	{g: 'File picker', k: 'esc/f/q', d: 'close', c: ['picker']},
	{g: 'Search', k: 'type', d: 'filter files', c: ['search']},
	{g: 'Search', k: 'backspace', d: 'delete char', c: ['search']},
	{g: 'Search', k: 'down/up', d: 'next / prev hit', c: ['search']},
	{g: 'Search', k: 'Enter', d: 'open hit', c: ['search']},
	{g: 'Search', k: 'esc', d: 'close', c: ['search']},
	{g: 'Move', k: 'j/k', d: 'move', c: ['mcp'], vim: true},
	{g: 'MCP (M)', k: '⏎/space', d: 'server on / off', c: ['mcp']},
	{g: 'MCP (M)', k: 'Enter', d: 'register agent', c: ['mcp']},
	{g: 'MCP (M)', k: 'd', d: 'unregister agent', c: ['mcp']},
	{g: 'MCP (M)', k: 'y/n', d: 'confirm / cancel', c: ['mcp']},
	{g: 'MCP (M)', k: 'c/w', d: 'copy cmd / prompt', c: ['mcp']},
	{g: 'MCP (M)', k: 'R', d: 'refresh status', c: ['mcp']},
	{g: 'MCP (M)', k: 'esc/q/M', d: 'close', c: ['mcp']},
	{g: 'Move', k: 'j/k', d: 'select setting', c: ['config'], vim: true},
	{g: 'Config (C)', k: 'h/l', d: 'change value', c: ['config']},
	{g: 'Config (C)', k: '⏎/space', d: 'toggle / apply', c: ['config']},
	{g: 'Config (C)', k: 'esc/q/C', d: 'close', c: ['config']},
	{g: 'Dialogs', k: 'y/Enter', d: 'confirm', c: ['dialog']},
	{g: 'Dialogs', k: 'n/esc', d: 'cancel (q: quit)', c: ['dialog']},
	{g: 'General', k: 'M/C', d: 'MCP / config', c: ['cursor']},
	{g: 'General', k: '?', d: 'help/more/close', c: ['cursor'], bar: 'help'},
	{g: 'General', k: 'q', d: 'quit', c: ['cursor']},
];
export type HelpState = {
	ask?: boolean;
	find?: boolean;
	goto?: boolean;
	dialog?: boolean;
	search?: boolean;
	mcp?: boolean;
	config?: boolean;
	picker?: boolean;
	browse?: boolean;
	visual?: boolean;
	focused?: boolean;
};
// priority mirrors the useInput handler order in app.tsx
export const helpCtx = (s: HelpState): HelpCtx => {
	if (s.ask) return 'editor';
	if (s.find) return 'find';
	if (s.goto) return 'goto';
	if (s.dialog) return 'dialog';
	if (s.search) return 'search';
	if (s.mcp) return 'mcp';
	if (s.config) return 'config';
	if (s.picker) return 'picker';
	if (s.focused) return 'comment';
	if (s.visual) return 'visual';
	if (s.browse) return 'browse';
	return 'cursor';
};
export const CTX_LABEL: Record<HelpCtx, string> = {
	cursor: 'Diff view',
	visual: 'Visual selection',
	comment: 'Focused comment',
	editor: 'Editor',
	find: 'Find in file',
	goto: 'Go to line',
	picker: 'File picker',
	search: 'File search',
	mcp: 'MCP',
	config: 'Config',
	dialog: 'Confirm',
	browse: 'File viewer',
};
// footer items, rendered as `k d`
const BAR = {
	scroll: 'j/k scroll',
	help: '? help',
	move: 'hjkl move',
	ask: 'enter ask',
	comments: 'J/K comments',
	pane: 'p pane',
	end: 'v/esc end',
	edit: 'e edit',
	delete: 'D delete',
	follow: 'a ask/follow up',
	back: 'esc back',
	send: 'enter send',
	save: 'tab save/ask',
	cancel: 'esc cancel',
} as const;
export type BarId = keyof typeof BAR;
type Bar = 'cursor' | 'split' | 'visual' | 'comment' | 'ask' | 'edit';
const BARS: Record<Bar, BarId[]> = {
	cursor: ['move', 'ask', 'comments', 'help'],
	split: ['move', 'ask', 'comments', 'pane', 'help'],
	visual: ['end', 'ask', 'move', 'help'],
	comment: ['edit', 'delete', 'follow', 'scroll', 'back', 'help'],
	ask: ['send', 'save', 'cancel'],
	edit: ['send', 'cancel'],
};
// footers that can show in each help context; modals and text inputs keep the view's footer, which doesn't describe their keys
const CTX_BARS: Partial<Record<HelpCtx, Bar[]>> = {
	cursor: ['cursor', 'split'],
	browse: ['cursor'],
	visual: ['visual'],
	comment: ['comment'],
	editor: ['ask', 'edit'],
};
const inBar = (c: HelpCtx, id?: BarId) => !!id && !!CTX_BARS[c]?.every((b) => BARS[b].includes(id));
export const keysFor = (c: HelpCtx, motions = false): Key[] =>
	KEYS.filter((k) => k.c.includes(c) && (motions || !k.vim)).flatMap((k) =>
		!inBar(c, k.bar) ? [k] : k.rest ? [{...k, ...k.rest}] : [],
	);
export const hasMotions = (c: HelpCtx) => keysFor(c, true).some((k) => k.vim);
export type FooterState = {
	visual?: boolean;
	split?: boolean;
	focused?: boolean;
	ask?: boolean;
	edit?: boolean;
};
const barFor = (s: FooterState): Bar => {
	if (s.ask && s.edit) return 'edit';
	if (s.ask) return 'ask';
	if (s.focused) return 'comment';
	if (s.visual) return 'visual';
	return s.split ? 'split' : 'cursor';
};
// footer variants per state; full list lives in help modal
export const footerFor = (s: FooterState) => BARS[barFor(s)].map((id) => BAR[id]).join('  ');
export const footer = footerFor({});
