import {createClaude} from './claude.js';
import {createCodex} from './codex.js';
import {createCopilot} from './copilot.js';
import {createOpencode} from './opencode.js';
import type {AgentIntegration, IntegrationDeps} from './types.js';

export type * from './types.js';
export {buildWatchPrompt} from './prompt.js';

export function createIntegrations(deps: IntegrationDeps): AgentIntegration[] {
	return [createClaude(deps), createCodex(deps), createOpencode(deps), createCopilot(deps)];
}

export function byId(list: AgentIntegration[], id: string): AgentIntegration | undefined {
	return list.find((i) => i.id === id);
}
