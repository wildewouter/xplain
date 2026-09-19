import {cliIntegration} from './common.js';
import type {IntegrationFactory} from './types.js';

export const createClaude: IntegrationFactory = (deps) =>
	cliIntegration(deps, {
		id: 'claude',
		label: 'Claude Code',
		bin: 'claude',
		pollSeconds: 100,
		addArgs: (ep) => [
			'mcp',
			'add',
			'xplain',
			ep.url,
			'--transport',
			'http',
			'--scope',
			'local',
			'--header',
			`Authorization: Bearer ${ep.token}`,
		],
		removeArgs: ['mcp', 'remove', 'xplain', '-s', 'local'],
		getArgs: ['mcp', 'get', 'xplain'],
		registeredHint: 'Registered. Restart or resume the session (claude --resume), then paste the watch prompt.',
		registerCommand: (ep) =>
			[
				`claude mcp add xplain ${ep.url} --transport http --scope local --header "Authorization: Bearer ${ep.token}"`,
				'',
				'Then restart/resume the session: claude --resume',
				'Allow the tools: claude --allowedTools "mcp__xplain"  (or add permission rule mcp__xplain)',
				'Optional: set env CLAUDE_CODE_MCP_AUTO_BACKGROUND_MS=0 to avoid auto-backgrounding long calls.',
			].join('\n'),
	});
