// group, keys, description
export const KEYS: {g: string; k: string; d: string}[] = [
	{g: 'Scroll', k: 'j/k', d: 'scroll line down / up (arrows too)'},
	{g: 'Scroll', k: 'd/u', d: 'half page down / up'},
	{g: 'Scroll', k: 'space/PgDn', d: 'page down'},
	{g: 'Scroll', k: 'PgUp', d: 'page up'},
	{g: 'Scroll', k: 'g/G', d: 'top / bottom'},
	{g: 'Scroll', k: ']/[', d: 'next / prev change'},
	{g: 'Find', k: '/', d: 'search in file / diff (Enter confirm, esc cancel)'},
	{g: 'Find', k: 'n/N', d: 'next / prev match (cursor jumps to it)'},
	{g: 'Find', k: ':', d: 'go to line (new side; nearest row if not in view)'},
	{g: 'Files', k: 'tab/S-tab', d: 'next / prev file (left/right arrows too)'},
	{g: 'Files', k: 'f', d: 'file picker'},
	{g: 'Files', k: 'F', d: 'search files by name, open in viewer'},
	{g: 'Files', k: 'esc', d: 'close file viewer'},
	{g: 'View', k: 's', d: 'toggle split / unified'},
	{g: 'View', k: 'c', d: 'toggle full file / diff'},
	{g: 'View', k: 'm', d: 'cycle mode: all, staged, unstaged'},
	{g: 'View', k: 't', d: 'cycle theme'},
	{g: 'View', k: 'r', d: 'reload from disk (also auto when agent reports changes)'},
	{g: 'Cursor', k: 'i', d: 'toggle cursor mode'},
	{g: 'Cursor', k: '0-9', d: 'count prefix (5j, 12G, 3l)'},
	{g: 'Cursor', k: 'j/k', d: 'move cursor line down / up (arrows too)'},
	{g: 'Cursor', k: 'h/l', d: 'move cursor char left / right (arrows too)'},
	{g: 'Cursor', k: 'd/u', d: 'move cursor half page'},
	{g: 'Cursor', k: 'space/PgDn', d: 'move cursor page down'},
	{g: 'Cursor', k: 'PgUp', d: 'move cursor page up'},
	{g: 'Cursor', k: 'g/G', d: 'first / last line (12G: line 12)'},
	{g: 'Cursor', k: ']/[', d: 'next / prev change'},
	{g: 'Cursor', k: '0/$', d: 'line start / end'},
	{g: 'Cursor', k: '^', d: 'first non-blank char'},
	{g: 'Cursor', k: 'w/b', d: 'word forward / back'},
	{g: 'Cursor', k: 'e', d: 'word end'},
	{g: 'Cursor', k: 'p', d: 'toggle old / new pane (split only)'},
	{g: 'Cursor', k: 'v/V', d: 'select chars / lines'},
	{g: 'Cursor', k: 'esc', d: 'end selection / exit cursor mode'},
	{g: 'Comments', k: 'Enter/a', d: 'comment on line or selection'},
	{g: 'Comments', k: 'J/K', d: 'focus next / prev comment'},
	{g: 'Comments', k: ')/(', d: 'next / prev numbered comment, all files (wraps)'},
	{g: 'Comments', k: 'j/k', d: 'scroll focused comment thread'},
	{g: 'Comments', k: 'd/u', d: 'thread half page'},
	{g: 'Comments', k: 'g/G', d: 'thread top / bottom'},
	{g: 'Comments', k: 'e/Enter', d: 'edit focused comment (not after follow-ups)'},
	{g: 'Comments', k: 'D', d: 'delete focused comment (asks y/n)'},
	{g: 'Comments', k: 'a', d: 'ask agent / follow up on focused comment'},
	{g: 'Comments', k: 'A', d: 'ask agent about all comments'},
	{g: 'Comments', k: 'esc', d: 'unfocus comment'},
	{g: 'Comments', k: 'E', d: 'export comments + threads to markdown'},
	{g: 'Editor', k: 'Enter', d: 'send comment / question'},
	{g: 'Editor', k: 'tab', d: 'toggle save only / ask agent (new comments)'},
	{g: 'Editor', k: 'left/right', d: 'move text cursor'},
	{g: 'Editor', k: 'backspace', d: 'delete char'},
	{g: 'Editor', k: 'esc', d: 'cancel'},
	{g: 'File picker', k: 'j/k', d: 'move (arrows, d/u half page)'},
	{g: 'File picker', k: 'Enter', d: 'open file'},
	{g: 'File picker', k: 'esc/f/q', d: 'close'},
	{g: 'Search', k: 'type', d: 'filter files'},
	{g: 'Search', k: 'down/up', d: 'next / prev hit (ctrl-n / ctrl-p)'},
	{g: 'Search', k: 'Enter', d: 'open hit'},
	{g: 'Search', k: 'esc', d: 'close'},
	{g: 'MCP (M)', k: 'M', d: 'open MCP / agent modal'},
	{g: 'MCP (M)', k: 'j/k', d: 'move between server and agents'},
	{g: 'MCP (M)', k: 'Enter/space', d: 'start / stop MCP server (on server row)'},
	{g: 'MCP (M)', k: 'Enter', d: 'register agent (on agent row)'},
	{g: 'MCP (M)', k: 'd', d: 'unregister agent'},
	{g: 'MCP (M)', k: 'c', d: 'copy register command'},
	{g: 'MCP (M)', k: 'w', d: 'copy watch prompt'},
	{g: 'MCP (M)', k: 'R', d: 'refresh registration status'},
	{g: 'MCP (M)', k: 'esc/q/M', d: 'close'},
	{g: 'Config (C)', k: 'C', d: 'open config'},
	{g: 'Config (C)', k: 'j/k', d: 'select setting'},
	{g: 'Config (C)', k: 'h/l', d: 'change value (arrows too)'},
	{g: 'Config (C)', k: 'Enter/space', d: 'toggle / apply setting'},
	{g: 'Config (C)', k: 'esc/q/C', d: 'close'},
	{g: 'Dialogs', k: 'y/Enter', d: 'confirm'},
	{g: 'Dialogs', k: 'n/esc', d: 'cancel (q too on quit)'},
	{g: 'General', k: '?', d: 'open / close help'},
	{g: 'General', k: 'q', d: 'quit (asks first if enabled)'},
];
// footer variants per state; full list lives in help modal
export const footer = 'j/k scroll  d/u half page  i cursor  ? help';
export type FooterState = {
	cursor?: boolean;
	visual?: boolean;
	split?: boolean;
	focused?: boolean;
	ask?: boolean;
	edit?: boolean;
};
export const footerFor = (s: FooterState) => {
	if (s.ask && s.edit) return 'enter send  esc cancel';
	if (s.ask) return 'enter send  tab save/ask  esc cancel';
	if (!s.cursor) return footer;
	if (s.focused) return 'e edit  D delete  a ask/follow up  j/k scroll  esc back  ? help';
	if (s.visual) return 'v/esc end  enter ask  hjkl move  ? help';
	return `hjkl move  v select  enter ask  J/K comments${s.split ? '  p pane' : ''}  esc exit  ? help`;
};
