import {claudeProvider} from './claude.js';
import {codexProvider} from './codex.js';
import {copilotProvider} from './copilot.js';
import {opencodeProvider} from './opencode.js';
import type {Agent, AgentProvider} from './types.js';

export type {Agent, AgentId, AgentProvider} from './types.js';

// Adding an agent = new file + one entry here.
export const PROVIDERS: AgentProvider[] = [claudeProvider(), copilotProvider(), codexProvider(), opencodeProvider()];

export async function listAgents(providers: AgentProvider[] = PROVIDERS): Promise<Agent[]> {
	const rs = await Promise.allSettled(providers.map((p) => p.list()));
	return rs
		.flatMap((r) => (r.status === 'fulfilled' ? r.value : []))
		.sort((a, b) => a.agent.localeCompare(b.agent) || a.pid - b.pid);
}
