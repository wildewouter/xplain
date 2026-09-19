// group, keys, description
export const KEYS: {g: string; k: string; d: string}[] = [
	{g: 'Navigate', k: 'j/k d/u', d: 'scroll line / half page'},
	{g: 'Navigate', k: 'g/G ]/[', d: 'top / bottom, next / prev change'},
	{g: 'Navigate', k: 'i', d: 'cursor mode (count: 5j, 12G, 3l)'},
	{g: 'Navigate', k: 'Enter/a', d: 'ask about line or selection (cursor)'},
	{g: 'Navigate', k: 'h/l 0^$ p', d: 'char left/right, line start/first/end, p: toggle split pane'},
	{g: 'Navigate', k: 'w/b/e', d: 'word forward / back / end'},
	{g: 'Navigate', k: 'J/K e D', d: 'focus next / prev comment, edit, delete'},
	{g: 'Navigate', k: 'v/V', d: 'select chars / lines (esc ends)'},
	{g: 'Files', k: 'tab/S-tab', d: 'next / prev file (or left/right)'},
	{g: 'Files', k: 'f', d: 'file picker'},
	{g: 'Files', k: 'F', d: 'search files / browse'},
	{g: 'View', k: 's/c/m', d: 'split / full file / mode all,staged,unstaged'},
	{g: 'View', k: 't', d: 'cycle theme'},
	{g: 'View', k: 'C', d: 'config'},
	{g: 'View', k: 'A', d: 'running agents'},
	{g: 'View', k: '?/q', d: 'help / quit'},
];
// footer variants per state; full list lives in help modal
export const footer = 'j/k scroll  d/u half page  i cursor  ? help';
export type FooterState = {cursor?: boolean; visual?: boolean; split?: boolean; focused?: boolean; ask?: boolean};
export const footerFor = (s: FooterState) => {
	if (s.ask) return 'enter send  esc cancel';
	if (!s.cursor) return footer;
	if (s.focused) return 'e edit  D delete  J/K prev/next  esc back  ? help';
	if (s.visual) return 'v/esc end  enter ask  hjkl move  ? help';
	return `hjkl move  v select  enter ask  J/K comments${s.split ? '  p pane' : ''}  esc exit  ? help`;
};
