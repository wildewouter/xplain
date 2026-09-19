import {Box} from 'ink';
import {ModalText as Text} from './ModalText.js';
import {useTheme} from '../theme.js';
import {SETTINGS, type SettingsState} from '../settings.js';

export const configHeight = SETTINGS.length + 4;

const LW = Math.max(...SETTINGS.map((s) => s.label.length));
const PREFIX = 2 + LW; // '> ' + label
const ARROWS = 2; // room for ‹ and › each side of the choices

// Slice of choices that fits `avail` cols and always contains the cursor (index `at`).
const windowChoices = (choices: readonly string[], at: number, avail: number) => {
	const w = choices.map((c) => c.length + 2);
	let lo = at;
	let hi = at;
	let used = w[at]!;
	for (let moved = true; moved;) {
		moved = false;
		if (hi + 1 < w.length && used + 1 + w[hi + 1]! <= avail) {
			used += 1 + w[++hi]!;
			moved = true;
		}
		if (lo > 0 && used + 1 + w[lo - 1]! <= avail) {
			used += 1 + w[--lo]!;
			moved = true;
		}
	}
	return {lo, hi};
};

// sel: selected row. cur: per-row choice cursor (h/l). state: the committed values, marked [x].
export function ConfigModal({
	sel,
	cur,
	state,
	width,
}: {
	sel: number;
	cur: number[];
	state: SettingsState;
	width: number;
}) {
	const t = useTheme();
	return (
		<Box
			flexDirection="column"
			borderStyle="round"
			borderColor={t.modalBorder}
			backgroundColor={t.modalBg}
			width={width}
			height={configHeight}
		>
			<Text bold> Config</Text>
			{SETTINGS.map((s, i) => {
				const value = s.get(state);
				const at = cur[i] ?? Math.max(0, s.choices.indexOf(value));
				const {lo, hi} = windowChoices(s.choices, at, width - 2 - PREFIX - ARROWS * 2);
				return (
					<Text
						key={s.path}
						wrap="truncate"
						backgroundColor={i === sel ? t.selBg : undefined}
						color={i === sel ? t.selFg : undefined}
					>
						{i === sel ? '>' : ' '} {s.label.padEnd(LW)}
						{lo > 0 ? '‹ ' : '  '}
						{s.choices.slice(lo, hi + 1).map((c, k) => (
							<Text
								key={c}
								inverse={i === sel && lo + k === at}
								backgroundColor={i === sel ? t.selBg : undefined}
								color={i === sel ? t.selFg : undefined}
							>
								{k ? ' ' : ''}
								{c === value ? `[${c}]` : ` ${c} `}
							</Text>
						))}
						{hi < s.choices.length - 1 ? ' ›' : ''}
					</Text>
				);
			})}
			<Text color={t.dim}> j/k row h/l browse enter select esc close</Text>
		</Box>
	);
}
