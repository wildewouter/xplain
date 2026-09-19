import {Box} from 'ink';
import {ModalText as Text} from './ModalText.js';
import {useTheme} from '../theme.js';

export function DeleteModal() {
	const t = useTheme();
	return (
		<Box borderStyle="round" borderColor={t.modalBorder} backgroundColor={t.modalBg} paddingX={1}>
			<Text>Delete comment? (y/n)</Text>
		</Box>
	);
}
