import {useEffect, useState} from 'react';
import {ModalText as Text} from './components/ModalText.js';

export const SPIN_FRAMES = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
export const SPIN_MS = 80;

export function Spinner({color}: {color?: string}) {
	const [i, setI] = useState(0);
	useEffect(() => {
		const id = setInterval(() => setI((n) => (n + 1) % SPIN_FRAMES.length), SPIN_MS);
		return () => clearInterval(id);
	}, []);
	return <Text color={color}>{SPIN_FRAMES[i]}</Text>;
}
