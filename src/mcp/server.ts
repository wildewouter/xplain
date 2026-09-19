import {randomUUID, createHash, timingSafeEqual} from 'node:crypto';
import {createServer, type IncomingMessage, type Server, type ServerResponse} from 'node:http';
import type {AddressInfo} from 'node:net';
import type {Hub, Question} from './hub.js';
import {callTool, TOOLS} from './tools.js';
import {readMcpConfig, writeMcpConfig} from './token.js';

export const DEFAULT_PORT = 47615;
export const SERVER_VERSION = '0.1.0';
export const PROTOCOL_VERSIONS = ['2025-06-18', '2025-03-26', '2024-11-05'] as const;
const MAX_BODY = 1024 * 1024;

export type McpServerOptions = {hub: Hub; port?: number; host?: string; token: string; dir?: string};
export type McpServer = {start(): Promise<{url: string; port: number}>; stop(): Promise<void>};

type Rpc = {jsonrpc?: string; id?: string | number | null; method?: string; params?: Record<string, unknown>};

const digest = (s: string) => createHash('sha256').update(s).digest();
const safeEqual = (a: string, b: string) => timingSafeEqual(digest(a), digest(b));

const LOOPBACK = new Set(['localhost', '127.0.0.1', '[::1]', '::1']);
const hostOk = (h: string | undefined) => {
	if (!h) return false;
	try {
		return LOOPBACK.has(new URL(`http://${h}`).hostname);
	} catch {
		return false;
	}
};
const originOk = (o: string | undefined) => {
	if (o === undefined) return true;
	try {
		return LOOPBACK.has(new URL(o).hostname);
	} catch {
		return false;
	}
};

const send = (
	res: ServerResponse,
	status: number,
	body?: unknown,
	headers: Record<string, string> = {},
	cb?: (err?: Error | null) => void,
) => {
	const data = body === undefined ? undefined : JSON.stringify(body);
	res.writeHead(status, {
		...(data !== undefined
			? {'content-type': 'application/json', 'content-length': String(Buffer.byteLength(data))}
			: {}),
		...headers,
	});
	res.end(data, () => cb?.());
};

const rpcErr = (id: Rpc['id'], code: number, message: string) => ({
	jsonrpc: '2.0',
	id: id ?? null,
	error: {code, message},
});

