export type Result = {ok: boolean; message: string; needsRestart?: boolean};
export interface McpEndpoint {
	url: string;
	token: string;
	port: number;
}
export interface RunResult {
	code: number;
	stdout: string;
	stderr: string;
}
export type Runner = (
	cmd: string,
	args: string[],
	opts?: {cwd?: string; env?: Record<string, string>},
) => Promise<RunResult>;
export interface IntegrationDeps {
	run: Runner;
	cwd: string;
	home: string;
	readFile?(p: string): Promise<string | undefined>;
}
export interface AgentIntegration {
	readonly id: string;
	readonly label: string;
	readonly needsRestart: boolean;
	readonly pollSeconds: number;
	readonly canRegister: boolean;
	registerCommand(ep: McpEndpoint): string;
	watchPrompt(ep?: McpEndpoint): string;
	register?(ep: McpEndpoint): Promise<Result>;
	unregister?(): Promise<Result>;
	isRegistered?(ep: McpEndpoint): Promise<{registered: boolean; stale?: boolean}>;
}
export type IntegrationFactory = (deps: IntegrationDeps) => AgentIntegration;
