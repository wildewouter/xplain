import {useEffect, useRef, useSyncExternalStore} from 'react';
import {createAskController, type AskController, type AskState} from './ask/index.js';

// one store per component instance; re-renders on state snapshots
export function useAsk(): [AskController, AskState] {
	const ref = useRef<AskController>(undefined);
	ref.current ??= createAskController();
	const ctl = ref.current;
	useEffect(() => () => ctl.dispose(), [ctl]);
	const state = useSyncExternalStore(ctl.subscribe, ctl.getState);
	return [ctl, state];
}
