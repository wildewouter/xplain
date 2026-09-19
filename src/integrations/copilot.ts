import {cliIntegration} from './common.js';
import type {IntegrationFactory} from './types.js';

export const createCopilot: IntegrationFactory = (deps) =>
	cliIntegration(deps, {
		id: 'copilot',
		label: 'Copilot',
		bin: 'copilot',
		pollSeconds: 120,
		addArgs: (ep) => [
			'mcp',
			'add',
			'xplain',
			ep.url,
			'--transport',
			'http',
			'--header',
			`Authorization: Bearer ${ep.token}`,
			'--timeout',
			'200000',
		],
		removeArgs: ['mcp', 'remove', 'xplain'],
		getArgs: ['mcp', 'get', 'xplain', '--json'],
		registeredHint: 'Registered. Restart copilot (or use /mcp), then paste the watch prompt.',
		registerCommand: (ep) =>
			[
				`copilot mcp add xplain ${ep.url} --transport http --header "Authorization: Bearer ${ep.token}" --timeout 200000`,
				'',
				'Then restart copilot (or use /mcp).',
				"Allow the tools: copilot --allow-tool='xplain'  (or approve once when asked).",
				"Session-only alternative: copilot --additional-mcp-config @file --allow-tool='xplain'",
			].join('\n'),
	});
