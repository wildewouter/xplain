import {randomBytes} from 'node:crypto';
import {chmodSync, mkdirSync, readFileSync, writeFileSync} from 'node:fs';
import {homedir} from 'node:os';
import {join} from 'node:path';

export type McpConfig = {token: string; port?: number};

export function defaultStateDir(): string {
	const xdg = process.env.XDG_STATE_HOME;
	return join(xdg && xdg.length > 0 ? xdg : join(homedir(), '.local', 'state'), 'xplain');
}

const file = (dir: string) => join(dir, 'mcp.json');
const newToken = () => randomBytes(32).toString('base64url');

export function readMcpConfig(dir: string = defaultStateDir()): McpConfig | null {
	try {
		const j = JSON.parse(readFileSync(file(dir), 'utf8')) as Partial<McpConfig>;
		if (typeof j.token === 'string' && j.token.length >= 16) {
			return {token: j.token, ...(typeof j.port === 'number' ? {port: j.port} : {})};
		}
	} catch {
		// missing or corrupt: caller recreates
	}
	return null;
}

export function writeMcpConfig(dir: string, cfg: McpConfig): void {
	mkdirSync(dir, {recursive: true, mode: 0o700});
	writeFileSync(file(dir), JSON.stringify(cfg, null, 2) + '\n', {mode: 0o600});
	chmodSync(file(dir), 0o600);
}

/** Returns the stored config, creating a fresh token if absent. */
export function loadOrCreateToken(dir: string = defaultStateDir()): McpConfig {
	const cur = readMcpConfig(dir);
	if (cur) return cur;
	const cfg: McpConfig = {token: newToken()};
	writeMcpConfig(dir, cfg);
	return cfg;
}

export function rotateToken(dir: string = defaultStateDir()): McpConfig {
	const cur = readMcpConfig(dir);
	const cfg: McpConfig = {token: newToken(), ...(cur?.port !== undefined ? {port: cur.port} : {})};
	writeMcpConfig(dir, cfg);
	return cfg;
}
