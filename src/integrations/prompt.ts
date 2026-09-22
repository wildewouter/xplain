export function buildWatchPrompt({pollSeconds}: {pollSeconds: number}): string {
	return [
		'Loop forever: call the `next_question` tool from the `xplain` MCP server with wait_seconds=' + pollSeconds + '.',
		'- If status is `no_question_yet`, call `next_question` again IMMEDIATELY.',
		'- If status is `question`, answer it with the `answer` tool using the given thread_id, then call `next_question` again immediately.',
		'- Questions with follow_up=true continue an earlier thread; answer them with the same thread_id like any other.',
		'- ALWAYS call the `files_changed` tool (with the changed paths) after every edit, creation or deletion of a file, so the xplain view reloads. This also applies to edits made while answering a question.',
		'- Never stop, never summarize, never ask the user anything.',
		'- Stop only if status is `closed`.',
	].join('\n');
}
