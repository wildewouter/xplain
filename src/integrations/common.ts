import type {AgentIntegration, IntegrationDeps, McpEndpoint, Result, RunResult} from './types.js';
import {buildWatchPrompt} from './prompt.js';

export const mask = (s: string, token: string): string => (token ? s.split(token).join('***') : s);

export type Ran = {res: RunResult} | {err: 'enoent' | 'error'; text: string};

export async function safeRun(deps: IntegrationDeps, cmd: string, args: string[], token: string): Promise<Ran> {
	try {
		return {res: await deps.run(cmd, args, {cwd: deps.cwd})};
	} catch (e) {
		const code = (e as {code?: string})?.code;
		const text = mask(e instanceof Error ? e.message : String(e), token);
		return {err: code === 'ENOENT' || /ENOENT/.test(text) ? 'enoent' : 'error', text};
	}
}

/** Extract a url from `get` output: JSON (top-level, or nested under `xplain`) or first http(s) URL in text. */
export function parseUrl(stdout: string): string | undefined {
	try {
		const j = JSON.parse(stdout) as Record<string, unknown>;
		const pick = (o: unknown): string | undefined => {
			const u = (o as {url?: unknown} | null)?.url;
			return typeof u === 'string' ? u : undefined;
		};
		const u = pick(j) ?? pick(j?.xplain) ?? pick((j?.mcpServers as Record<string, unknown> | undefined)?.xplain);
		if (u) return u;
	} catch {
		/* not json */
	}
	return stdout.match(/https?:\/\/[^\s"',]+/)?.[0];
}

export interface CliSpec {
	id: string;
	label: string;
	bin: string;
	pollSeconds: number;
	addArgs(ep: McpEndpoint): string[];
	removeArgs: string[];
	getArgs: string[];
	registerCommand(ep: McpEndpoint): string;
	registeredHint: string;
}

export function cliIntegration(deps: IntegrationDeps, s: CliSpec): AgentIntegration {
	let lastToken = '';
	const fail = (r: Ran, token: string, what: string): Result => {
		if ('err' in r) {
			return {
				ok: false,
				message: r.err === 'enoent' ? `${s.label} CLI not found` : `${s.label} ${what} failed: ${r.text}`,
			};
		}
		const out = mask((r.res.stderr || r.res.stdout).trim().split('\n').slice(0, 3).join(' '), token);
		return {ok: false, message: `${s.label} ${what} failed (exit ${r.res.code})${out ? ': ' + out : ''}`};
	};
	return {
		id: s.id,
		label: s.label,
		needsRestart: true,
		pollSeconds: s.pollSeconds,
		canRegister: true,
		registerCommand: s.registerCommand,
		watchPrompt: () => buildWatchPrompt({pollSeconds: s.pollSeconds}),
		async register(ep) {
			lastToken = ep.token;
			await safeRun(deps, s.bin, s.removeArgs, ep.token); // ignore failure
			const r = await safeRun(deps, s.bin, s.addArgs(ep), ep.token);
			if ('err' in r || r.res.code !== 0) return fail(r, ep.token, 'register');
			return {ok: true, message: s.registeredHint, needsRestart: true};
		},
		async unregister() {
			const r = await safeRun(deps, s.bin, s.removeArgs, lastToken);
			if ('err' in r || r.res.code !== 0) return fail(r, lastToken, 'unregister');
			return {ok: true, message: `Removed xplain from ${s.label}`, needsRestart: true};
		},
		async isRegistered(ep) {
			lastToken = ep.token;
			const r = await safeRun(deps, s.bin, s.getArgs, ep.token);
			if ('err' in r || r.res.code !== 0) return {registered: false};
			const url = parseUrl(r.res.stdout);
			return url && url !== ep.url ? {registered: true, stale: true} : {registered: true, stale: false};
		},
	};
}
