// group, keys, description, foot = short label in footer
export const KEYS: {g: string; k: string; d: string; foot?: string}[] = [
	{g: 'Navigate', k: 'j/k', d: 'scroll line', foot: 'j/k scroll'},
	{g: 'Navigate', k: 'd/u', d: 'half page', foot: 'd/u half'},
	{g: 'Navigate', k: 'pgup/pgdn', d: 'page', foot: 'pgup/pgdn page'},
	{g: 'Navigate', k: 'g/G', d: 'top / bottom', foot: 'g/G top/bot'},
	{g: 'Navigate', k: ']/[', d: 'next / prev change', foot: ']/[ change'},
	{g: 'Files', k: 'n/p/tab', d: 'next / prev file', foot: 'n/p/tab file'},
	{g: 'Files', k: 'f', d: 'file picker', foot: 'f files'},
	{g: 'Files', k: 'F', d: 'search files / browse', foot: 'F search'},
	{g: 'View', k: 's', d: 'split / unified', foot: 's split'},
	{g: 'View', k: 'c', d: 'full file / changes only', foot: 'c full/changes'},
	{g: 'View', k: 'm', d: 'mode all/staged/unstaged', foot: 'm mode'},
	{g: 'View', k: 't', d: 'cycle theme', foot: 't theme'},
	{g: 'View', k: 'C', d: 'config', foot: 'C config'},
	{g: 'General', k: '?', d: 'help', foot: '? help'},
	{g: 'General', k: 'q', d: 'quit', foot: 'q quit'},
];
export const footer = KEYS.filter((x) => x.foot)
	.map((x) => x.foot)
	.join(' ');
