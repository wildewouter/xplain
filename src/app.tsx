import {useEffect, useRef, useMemo, useState} from 'react';
import {Box, Text, useApp, useInput, useStdin, useStdout} from 'ink';
import {loadDiff, MODES, type DiffFile, type Mode} from './diff/load.js';
import {FileModal} from './components/FileModal.js';
import {HelpModal, helpHeight} from './components/HelpModal.js';
import {ThemeContext, THEMES, THEME_NAMES, type ThemeName} from './theme.js';
import {ConfigModal, configHeight} from './components/ConfigModal.js';
import {SETTINGS, type Actions} from './settings.js';
import {DEFAULTS, saveConfig} from './config.js';
import {footer} from './keys.js';
import {DiffView, toRows, toSplit, changeStarts} from './components/DiffView.js';

export default function App({
	args,
	cwd,
	mode: mode0 = DEFAULTS.view.mode,
	split: split0 = DEFAULTS.view.split,
	full: full0 = DEFAULTS.view.full,
	theme: theme0 = DEFAULTS.theme,
	configPath,
}: {
	args: string[];
	cwd?: string;
	mode?: Mode;
	split?: boolean;
	full?: boolean;
	theme?: ThemeName;
	configPath?: string;
}) {
	const [note, setNote] = useState<string>();
	const {exit} = useApp();
	const {stdout} = useStdout();
	const {isRawModeSupported} = useStdin();
	const [files, setFiles] = useState<DiffFile[] | null>(null);
	const [err, setErr] = useState<string>();
	const [mode, setMode] = useState<Mode>(mode0);
	const [idx, setIdx] = useState(0);
	const [off, setOff] = useState(0);
	const [modal, setModal] = useState(false);
	const [help, setHelp] = useState(false);
	const [theme, setTheme] = useState<ThemeName>(theme0);
	const th = THEMES[theme];
	const [cmodal, setCmodal] = useState(false);
	const [csel, setCsel] = useState(0);
	const [ccur, setCcur] = useState<number[]>([]);
	const [tCommit, setTCommit] = useState<ThemeName>(theme0); // theme that esc reverts to
	const [split, setSplit] = useState(split0);
	const cols = stdout.columns || 80;
	const eff = split && cols >= 100;
	const [full, setFull] = useState(full0);
	const keep = useRef<string | undefined>(undefined);
	const [sel, setSel] = useState(0);
	const height = Math.max(3, (stdout.rows || 24) - 3);
	const half = Math.max(1, Math.floor(height / 2));
	const mvSel = (n: number) => setSel((s) => Math.min((files?.length ?? 1) - 1, Math.max(0, s + n)));

	useEffect(() => {
		let live = true;
		loadDiff(mode, args, cwd, full).then(
			(f) => {
				if (!live) return;
				const k = keep.current;
				keep.current = undefined;
				if (k)
					setIdx(
						Math.max(
							0,
							f.findIndex((x) => x.path === k),
						),
					);
				setFiles(f);
				setErr(undefined);
			},
			(e) => live && setErr(String(e.message ?? e)),
		);
		return () => {
			live = false;
		};
	}, [mode, full]);

	const file = files?.[idx];
	const rows = useMemo(() => (file ? (eff ? toSplit(toRows(file)) : toRows(file)) : []), [file, eff]);
	const max = Math.max(0, rows.length - height);
	const starts = useMemo(() => changeStarts(rows), [rows]);
	const CTX = 3;
	useEffect(() => {
		setOff(full && starts.length ? Math.min(max, Math.max(0, starts[0]! - CTX)) : 0);
	}, [rows, full]);
	const jump = (d: 1 | -1) => {
		const t = d > 0 ? starts.find((s) => s - CTX > off) : [...starts].reverse().find((s) => s - CTX < off);
		if (t !== undefined) setOff(Math.min(max, Math.max(0, t - CTX)));
	};
	const scroll = (n: number) => setOff((o) => Math.min(max, Math.max(0, o + n)));
	const sw = (d: number) => {
		if (!files?.length) return;
		setIdx((i) => (i + d + files.length) % files.length);
		setOff(0);
	};

	const actions: Actions = {
		theme: (v) => {
			setTheme(v);
			setTCommit(v); // selected: preview becomes the committed theme
		},
		mode: (v) => {
			setMode(v);
			setIdx(0);
			setOff(0);
		},
		split: (v) => {
			setSplit(v);
			setOff(0);
		},
		full: (v) => {
			keep.current = file?.path;
			setFull(v);
			setOff(0);
		},
	};
	const cfgOpen = () => {
		const st = {theme, mode, split, full};
		setCcur(SETTINGS.map((s) => Math.max(0, s.choices.indexOf(s.get(st)))));
		setTCommit(theme);
		setCmodal(true);
	};
	// close without enter: drop any theme preview
	const cfgClose = () => {
		setTheme(tCommit);
		setCmodal(false);
	};
	// h/l: move the choice cursor. Theme row previews live; nothing is selected or saved yet.
	const cfgMove = (d: number) => {
		const s = SETTINGS[csel]!;
		const next = Math.min(s.choices.length - 1, Math.max(0, (ccur[csel] ?? 0) + d));
		setCcur((c) => c.map((x, i) => (i === csel ? next : x)));
		if (s.path === 'theme') setTheme(s.choices[next] as ThemeName);
	};
	// enter: select the choice under the cursor (apply + save)
	const cfgSelect = () => {
		const s = SETTINGS[csel]!;
		const v = s.choices[ccur[csel] ?? 0]!;
		s.set(v, actions);
		if (configPath) setNote(saveConfig(configPath, s.patch(v)));
	};

	useInput(
		(input, key) => {
			if (help) {
				if (key.escape || input === 'q' || input === '?') setHelp(false);
				return;
			}
			if (cmodal) {
				if (key.escape || input === 'q' || input === 'C') cfgClose();
				else if (input === 'j' || key.downArrow) setCsel((s) => Math.min(SETTINGS.length - 1, s + 1));
				else if (input === 'k' || key.upArrow) setCsel((s) => Math.max(0, s - 1));
				else if (input === 'h' || key.leftArrow) cfgMove(-1);
				else if (input === 'l' || key.rightArrow) cfgMove(1);
				else if (key.return || input === ' ') cfgSelect();
				return;
			}
			if (modal) {
				if (key.escape || input === 'q' || input === 'f') setModal(false);
				else if (key.return) {
					setIdx(sel);
					setOff(0);
					setModal(false);
				} else if (key.ctrl) return;
				else if (input === 'd') mvSel(half);
				else if (input === 'u') mvSel(-half);
				else if (input === 'j' || key.downArrow) mvSel(1);
				else if (input === 'k' || key.upArrow) mvSel(-1);
				return;
			}
			if (key.ctrl) return;
			if (input === 'd') scroll(half);
			else if (input === 'u') scroll(-half);
			else if (input === '?') setHelp(true);
			else if (input === 'C') cfgOpen();
			else if (input === 't') setTheme((v) => THEME_NAMES[(THEME_NAMES.indexOf(v) + 1) % THEME_NAMES.length]!);
			else if (input === 's') {
				setSplit((v) => !v);
				setOff(0);
			} else if (input === 'c') {
				keep.current = file?.path;
				setFull((v) => !v);
				setOff(0);
			} else if (input === ']') jump(1);
			else if (input === '[') jump(-1);
			else if (input === 'm') {
				setMode((m) => MODES[(MODES.indexOf(m) + 1) % MODES.length]!);
				setIdx(0);
				setOff(0);
			} else if (input === 'f') {
				setSel(idx);
				setModal(true);
			} else if (input === 'q') exit();
			else if (input === 'n' || (key.tab && !key.shift) || key.rightArrow) sw(1);
			else if (input === 'p' || (key.tab && key.shift) || key.leftArrow) sw(-1);
			else if (input === 'j' || key.downArrow) scroll(1);
			else if (input === 'k' || key.upArrow) scroll(-1);
			else if (key.pageDown || input === ' ') scroll(height - 1);
			else if (key.pageUp) scroll(-(height - 1));
			else if (input === 'g') setOff(0);
			else if (input === 'G') setOff(max);
		},
		{isActive: !!isRawModeSupported},
	);

	if (err) return <Text color="red">{err}</Text>;
	if (!files) return <Text dimColor>Loading...</Text>;
	if (!file) return <Text>No changes [{mode}] (m cycles mode, q quits)</Text>;

	const rowsT = height + 3;
	const mw = Math.min(cols, Math.max(20, Math.floor(cols * 0.7)));
	const mh = Math.min(rowsT, Math.max(5, Math.min(files.length + 4, Math.floor(rowsT * 0.6))));

	return (
		<ThemeContext value={th}>
			<Box flexDirection="column" width={cols} height={rowsT}>
				<Text wrap="truncate">
					<Text color={th.mode}>[{mode}] </Text>
					<Text color={th.mode}>[{full ? 'full' : 'changes'}] </Text>
					<Text color={th.view}>[{split ? 'split' : 'unified'}] </Text>
					<Text color={th.view}>[{theme}] </Text>
					<Text bold>
						[{idx + 1}/{files.length}]{' '}
					</Text>
					<Text color={th.file}>
						{file.from ? `${file.from} -> ` : ''}
						{file.path}
					</Text>
					<Text color={th.adds}> +{file.adds}</Text>
					<Text color={th.dels}> -{file.dels}</Text>
				</Text>
				<Text color={th.dim}>{'─'.repeat(Math.max(1, (stdout.columns || 80) - 1))}</Text>
				<DiffView file={file} rows={rows} offset={off} height={height} cols={cols} name={theme} />
				<Text color={th.dim} wrap="truncate">
					{note ? `${note} | ` : ''}
					{split && !eff ? 'too narrow for split | ' : ''}({Math.min(rows.length, off + 1)}-
					{Math.min(rows.length, off + height)}/{rows.length}) {footer}
				</Text>
				{help && (
					<Box position="absolute" width="100%" height="100%" alignItems="center" justifyContent="center">
						<HelpModal width={mw} height={Math.min(rowsT, helpHeight)} />
					</Box>
				)}
				{cmodal && (
					<Box position="absolute" width="100%" height="100%" alignItems="center" justifyContent="center">
						<ConfigModal sel={csel} cur={ccur} state={{theme: tCommit, mode, split, full}} width={Math.min(cols, 56)} />
					</Box>
				)}
				{modal && (
					<Box position="absolute" width="100%" height="100%" alignItems="center" justifyContent="center">
						<FileModal files={files} sel={sel} current={idx} height={mh} width={mw} />
					</Box>
				)}
			</Box>
		</ThemeContext>
	);
}
