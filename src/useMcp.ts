import {useEffect, useRef, useSyncExternalStore} from 'react';
import type {AskController} from './ask/index.js';
import {createMcpBridge, type McpBridge, type BridgeState} from './mcp/bridge.js';

// lazily created (not started) bridge unless one is injected; re-renders on state snapshots
export function useMcp(
	ctl: AskController,
	injected?: McpBridge | ((ctl: AskController) => McpBridge),
	cwd?: string,
): [McpBridge, BridgeState] {
	const ref = useRef<McpBridge>(undefined);
	ref.current ??=
		(typeof injected === 'function' ? injected(ctl) : injected) ?? createMcpBridge({controller: ctl, cwd});
	const bridge = ref.current;
	useEffect(() => (injected ? undefined : () => bridge.dispose()), [bridge, injected]);
	const state = useSyncExternalStore(bridge.subscribe, bridge.getState);
	return [bridge, state];
}
