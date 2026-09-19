export type AgentId = 'claude' | 'copilot' | 'codex' | 'opencode';
export type Agent = {
	agent: AgentId;
	pid: number;
	uptime: string;
	cwd: string;
	cmd: string;
	name?: string;
	sessionId?: string;
	status?: string;
	kind?: string;
};

// Discovery contract only. Sending questions to agents is a later block.
export interface AgentProvider {
	id: AgentId;
	label: string;
	list(): Promise<Agent[]>;
}
