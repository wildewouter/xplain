import {cliIntegration} from './common.js';
import type {IntegrationFactory} from './types.js';

export const createCodex: IntegrationFactory = (deps) =>
	cliIntegration(deps, {
		id: 'codex',
		label: 'Codex',
		bin: 'codex',
		pollSeconds: 45,
		addArgs: (ep) => ['mcp', 'add', 'xplain', '--url', ep.url, '--bearer-token-env-var', 'XPLAIN_MCP_TOKEN'],
		removeArgs: ['mcp', 'remove', 'xplain'],
		getArgs: ['mcp', 'get', 'xplain', '--json'],
		registeredHint:
			'Registered. Codex reads the token from env: export XPLAIN_MCP_TOKEN=<token> before starting codex, then resume (codex resume --last).',
		registerCommand: (ep) =>
			[
				`export XPLAIN_MCP_TOKEN=${ep.token}`,
				`codex mcp add xplain --url ${ep.url} --bearer-token-env-var XPLAIN_MCP_TOKEN`,
				'',
				'Codex must be started with XPLAIN_MCP_TOKEN set in its environment.',
				'Optionally add to ~/.codex/config.toml under [mcp_servers.xplain] (your consent needed):',
				'  tool_timeout_sec = 120',
				'  default_tools_approval_mode = "approve"',
				'Then restart/resume: codex resume --last',
			].join('\n'),
	});
