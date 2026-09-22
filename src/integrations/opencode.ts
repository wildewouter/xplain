import {buildWatchPrompt} from './prompt.js';
//hahahaha
import type {IntegrationFactory} from './types.js';

export const createOpencode: IntegrationFactory = () => ({
	id: 'opencode',
	label: 'OpenCode',
	needsRestart: true,
	pollSeconds: 45,
	canRegister: false,
	registerCommand: (ep) =>
		[
			'Add to opencode.json (project) or ~/.config/opencode/opencode.json:',
			'',
			'"mcp": {',
			'  "xplain": {',
			'    "type": "remote",',
			`    "url": "${ep.url}",`,
			'    "enabled": true,',
			'    "oauth": false,',
			'    "timeout": 120000,',
			`    "headers": {"Authorization": "Bearer ${ep.token}"}`,
			'  }',
			'}',
			'',
			'Optional, to skip approval prompts:',
			'"permission": {"xplain_*": "allow"}',
			'',
			'Tool names are prefixed by the server name (xplain_next_question).',
			'Restart opencode after editing.',
		].join('\n'),
	watchPrompt: () =>
		buildWatchPrompt({pollSeconds: 45})
			.replace(/`next_question`/g, '`xplain_next_question`')
			.replace(/`answer`/g, '`xplain_answer`')
			.replace(/`files_changed`/g, '`xplain_files_changed`'),
});
