import {claudeClient} from './claude.js';
import type {ClientFactory} from './types.js';
export * from './types.js';

// Planned native routes (not built yet, createClient returns undefined):
//  - opencode: `opencode serve` HTTP + SSE
//  - codex: `codex exec --json` / app-server
//  - copilot: ACP or `-p` with json output
export const createClient: ClientFactory = (id) => (id === 'claude' ? claudeClient() : undefined);
