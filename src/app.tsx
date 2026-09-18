import {useEffect, useMemo, useState} from 'react';
import {Box, Text, useApp, useInput, useStdin, useStdout} from 'ink';
import {loadDiff, MODES, type DiffFile, type Mode} from './diff/load.js';
import {FileModal} from './components/FileModal.js';
import {DiffView, toRows} from './components/DiffView.js';

export default function App({args, cwd, mode: mode0 = 'all'}: {args: string[]; cwd?: string; mode?: Mode}) {
	const {exit} = useApp();
	const {stdout} = useStdout();
	const {isRawModeSupported} = useStdin();
	const [files, setFiles] = useState<DiffFile[] | null>(null);
	const [err, setErr] = useState<string>();
	const [mode, setMode] = useState<Mode>(mode0);
	const [idx, setIdx] = useState(0);
	const [off, setOff] = useState(0);
	const [modal, setModal] = useState(false);
	const [sel, setSel] = useState(0);
	const height = Math.max(3, (stdout.rows || 24) - 3);
	const half = Math.max(1, Math.floor(height / 2));
	const mvSel = (n: number) => setSel(s => Math.min((files?.length ?? 1) - 1, Math.max(0, s + n)));

	useEffect(() => {
		let live = true;
		loadDiff(mode, args, cwd).then(f => live && (setFiles(f), setErr(undefined)), e => live && setErr(String(e.message ?? e)));
		return () => {
			live = false;
		};
	}, [mode]);

	const file = files?.[idx];
	const rows = useMemo(() => (file ? toRows(file) : []), [file]);
	const max = Math.max(0, rows.length - height);
	const scroll = (n: number) => setOff(o => Math.min(max, Math.max(0, o + n)));
	const sw = (d: number) => {
		if (!files?.length) return;
		setIdx(i => (i + d + files.length) % files.length);
		setOff(0);
	};

	useInput((input, key) => {
		if (modal) {
			if (key.escape || input === 'q' || input === 'f') setModal(false);
			else if (key.return) {
				setIdx(sel);
				setOff(0);
				setModal(false);
			} else if (key.ctrl && input === 'd') mvSel(half);
			else if (key.ctrl && input === 'u') mvSel(-half);
			else if (key.ctrl) return;
			else if (input === 'j' || key.downArrow) mvSel(1);
			else if (input === 'k' || key.upArrow) mvSel(-1);
			return;
		}
		if (key.ctrl) {
			if (input === 'd') scroll(half);
			else if (input === 'u') scroll(-half);
			return;
		}
		if (input === 'm') {
			setMode(m => MODES[(MODES.indexOf(m) + 1) % MODES.length]!);
			setIdx(0);
			setOff(0);
		} else if (input === 'f') {
			setSel(idx);
			setModal(true);
		} else if (input === 'q') exit();
		else if (input === 'n' || key.tab && !key.shift || key.rightArrow) sw(1);
		else if (input === 'p' || key.tab && key.shift || key.leftArrow) sw(-1);
		else if (input === 'j' || key.downArrow) scroll(1);
		else if (input === 'k' || key.upArrow) scroll(-1);
		else if (key.pageDown || input === ' ') scroll(height - 1);
		else if (key.pageUp) scroll(-(height - 1));
		else if (input === 'g') setOff(0);
		else if (input === 'G') setOff(max);
	}, {isActive: !!isRawModeSupported});

	if (err) return <Text color="red">{err}</Text>;
	if (!files) return <Text dimColor>Loading...</Text>;
	if (!file) return <Text>No changes [{mode}]  (m cycles mode, q quits)</Text>;

	const cols = stdout.columns || 80;
	const rowsT = height + 3;
	const mw = Math.min(cols, Math.max(20, Math.floor(cols * 0.7)));
	const mh = Math.min(rowsT, Math.max(5, Math.min(files.length + 4, Math.floor(rowsT * 0.6))));

	return (
		<Box flexDirection="column" width={cols} height={rowsT}>
			<Text wrap="truncate">
				<Text color="yellow">[{mode}] </Text>
				<Text bold>[{idx + 1}/{files.length}] </Text>
				<Text color="cyan">{file.from ? `${file.from} -> ` : ''}{file.path}</Text>
				<Text color="green"> +{file.adds}</Text>
				<Text color="red"> -{file.dels}</Text>
			</Text>
			<Text dimColor>{'─'.repeat(Math.max(1, (stdout.columns || 80) - 1))}</Text>
			<DiffView file={file} rows={rows} offset={off} height={height} />
			<Text dimColor wrap="truncate">n/p/tab file  f files  j/k scroll  ctrl-d/u half  pgup/pgdn page  g/G top/bot  m mode  q quit  ({Math.min(rows.length, off + 1)}-{Math.min(rows.length, off + height)}/{rows.length})</Text>
			{modal && (
				<Box position="absolute" width="100%" height="100%" alignItems="center" justifyContent="center">
					<FileModal files={files} sel={sel} current={idx} height={mh} width={mw} />
				</Box>
			)}
		</Box>
	);
}