export function createMcpServer(opts: McpServerOptions): McpServer {
	const {hub, token, dir} = opts;
	const host = opts.host ?? '127.0.0.1';
	const wantPort = opts.port ?? DEFAULT_PORT;
	let server: Server | null = null;

	const clientIdOf = (req: IncomingMessage) => {
		const sid = req.headers['mcp-session-id'];
		return (Array.isArray(sid) ? sid[0] : sid) || `anon:${req.socket.remotePort ?? 0}`;
	};

	async function handleRpc(msg: Rpc, req: IncomingMessage, res: ServerResponse, sid: {value?: string}) {
		const id = msg.id;
		const params = msg.params ?? {};
		switch (msg.method) {
			case 'initialize': {
				const want = typeof params.protocolVersion === 'string' ? params.protocolVersion : '';
				const protocolVersion = (PROTOCOL_VERSIONS as readonly string[]).includes(want) ? want : PROTOCOL_VERSIONS[0];
				const ci = (params.clientInfo ?? {}) as {name?: unknown; version?: unknown};
				sid.value = randomUUID();
				hub.registerClient(
					sid.value,
					typeof ci.name === 'string' ? ci.name : 'unknown',
					typeof ci.version === 'string' ? ci.version : '',
				);
				return {
					jsonrpc: '2.0',
					id,
					result: {
						protocolVersion,
						capabilities: {tools: {listChanged: false}},
						serverInfo: {name: 'xplain', version: SERVER_VERSION},
					},
				};
			}
			case 'ping':
				return {jsonrpc: '2.0', id, result: {}};
			case 'tools/list':
				return {jsonrpc: '2.0', id, result: {tools: TOOLS}};
			case 'tools/call': {
				const name = typeof params.name === 'string' ? params.name : '';
				if (!TOOLS.some((t) => t.name === name)) return rpcErr(id, -32602, `Unknown tool: ${name.slice(0, 100)}`);
				const ac = new AbortController();
				let gone = false;
				let finished = false;
				const onClose = () => {
					if (!finished) {
						gone = true;
						ac.abort();
					}
				};
				res.on('close', onClose);
				let delivered: Question | undefined;
				const result = await callTool(name, params.arguments, {
					hub,
					clientId: sid.value ?? clientIdOf(req),
					signal: ac.signal,
					onDeliver: (q) => (delivered = q),
				});
				res.off('close', onClose);
				if (delivered) {
					if (gone || res.destroyed || req.socket.destroyed) {
						hub.requeue(delivered);
						return null;
					}
					// Requeue also if the write itself fails.
					const q = delivered;
					res.once('close', () => {
						if (!finished) hub.requeue(q);
					});
					res.once('finish', () => (finished = true));
				}
				return {jsonrpc: '2.0', id, result};
			}
			default:
				return rpcErr(id, -32601, `Method not found: ${String(msg.method).slice(0, 100)}`);
		}
	}

	async function onRequest(req: IncomingMessage, res: ServerResponse) {
		try {
			const path = (req.url ?? '').split('?')[0];
			if (!hostOk(req.headers.host) || !originOk(req.headers.origin)) return send(res, 403, {error: 'forbidden'});
			if (path !== '/mcp') return send(res, 404, {error: 'not found'});
			const auth = req.headers.authorization ?? '';
			const m = /^Bearer (.+)$/.exec(auth);
			if (!m || !safeEqual(m[1]!, token))
				return send(res, 401, {error: 'unauthorized'}, {'www-authenticate': 'Bearer'});
			if (req.method !== 'POST') return send(res, 405, {error: 'method not allowed'}, {allow: 'POST'});

			const chunks: Buffer[] = [];
			let size = 0;
			for await (const c of req) {
				size += (c as Buffer).length;
				if (size > MAX_BODY) return send(res, 413, {error: 'body too large'}, {connection: 'close'});
				chunks.push(c as Buffer);
			}
			let body: unknown;
			try {
				body = JSON.parse(Buffer.concat(chunks).toString('utf8'));
			} catch {
				return send(res, 400, rpcErr(null, -32700, 'Parse error'));
			}
			const sid: {value?: string} = {};
			const batch = Array.isArray(body);
			const msgs = (batch ? body : [body]) as Rpc[];
			const out: unknown[] = [];
			for (const msg of msgs) {
				if (!msg || typeof msg !== 'object' || typeof msg.method !== 'string') {
					out.push(rpcErr((msg as Rpc | null)?.id, -32600, 'Invalid Request'));
					continue;
				}
				if (msg.id === undefined) continue; // notification: no response
				const r = await handleRpc(msg, req, res, sid);
				if (r === null) return; // connection gone
				out.push(r);
			}
			const headers: Record<string, string> = sid.value ? {'mcp-session-id': sid.value} : {};
			if (out.length === 0) return send(res, 202, undefined, headers);
			send(res, 200, batch ? out : out[0], headers);
		} catch {
			if (!res.headersSent) send(res, 500, rpcErr(null, -32603, 'Internal error'));
			else res.destroy();
		}
	}

	return {
		start() {
			return new Promise((resolve, reject) => {
				const s = createServer((req, res) => void onRequest(req, res));
				s.requestTimeout = 0;
				s.timeout = 0;
				s.keepAliveTimeout = 5000;
				s.once('error', (e: NodeJS.ErrnoException) => {
					reject(
						e.code === 'EADDRINUSE'
							? new Error(
									`MCP port ${wantPort} is already in use on ${host}. Stop the other process or choose another port.`,
								)
							: e,
					);
				});
				s.listen(wantPort, host, () => {
					server = s;
					const port = (s.address() as AddressInfo).port;
					if (dir) {
						try {
							const cur = readMcpConfig(dir);
							if (cur && cur.port !== port) writeMcpConfig(dir, {...cur, port});
						} catch {
							// non-fatal
						}
					}
					resolve({url: `http://${host.includes(':') ? `[${host}]` : host}:${port}/mcp`, port});
				});
			});
		},
		stop() {
			const s = server;
			server = null;
			if (!s) return Promise.resolve();
			return new Promise<void>((resolve) => {
				s.close(() => resolve());
				s.closeAllConnections();
			});
		},
	};
}
