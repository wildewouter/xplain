// group, keys, description
export const KEYS: {g: string; k: string; d: string}[] = [
	{g: 'Navigate', k: 'j/k', d: 'scroll line'},
	{g: 'Navigate', k: 'd/u', d: 'half page'},
	{g: 'Navigate', k: 'pgup/pgdn', d: 'page'},
	{g: 'Navigate', k: 'g/G', d: 'top / bottom'},
	{g: 'Navigate', k: ']/[', d: 'next / prev change'},
	{g: 'Files', k: 'n/p/tab', d: 'next / prev file'},
	{g: 'Files', k: 'f', d: 'file picker'},
	{g: 'Files', k: 'F', d: 'search files / browse'},
	{g: 'View', k: 's', d: 'split / unified'},
	{g: 'View', k: 'c', d: 'full file / changes only'},
	{g: 'View', k: 'm', d: 'mode all/staged/unstaged'},
	{g: 'View', k: 't', d: 'cycle theme'},
	{g: 'View', k: 'C', d: 'config'},
	{g: 'General', k: 'A', d: 'running agents'},
	{g: 'General', k: '?', d: 'help'},
	{g: 'General', k: 'q', d: 'quit'},
];
// footer: only scroll + help hint; full list lives in help modal
export const footer = 'j/k scroll  d/u half page  ? help';
