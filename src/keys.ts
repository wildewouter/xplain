// group, keys, description
export const KEYS: {g: string; k: string; d: string}[] = [
	{g: 'Navigate', k: 'j/k d/u', d: 'scroll line / half page'},
	{g: 'Navigate', k: 'g/G ]/[', d: 'top / bottom, next / prev change'},
	{g: 'Navigate', k: 'i Enter/a', d: 'cursor mode (count: 5j, 12G, 3l); comment on line or selection'},
	{g: 'Navigate', k: 'h/l 0^$ p', d: 'char left/right, line start/first/end, p: toggle split pane'},
	{g: 'Navigate', k: 'w/b/e', d: 'word forward / back / end'},
	{g: 'Navigate', k: 'J/K e D', d: 'focus comment, edit, delete'},
	{g: 'Navigate', k: 'a A', d: 'a ask / follow up, A ask all comments'},
	{g: 'Navigate', k: 'focused', d: 'j/k d/u g/G scroll the comment thread'},
	{g: 'Navigate', k: 'v/V', d: 'select chars / lines (esc ends)'},
	{g: 'Files', k: 'tab/S-tab', d: 'next / prev file (or left/right)'},
	{g: 'Files', k: 'f/F', d: 'file picker / search files, browse'},
	{g: 'View', k: 's/c/m', d: 'split / full file / mode all,staged,unstaged'},
	{g: 'View', k: 't/C/M', d: 'cycle theme / config / MCP (agents answer)'},
	{g: 'View', k: 'r', d: 'reload from disk (also when agent reports changes)'},
	{g: 'View', k: 'E', d: 'export comments + threads to markdown'},
	{g: 'View', k: '?/q', d: 'help / quit'},
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
