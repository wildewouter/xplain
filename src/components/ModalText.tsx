import type {ComponentProps} from 'react';
import {Text} from 'ink';
import {useTheme} from '../theme.js';

// Text for modals: defaults to the theme's modal foreground (Ink boxes can't pass a text color down).
export function ModalText({color, ...props}: ComponentProps<typeof Text>) {
	const t = useTheme();
	return <Text color={color ?? t.modalFg} {...props} />;
}
