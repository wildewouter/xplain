// Copy via OSC 52 terminal escape (works over ssh, no external tools).
export function osc52Copy(text: string, out: {write(s: string): unknown} = process.stdout): void {
	out.write(`\x1b]52;c;${Buffer.from(text, 'utf8').toString('base64')}\x07`);
}
