// Dependency-free async test helpers (polling instead of fixed sleeps).
export const settle = (ms = 15): Promise<void> => new Promise((r) => setTimeout(r, ms));

/** Poll pred until true or timeout. Never throws; returns whether pred became true. */
export async function waitFor(
	pred: () => boolean | Promise<boolean>,
	{timeout = 1000, interval = 5}: {timeout?: number; interval?: number} = {},
): Promise<boolean> {
	const end = Date.now() + timeout;
	for (;;) {
		try {
			if (await pred()) return true;
		} catch {
			// treat throw as not-yet
		}
		if (Date.now() >= end) return false;
		await settle(interval);
	}
}

export type Stdin = {write: (s: string) => unknown};

/** Write key, then wait for pred (or a short settle when no pred). Returns pred result (true w/o pred). */
export async function keyAndWait(
	stdin: Stdin,
	key: string,
	pred?: () => boolean,
	opts?: {timeout?: number; interval?: number; settleMs?: number},
): Promise<boolean> {
	stdin.write(key);
	if (!pred) {
		await settle(opts?.settleMs);
		return true;
	}
	return waitFor(pred, opts);
}

/**
 * press(key, until?): until is a predicate on the last frame, or a string/RegExp the frame must contain/match.
 * Without `until`, waits for the frame to change (max ~200ms) then a short settle.
 */
export function makePress(stdin: Stdin, lastFrame: () => string | undefined, opts: {changeTimeout?: number} = {}) {
	return async function press(key: string, until?: string | RegExp | ((frame: string) => boolean)): Promise<boolean> {
		const before = lastFrame() ?? '';
		stdin.write(key);
		if (until === undefined) {
			const changed = await waitFor(() => (lastFrame() ?? '') !== before, {timeout: opts.changeTimeout ?? 200});
			await settle(changed ? 10 : 15);
			return changed;
		}
		const p =
			typeof until === 'function'
				? until
				: typeof until === 'string'
					? (f: string) => f.includes(until)
					: (f: string) => until.test(f);
		return waitFor(() => p(lastFrame() ?? ''));
	};
}
