import {useEffect, useRef, useMemo, useState} from 'react';
import {Box, Text, useApp, useInput, useStdin, useStdout} from 'ink';
import {readFile, writeFile} from 'node:fs/promises';
import {join, resolve} from 'node:path';
import fuzzysort from 'fuzzysort';
import {loadDiff, listFiles, MODES, type DiffFile, type Mode} from './diff/load.js';
import {FileModal} from './components/FileModal.js';
import {SearchModal, type Hit} from './components/SearchModal.js';
import {HelpModal, helpHeight} from './components/HelpModal.js';
import {ThemeContext, THEMES, THEME_NAMES, type ThemeName} from './theme.js';
import {ConfigModal} from './components/ConfigModal.js';
import {SETTINGS, type Actions} from './settings.js';
import {DEFAULTS, saveConfig} from './config.js';
import {askH, sentBase, sentH, type AskMode, type AskSel, type SentQ} from './components/AskBox.js';
import {DeleteModal} from './components/DeleteModal.js';
import {QuitModal} from './components/QuitModal.js';
import {threadBody, windowBody} from './components/answerView.js';
import {McpModal, type McpConfirm, type McpPreview} from './components/McpModal.js';
import {useMcp} from './useMcp.js';
import type {McpBridge} from './mcp/bridge.js';
import {useAsk} from './useAsk.js';
import {exportName, renderReviewMarkdown, type AskController, type Question} from './ask/index.js';
import {footerFor} from './keys.js';
import {
	DiffView,
	toRows,
	toSplit,
	changeStarts,
	rowNo,
	rowCode,
	type Row,
	type SRow,
	type Sel,
	type PaneSide,
	paneOf,
} from './components/DiffView.js';

export type {Question};
type Sent = Omit<SentQ, 'body'> & {
	message: string;
	id: string;
	file: string;
	no?: number;
	del: boolean;
	idx: number;
	side: PaneSide;
};
// does row match the anchor (line number, deleted-side flag)? works for unified + split rows
const isDel = (r?: Row | SRow) => (r?.kind === 'line' ? r.type === 'del' : r?.kind === 'pair' ? !r.r : false);
const anchors = (r: Row | SRow | undefined, no: number, del: boolean, side: PaneSide = 'new') =>
	!r
		? false
		: side === 'old' && r.kind === 'pair'
			? r.l?.oldNo === no
			: side === 'old' && r.kind === 'line'
				? r.type !== 'add' && r.oldNo === no
				: r.kind === 'line'
					? del
						? r.type === 'del' && r.oldNo === no
						: r.newNo === no
					: r.kind === 'pair'
						? del
							? !r.r && r.l?.oldNo === no
							: r.r?.newNo === no
						: false;
const cls = (ch?: string) => (!ch || /\s/.test(ch) ? 0 : /\w/.test(ch) ? 1 : 2);
const rowText = (r?: Row | SRow, side: PaneSide = 'new') =>
	!r ? '' : r.kind === 'pair' ? ((paneOf(r, side) === 'old' ? r.l : r.r)?.text ?? '') : r.text;

const BINARY_MSG = 'binary file, not shown';
const EMPTY: DiffFile = {path: '', adds: 0, dels: 0, binary: false, hunks: []} as DiffFile;

export default function App({
	args,
	cwd,
	mode: mode0 = DEFAULTS.view.mode,
	split: split0 = DEFAULTS.view.split,
	full: full0 = DEFAULTS.view.full,
	theme: theme0 = DEFAULTS.theme,
	configPath,
	confirmQuit: cq0 = DEFAULTS.app.confirmQuit,
	mcpAutostart: ma0 = DEFAULTS.mcp.autostart,
	onCursor,
	onQuestion,
	onQuestionUpdate,
	onQuestionDelete,
	mcp: mcp0,
}: {
	args: string[];
	cwd?: string;
	mode?: Mode;
	split?: boolean;
	full?: boolean;
	theme?: ThemeName;
	configPath?: string;
	confirmQuit?: boolean;
	mcpAutostart?: boolean; // start the MCP bridge once after first render
	mcp?: McpBridge | ((ctl: AskController) => McpBridge); // MCP bridge (default: lazily created, off until started)
	onQuestion?: (q: Question) => void; // prompt submitted
	onQuestionUpdate?: (q: Question) => void; // sent comment edited
	onQuestionDelete?: (q: Question) => void; // sent comment deleted
	onCursor?: (c: {index: number; row: Row | SRow | undefined} | undefined) => void; // cursor row hook (prompt anchor)
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
	const [confirmQuit, setConfirmQuit] = useState(cq0);
	const [mcpAutostart, setMcpAutostart] = useState(ma0);
	const [qmodal, setQmodal] = useState(false);
	const [ask, setAsk] = useState(false);
	const [askText, setAskText] = useState('');
	const [askPos, setAskPos] = useState(0);
	const [sent, setSent] = useState<Sent[]>([]);
	const [focus, setFocus] = useState<string>();
	const [editId, setEditId] = useState<string>();
	const [fuId, setFuId] = useState<string>(); // follow-up input open for this comment
	const [scrolls, setScrolls] = useState<Record<string, {off: number; follow?: boolean}>>({});
	const [dmodal, setDmodal] = useState(false);
	const [tCommit, setTCommit] = useState<ThemeName>(theme0); // theme that esc reverts to
	const [split, setSplit] = useState(split0);
	const cols = stdout.columns || 80;
	const eff = split && cols >= 100;
	const [full, setFull] = useState(full0);
	const keep = useRef<string | undefined>(undefined);
	const [sel, setSel] = useState(0);
	const [ctl, ask0] = useAsk();
	const {questions, answers} = ask0;
	const [mcp, ms] = useMcp(ctl, mcp0, cwd);
	const [mmodal, setMmodal] = useState(false);
	const [msel, setMsel] = useState(0);
	const [mconfirm, setMconfirm] = useState<McpConfirm>();
	const [mprev, setMprev] = useState<McpPreview>();
	const [mnote, setMnote] = useState<string>();
	const [chosen, setChosen] = useState<AskMode>();
	// autostart: once after mount, not blocking; failure leaves MCP off and shows the bridge error (never the token)
	useEffect(() => {
		if (!ma0) return;
		void mcp.start().then(() => {
			const e = mcp.getState().error;
			if (e) setNote(`mcp autostart failed: ${e}`);
		});
	}, []);
	// default: ask while MCP runs, else save; a chosen mode only counts while MCP runs
	const sendMode: AskMode = ms.running ? (chosen ?? 'ask') : 'save';
	useEffect(() => {
		if (!ms.running) setChosen(undefined);
	}, [ms.running]);
	// comments added by an agent join the inline list
	useEffect(() => {
		const fresh = questions.filter((q) => q.origin === 'agent' && !sent.some((x) => x.id === q.id));
		if (fresh.length)
			setSent((l) => [
				...l,
				...fresh.map((q) => ({
					id: q.id!,
					file: q.file,
					no: q.line,
					del: false,
					idx: q.index,
					side: q.side ?? ('new' as PaneSide),
					head: `agent note ${q.line !== undefined ? 'L' + q.line : ''}`.trim(),
					lines: [],
					message: q.message,
				})),
			]);
	}, [questions]);
	const mcpClose = () => {
		setMmodal(false);
		setMconfirm(undefined);
		setMprev(undefined);
		setMnote(undefined);
	};
	const quit = () => {
		ctl.dispose();
		mcp.dispose();
		exit();
	};
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

	const [browsePath, setBrowsePath] = useState<string>();
	const [browseText, setBrowseText] = useState('');
	const [srch, setSrch] = useState(false);
	const [query, setQuery] = useState('');
	const [ssel, setSsel] = useState(0);
	const [all, setAll] = useState<string[]>([]);
	const hits: Hit[] = useMemo(
		() =>
			query
				? fuzzysort.go(query, all).map((r) => ({path: r.target, idx: r.indexes}))
				: all.map((p) => ({path: p, idx: []})),
		[query, all],
	);
	const srchOpen = () => {
		setQuery('');
		setSsel(0);
		setAll([]);
		setSrch(true);
		listFiles(cwd).then(setAll, () => {});
	};
	// Reload diff + browsed file from disk. Silent: no state change if nothing differs.
	const filesRef = useRef(files);
	filesRef.current = files;
	const reload = (manual: boolean) => {
		const bp = browsePath;
		loadDiff(mode, args, cwd, full).then(
			(f) => {
				const cur = filesRef.current;
				if (cur && JSON.stringify(cur) === JSON.stringify(f)) return;
				const path = cur?.[idx]?.path;
				setFiles(f);
				setErr(undefined);
				if (path !== undefined)
					setIdx(
						Math.max(
							0,
							f.findIndex((x) => x.path === path),
						),
					);
			},
			(e) => manual && setNote(String(e.message ?? e)),
		);
		if (bp !== undefined)
			readFile(join(cwd ?? '.', bp)).then(
				(buf) => setBrowseText(buf.subarray(0, 8000).includes(0) ? BINARY_MSG : buf.toString('utf8')),
				() => {},
			);
		if (manual) setNote('reloaded');
	};
	const reloadRef = useRef(reload);
	reloadRef.current = reload;
	useEffect(() => mcp.onFilesChanged(() => reloadRef.current(false)), [mcp]);
	const openBrowse = (p: string) => {
		readFile(join(cwd ?? '.', p)).then(
			(buf) => {
				const t = buf.subarray(0, 8000).includes(0) ? BINARY_MSG : buf.toString('utf8');
				setBrowseText(t);
				setBrowsePath(p);
				setOff(0);
			},
			(e) => setNote(String(e.message ?? e)),
		);
	};
	const dfile = files?.[idx];
	const noChanges = !!files && !dfile;
	const file: DiffFile | undefined = useMemo(() => {
		if (browsePath === undefined) return dfile ?? EMPTY;
		const lines = browseText.replace(/\n$/, '').split('\n');
		return {
			path: browsePath,
			adds: 0,
			dels: 0,
			binary: false,
			hunks: [{header: '', lines: lines.map((text, i) => ({type: 'normal', oldNo: i + 1, newNo: i + 1, text}))}],
		} as DiffFile;
	}, [dfile, browsePath, browseText]);
	const rows = useMemo(() => {
		if (!file) return [];
		if (browsePath !== undefined) return toRows(file).slice(1); // drop empty hunk header
		return eff ? toSplit(toRows(file)) : toRows(file);
	}, [file, eff, browsePath]);
	const built = useMemo(() => {
		const m = new Map<number, SentQ[]>();
		const info = new Map<string, {v: number; off: number; maxOff: number}>();
		const bw = Math.max(1, Math.max(10, cols - 1) - 3);
		const fuRows = fuId ? askH({head: '', lines: []}) : 0; // follow-up input under the thread
		const mkQ = (q: Sent, foc: boolean, v: number): SentQ => {
			const an = answers[q.id];
			const qq = questions.find((x) => x.id === q.id);
			const agentQ = qq?.origin === 'agent';
			const tu = qq?.turns?.length ? qq.turns : [{message: q.message}];
			const multi = tu.length > 1;
			const live = an?.status === 'pending' || an?.status === 'streaming';
			const body = threadBody([{...tu[0]!, message: q.message}, ...tu.slice(1)], q.message, bw);
			const sc = scrolls[q.id];
			const w = windowBody(body, foc, v, live && sc?.follow !== false ? Infinity : (sc?.off ?? 0));
			if (foc) info.set(q.id, {v, off: w.off, maxOff: w.maxOff});
			const {id, head, lines} = q;
			return {
				id,
				head,
				lines,
				body: w.shown,
				more: w.more,
				focused: foc,
				saved: !an && !agentQ && !multi,
				canAsk: !agentQ && !multi && (!an || an.status === 'error' || an.status === 'cancelled'),
				canFollow: !agentQ && an?.status === 'done',
				scroll: foc && w.maxOff > 0,
				pos: foc && w.maxOff > 0 ? `${w.from}-${w.to}/${w.total}` : undefined,
			};
		};
		const rowOf = new Map<string, number>();
		for (const q of sent) {
			if (q.file !== file?.path) continue;
			const i =
				q.no === undefined
					? q.side === 'old' || rowNo(rows[q.idx]) === undefined
						? q.idx
						: -1
					: rows.findIndex((r) => anchors(r, q.no!, q.del, q.side));
			if (i < 0) continue;
			rowOf.set(q.id, i);
			m.set(i, [...(m.get(i) ?? []), mkQ(q, false, 0)]);
		}
		// focused box takes the room the viewport has left on its row
		const f = sent.find((x) => x.id === focus && rowOf.has(x.id));
		if (f) {
			const list = m.get(rowOf.get(f.id)!)!;
			const k = list.findIndex((x) => x.id === f.id);
			const others = list.reduce((n, x, j) => n + (j === k ? 0 : sentH(x)), 0);
			const v = Math.max(1, height - 1 - sentBase({lines: f.lines, focused: true}) - others - fuRows);
			list[k] = mkQ(f, true, v);
		}
		return {m, info};
	}, [sent, rows, file, focus, answers, questions, cols, height, scrolls, fuId]);
	const sentAt = built.m;
	// comments of this file in visual order
	const focusList = useMemo(
		() => [...sentAt.entries()].sort((a, b) => a[0] - b[0]).flatMap(([row, l]) => l.map((q) => ({id: q.id!, row}))),
		[sentAt],
	);
	const fo = focusList.findIndex((x) => x.id === focus);
	const askable = (id: string) => {
		const q = questions.find((x) => x.id === id);
		const a = answers[id];
		return (
			!!q &&
			q.origin !== 'agent' &&
			(q.turns?.length ?? 1) <= 1 &&
			(!a || a.status === 'error' || a.status === 'cancelled')
		);
	};
	const askFocused = () => {
		const q = questions.find((x) => x.id === focus);
		if (!q) return;
		if (q.origin === 'agent') return setNote("agent notes can't be asked");
		const an = answers[q.id!];
		if (an?.status === 'pending' || an?.status === 'streaming') return setNote('still waiting for the agent');
		if (an?.status === 'done') {
			setAskText('');
			setAskPos(0);
			setFuId(q.id);
			setAsk(true);
			return;
		}
		if (!askable(q.id!)) return setNote("can't retry a follow-up yet");
		setNote(mcp.ask(q) ? 'question queued' : 'MCP is off (M to start)');
	};
	const askAll = () => {
		const todo = focusList.filter((x) => askable(x.id)).map((x) => questions.find((q) => q.id === x.id)!);
		if (!todo.length) return setNote('nothing to ask');
		if (!ms.running) return setNote('MCP is off (M to start)');
		const n = todo.filter((q) => mcp.ask(q)).length;
		setNote(`queued ${n} question${n === 1 ? '' : 's'}`);
	};
	const focusTo = (x?: {id: string; row: number}) => {
		setFocus(x?.id);
		if (x) {
			setCur(x.row);
			endVis();
		}
	};
	const rowH = (i: number, extra = 0) => 1 + (sentAt.get(i) ?? []).reduce((n, q) => n + sentH(q), 0) + extra;
	// smallest offset showing everything to the last row within h lines
	const fitOff = (h: number, upto = rows.length - 1, askAt = -1, askExtra = 0) => {
		let o = upto;
		let used = rowH(o, o === askAt ? askExtra : 0);
		while (o > 0 && used + rowH(o - 1, o - 1 === askAt ? askExtra : 0) <= h)
			used += rowH(--o, o === askAt ? askExtra : 0);
		return o;
	};
	const max = rows.length ? fitOff(height) : 0;
	const starts = useMemo(() => changeStarts(rows), [rows]);
	const CTX = 3;
	const CTX_WIDE = 15;
	const [curOn, setCurOn] = useState(false);
	const [cur, setCur] = useState(0);
	const pend = useRef(0); // pending count prefix
	const last = Math.max(0, rows.length - 1);
	const curI = Math.min(cur, last);
	const curRow: Row | SRow | undefined = curOn ? rows[curI] : undefined;
	// char cursor: `col` is the desired column (vim curswant); shown column is clamped to the row
	const [col, setCol] = useState(0);
	const [anchor, setAnchor] = useState<{row: number; col: number}>();
	const [vline, setVline] = useState(false);
	const [hoff, setHoff] = useState(0);
	// split cursor pane; locked to one side, `p` toggles. Unified/browse: always 'new'
	const [side0, setSide] = useState<PaneSide>('new');
	const canSide = eff && browsePath === undefined && curOn;
	const side: PaneSide = canSide ? side0 : 'new';
	const tx = (i: number) => rowCode(rows[i], side);
	const clampC = (i: number, c: number) => Math.min(c, Math.max(0, tx(i).length - 1));
	const ccol = clampC(curI, col);
	const endVis = () => {
		setAnchor(undefined);
		setVline(false);
	};
	useEffect(() => {
		setCur(full && starts.length ? starts[0]! : 0);
		setCol(0);
		setSide('new');
		endVis();
		pend.current = 0;
	}, [rows, full]);
	// selection range, ordered, inclusive
	const vsel: Sel | undefined = useMemo(() => {
		if (!curOn || !anchor) return undefined;
		const a = {row: Math.min(anchor.row, last), col: anchor.col};
		const b = {row: curI, col: ccol};
		const [p, q] = a.row < b.row || (a.row === b.row && a.col <= b.col) ? [a, b] : [b, a];
		return {sr: p.row, sc: p.col, er: q.row, ec: q.col, line: vline};
	}, [curOn, anchor, vline, curI, ccol, last]);
	const selText = (s: Sel) =>
		Array.from({length: s.er - s.sr + 1}, (_, k) => {
			const t = tx(s.sr + k);
			return s.line ? t : t.slice(k === 0 ? s.sc : 0, s.sr + k === s.er ? s.ec + 1 : undefined);
		}).join('\n');
	const selTag = (s: Sel) => {
		const lb = (i: number) => {
			const n = rowNo(rows[i], side);
			return n === undefined ? `r${i + 1}` : `L${n}`;
		};
		const [a, b] = [lb(s.sr), lb(s.er)];
		if (s.line) return a === b ? a : `${a}-${b.slice(1)}`;
		return a === b ? `${a}:C${s.sc + 1}-C${s.ec + 1}` : `${a}:C${s.sc + 1}-${b}:C${s.ec + 1}`;
	};
	const editing = editId ? sent.find((q) => q.id === editId) : undefined;
	const askSel: AskSel | undefined = editing
		? {head: `edit ${editing.head}`, lines: editing.lines}
		: fuId
			? {head: 'follow-up', lines: []}
			: ask && vsel
				? {head: `selection ${selTag(vsel)}`, lines: selText(vsel).split('\n')}
				: undefined;
	const wordMove = (kind: 'w' | 'b' | 'e', n: number) => {
		let r = curI;
		let c = ccol;
		const len = (i: number) => tx(i).length;
		for (let k = 0; k < n; k++) {
			if (kind === 'w') {
				let t = tx(r);
				const k0 = cls(t[c]);
				if (k0) while (c < t.length && cls(t[c]) === k0) c++;
				for (;;) {
					while (c < t.length && cls(t[c]) === 0) c++;
					if (c < t.length) break;
					if (r >= last) {
						c = Math.max(0, t.length - 1);
						break;
					}
					r++;
					t = tx(r);
					c = 0;
					if (!t.length) break;
				}
			} else if (kind === 'b') {
				let c0 = c - 1;
				let done = false;
				for (;;) {
					while (c0 < 0) {
						if (r === 0) {
							c0 = 0;
							done = true;
							break;
						}
						r--;
						c0 = len(r) - 1;
						if (len(r) === 0) {
							c0 = 0;
							done = true;
							break;
						}
					}
					if (done) break;
					if (cls(tx(r)[c0]) === 0) c0--;
					else break;
				}
				if (!done) {
					const t = tx(r);
					const k0 = cls(t[c0]);
					while (c0 > 0 && cls(t[c0 - 1]) === k0) c0--;
				}
				c = c0;
			} else {
				let c0 = c + 1;
				let t = tx(r);
				for (;;) {
					while (c0 >= t.length) {
						if (r >= last) {
							c0 = Math.max(0, t.length - 1);
							break;
						}
						r++;
						t = tx(r);
						c0 = 0;
					}
					if (c0 < t.length && cls(t[c0]) === 0) c0++;
					else break;
				}
				const k0 = cls(t[c0]);
				if (k0) while (c0 + 1 < t.length && cls(t[c0 + 1]) === k0) c0++;
				c = c0;
			}
		}
		setCur(r);
		setCol(c);
	};
	// scroll follows cursor, keeping ~2 rows of context
	useEffect(() => {
		if (!curOn) return;
		const ex = ask ? askH(askSel) : 0; // input box under the cursor row
		const so = ask ? 0 : Math.min(2, Math.floor((height - 1) / 2));
		setLskip((v) => (v.n ? {row: 0, n: 0} : v)); // cursor follow shows whole row blocks
		setOff((o) => {
			if (curI < o + so) o = Math.max(0, curI - so);
			const end = Math.min(last, curI + so);
			let need = 0;
			for (let k = o; k <= end; k++) need += rowH(k, k === curI ? ex : 0);
			while (need > height && o < curI) need -= rowH(o++, o - 1 === curI ? ex : 0);
			return Math.max(0, Math.min(o, fitOff(height, last, ask ? curI : -1, ex)));
		});
	}, [cur, curOn, height, max, ask, askSel?.lines.length, sentAt]);
	// horizontal scroll keeps the char cursor visible; code area width excludes the gutter
	const cw = Math.max(
		1,
		eff && browsePath === undefined ? Math.floor((cols - 1) / 2) - 7 : cols - (browsePath !== undefined ? 7 : 12),
	);
	useEffect(() => {
		if (!curOn) return setHoff(0);
		const m = Math.min(4, Math.floor((cw - 1) / 2));
		setHoff((h) => (ccol < h + m ? Math.max(0, ccol - m) : ccol > h + cw - 1 - m ? ccol - cw + 1 + m : h));
	}, [curOn, ccol, cw, curI, rows]);
	useEffect(() => {
		onCursor?.(curOn ? {index: Math.min(cur, last), row: curRow} : undefined);
	}, [curOn, cur, rows]);
	const mvCur = (n: number) => setCur((c) => Math.min(last, Math.max(0, c + n)));
	useEffect(() => {
		setOff(full && starts.length ? Math.min(max, Math.max(0, starts[0]! - CTX)) : 0);
	}, [rows, full]);
	const jump = (d: 1 | -1) => {
		const t = d > 0 ? starts.find((s) => s - CTX > off) : [...starts].reverse().find((s) => s - CTX < off);
		if (t !== undefined) setOff(Math.min(max, Math.max(0, t - CTX)));
	};
	// line-level scroll: a row block (row + comment boxes) may be cut at the top; `skip` belongs to row `row`
	const [lskip, setLskip] = useState({row: 0, n: 0});
	const skip = lskip.row === off ? lskip.n : 0;
	const scroll = (n: number) => {
		let o = off;
		let k = skip;
		for (let i = 0; i < Math.abs(n); i++) {
			if (n > 0) {
				if (o >= max) break;
				if (k + 1 < rowH(o)) k++;
				else {
					o++;
					k = 0;
				}
			} else if (k > 0) k--;
			else if (o > 0) k = rowH(--o) - 1;
			else break;
		}
		if (o >= max) k = 0;
		setOff(o);
		setLskip({row: o, n: k});
	};
	const sw = (d: number) => {
		if (!files?.length) return;
		setIdx((i) => (i + d + files.length) % files.length);
		setOff(0);
	};

	const actions: Actions = {
		confirmQuit: setConfirmQuit,
		mcpAutostart: setMcpAutostart, // preference only; the M modal starts/stops
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
		const st = {theme, mode, split, full, confirmQuit, mcpAutostart};
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

	const exportReview = () => {
		const qs = ctl.getState().questions;
		if (!qs.length) return setNote('no comments to export');
		const now = new Date();
		const path = join(resolve(cwd ?? '.'), exportName(now));
		const md = renderReviewMarkdown(qs, {cwd: resolve(cwd ?? '.'), mode, args, date: now});
		writeFile(path, md).then(
			() => setNote(`exported ${qs.length} comment${qs.length === 1 ? '' : 's'} -> ${path}`),
			(e) => setNote(`export failed: ${e.message ?? e}`),
		);
	};

	useInput(
		(input, key) => {
			if (ask) {
				pend.current = 0;
				if (key.escape) {
					setAsk(false);
					setAskText('');
					setEditId(undefined);
					setFuId(undefined);
				} else if (key.return) {
					const message = askText.trim();
					if (!message) return;
					if (fuId) {
						if (mcp.followUp(fuId, message)) {
							setScrolls((v) => ({...v, [fuId]: {off: 0, follow: true}}));
							setNote('follow-up queued');
							setFuId(undefined);
							setAsk(false);
							setAskText('');
						} else setNote(ms.running ? "can't follow up yet" : 'MCP is off (M to start)');
						return;
					}
					if (editId) {
						const q1 = ctl.edit(editId, message);
						setSent((l) => l.map((x) => (x.id === editId ? {...x, message} : x)));
						if (q1) onQuestionUpdate?.(q1);
						setNote('comment updated');
						setEditId(undefined);
						setAsk(false);
						setAskText('');
						return;
					}
					const q: Question = {
						file: file?.path ?? '',
						index: curI,
						side,
						line: rowNo(curRow, side),
						text: rowText(curRow, side),
						message,
					};
					if (vsel) {
						q.text = selText(vsel);
						q.startLine = rowNo(rows[vsel.sr], side) ?? q.line;
						q.endLine = rowNo(rows[vsel.er], side) ?? q.line;
						q.startCol = vsel.line ? 1 : vsel.sc + 1;
						q.endCol = vsel.line ? Math.max(1, tx(vsel.er).length) : vsel.ec + 1;
					}
					const ar = vsel ? vsel.er : curI;
					q.context = [];
					q.wide = [];
					for (let i = Math.max(0, (vsel ? vsel.sr : curI) - CTX_WIDE); i <= Math.min(last, ar + CTX_WIDE); i++)
						if (rows[i]?.kind === 'line' || rows[i]?.kind === 'pair') {
							const tx = rowText(rows[i], side);
							q.wide.push(tx);
							if (i >= (vsel ? vsel.sr : curI) - CTX && i <= ar + CTX) q.context.push(tx);
						}
					const added = ctl.add(q);
					q.id = added.id!;
					const id = q.id;
					setSent((l) => [
						...l,
						{
							id,
							file: file?.path ?? '',
							no: rowNo(rows[ar], side),
							del: isDel(rows[ar]),
							idx: ar,
							side,
							head: vsel
								? `selection ${selTag(vsel)}`
								: `line ${rowNo(curRow, side) !== undefined ? 'L' + rowNo(curRow, side) : 'r' + (curI + 1)}`,
							lines: vsel ? selText(vsel).split('\n') : [],
							message,
						},
					]);
					endVis();
					setNote(
						sendMode === 'ask'
							? mcp.ask(added)
								? 'question sent to agent'
								: 'MCP is off (M to start)'
							: `question saved (${questions.length + 1})`,
					);
					onQuestion?.(q);
					setAsk(false);
					setAskText('');
				} else if (key.tab) {
					if (editId || fuId) return;
					if (sendMode === 'ask') setChosen('save');
					else if (ms.running) setChosen('ask');
					else setNote('MCP is off (M to start)');
				} else if (key.leftArrow) setAskPos((p) => Math.max(0, p - 1));
				else if (key.rightArrow) setAskPos((p) => Math.min(askText.length, p + 1));
				else if (key.backspace || key.delete) {
					if (askPos > 0) {
						setAskText(askText.slice(0, askPos - 1) + askText.slice(askPos));
						setAskPos(askPos - 1);
					}
				} else if (input && !key.ctrl && !key.meta && !key.tab) {
					const s = input.replace(/[\r\n]+/g, ' ');
					setAskText(askText.slice(0, askPos) + s + askText.slice(askPos));
					setAskPos(askPos + s.length);
				}
				return;
			}
			if (dmodal) {
				if (input === 'y' || key.return) {
					const nx = focusList[fo + 1];
					const q0 = questions.find((x) => x.id === focus);
					if (focus) ctl.remove(focus);
					setSent((l) => l.filter((x) => x.id !== focus));
					if (q0) onQuestionDelete?.(q0);
					focusTo(nx);
					setNote('comment deleted');
					setDmodal(false);
				} else if (input === 'n' || key.escape) setDmodal(false);
				return;
			}
			if (qmodal) {
				if (input === 'y' || key.return) quit();
				else if (input === 'n' || input === 'q' || key.escape) setQmodal(false);
				return;
			}
			if (srch) {
				if (key.escape) setSrch(false);
				else if (key.return) {
					const h = hits[ssel];
					if (h) {
						setSrch(false);
						openBrowse(h.path);
					}
				} else if (key.downArrow || (key.ctrl && input === 'n')) setSsel((s) => Math.min(hits.length - 1, s + 1));
				else if (key.upArrow || (key.ctrl && input === 'p')) setSsel((s) => Math.max(0, s - 1));
				else if (key.backspace || key.delete) {
					setQuery((q) => q.slice(0, -1));
					setSsel(0);
				} else if (input && !key.ctrl && !key.meta && !key.tab) {
					setQuery((q) => q + input);
					setSsel(0);
				}
				return;
			}
			if (help) {
				if (key.escape || input === 'q' || input === '?') setHelp(false);
				return;
			}
			if (mmodal) {
				const it = msel > 0 ? ms.integrations[msel - 1] : undefined;
				if (mconfirm) {
					if (input === 'y' || key.return) {
						const c = mconfirm;
						setMconfirm(undefined);
						setMnote(undefined);
						void (c.kind === 'register' ? mcp.register(c.id) : mcp.unregister(c.id));
					} else if (input === 'n' || key.escape) setMconfirm(undefined);
					return;
				}
				const pick = (n: number) => {
					setMsel((v) => Math.min(ms.integrations.length, Math.max(0, v + n)));
					setMprev(undefined);
					setMnote(undefined);
				};
				if (key.escape || input === 'q' || input === 'M') mcpClose();
				else if (input === 'j' || key.downArrow) pick(1);
				else if (input === 'k' || key.upArrow) pick(-1);
				else if ((key.return || input === ' ') && msel === 0) {
					setMnote(undefined);
					void (ms.running ? mcp.stop() : mcp.start());
				} else if (it && (key.return || input === 'd')) {
					setMprev(undefined);
					if (!it.canRegister) setMnote('copy-paste only: c copies the snippet');
					else if (input === 'd') {
						if (!it.registered) setMnote('not registered');
						else {
							setMnote(undefined);
							setMconfirm({kind: 'unregister', id: it.id});
						}
					} else if (!ms.running) setMnote('start MCP first');
					else if (it.registered && !it.stale) setMnote('already registered (d to remove)');
					else {
						setMnote(undefined);
						setMconfirm({kind: 'register', id: it.id});
					}
				} else if (it && (input === 'c' || input === 'w')) {
					const text = input === 'c' ? mcp.commandText(it.id) : mcp.watchText(it.id);
					if (text === null) {
						setMprev(undefined);
						setMnote('start MCP first');
					} else {
						mcp.copy(text);
						setMnote(undefined);
						setMprev({
							what: `${input === 'c' ? 'register command' : 'watch prompt'} (copied)`,
							text: (input === 'c' ? mcp.commandText(it.id, true) : mcp.watchText(it.id, true)) ?? '',
						});
					}
				} else if (input === 'R') {
					if (!ms.running) setMnote('start MCP first');
					else void mcp.refreshRegistration();
				}
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
			if (input === 'F') return srchOpen();
			if (input === 'r') return reload(true);
			if (input === 'E') return exportReview();
			if (input === 'M') {
				setMsel(0);
				setMnote(undefined);
				setMprev(undefined);
				setMconfirm(undefined);
				return setMmodal(true);
			}
			if (input === 'i') {
				pend.current = 0;
				setFocus(undefined);
				setCurOn((v) => !v);
				setCol(0);
				setSide('new');
				endVis();
				return;
			}
			if (curOn) {
				const c = pend.current;
				const n = c || 1;
				pend.current = 0;
				if (key.escape) {
					if (fo >= 0) return setFocus(undefined);
					if (anchor) return endVis();
					return setCurOn(false);
				}
				if (input === 'J' || input === 'K') {
					if (!focusList.length) return setNote('no comments');
					const n = focusList.length;
					return focusTo(
						focusList[
							fo < 0 ? (input === 'J' ? 0 : n - 1) : Math.min(n - 1, Math.max(0, fo + (input === 'J' ? 1 : -1)))
						],
					);
				}
				if (fo >= 0 && (input === 'e' || key.return)) {
					if (ctl.turns(focus).length > 1) return setNote("can't edit after follow-ups");
					const m = sent.find((x) => x.id === focus)?.message ?? '';
					setAskText(m);
					setAskPos(m.length);
					setEditId(focus);
					setAsk(true);
					return;
				}
				if (fo >= 0 && input === 'D') return setDmodal(true);
				if (fo >= 0 && input === 'a') return askFocused();
				if (fo >= 0 && input === 'A') return askAll();
				const ti = fo >= 0 ? built.info.get(focus!) : undefined;
				if (ti && ti.maxOff > 0 && /^[jkdugG]$/.test(input)) {
					const hv = Math.max(1, Math.floor(ti.v / 2));
					const want =
						input === 'j'
							? ti.off + 1
							: input === 'k'
								? ti.off - 1
								: input === 'd'
									? ti.off + hv
									: input === 'u'
										? ti.off - hv
										: input === 'g'
											? 0
											: ti.maxOff;
					const o = Math.min(ti.maxOff, Math.max(0, want));
					const id = focus!;
					setScrolls((v) => ({
						...v,
						[id]: {off: o, follow: input === 'G' ? true : o < ti.off ? false : v[id]?.follow},
					}));
					return;
				}
				if (
					fo >= 0 &&
					(key.leftArrow ||
						key.rightArrow ||
						key.upArrow ||
						key.downArrow ||
						key.pageUp ||
						key.pageDown ||
						/^[hjklwbevVdugG0$^[\] p]$/.test(input))
				)
					setFocus(undefined);
				if (key.return || input === 'a') {
					setFocus(undefined);
					setAskText('');
					setAskPos(0);
					setAsk(true);
					return;
				}
				if (input && /^[0-9]$/.test(input) && (input !== '0' || c)) {
					pend.current = Math.min(99999, c * 10 + Number(input));
					return;
				}
				if (input === 'p' && canSide) {
					endVis(); // toggling pane ends any visual selection (selection stays on one side)
					return setSide((v) => (v === 'new' ? 'old' : 'new'));
				}
				if (input === 'h' || key.leftArrow) return setCol(Math.max(0, ccol - n));
				if (input === 'l' || key.rightArrow) return setCol(clampC(curI, ccol + n));
				if (input === '0') return setCol(0);
				if (input === '$') return setCol(1e9);
				if (input === '^') return setCol(Math.max(0, tx(curI).search(/\S/)));
				if (input === 'w' || input === 'b' || input === 'e') return wordMove(input, n);
				if (input === 'v' || input === 'V') {
					const line = input === 'V';
					if (anchor && vline === line) return endVis();
					if (!anchor) setAnchor({row: curI, col: ccol});
					setVline(line);
					return;
				}
				if (input === 'j' || key.downArrow) return mvCur(n);
				if (input === 'k' || key.upArrow) return mvCur(-n);
				if (input === 'd') return mvCur(half * n);
				if (input === 'u') return mvCur(-half * n);
				if (key.pageDown || input === ' ') return mvCur(height - 1);
				if (key.pageUp) return mvCur(-(height - 1));
				if (input === 'g') return setCur(0);
				if (input === 'G') return setCur(c ? Math.min(last, c - 1) : last);
				if (input === ']') return setCur((x) => starts.find((s) => s > x) ?? x);
				if (input === '[') return setCur((x) => [...starts].reverse().find((s) => s < x) ?? x);
			}
			if (browsePath !== undefined) {
				if (key.escape) {
					setBrowsePath(undefined);
					setOff(0);
					return;
				}
				if ('nfpcsm[]'.includes(input) && input) return;
				if (key.tab || key.leftArrow || key.rightArrow) return;
			}
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
			} else if (input === 'q') {
				if (confirmQuit) setQmodal(true);
				else quit();
			} else if ((key.tab && !key.shift) || key.rightArrow) sw(1);
			else if ((key.tab && key.shift) || key.leftArrow) sw(-1);
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

	const cn = rowNo(curRow, side);
	const curTag = curOn ? (
		<Text color={th.accent} bold>
			[{vsel ? 'visual' : 'cursor'}
			{canSide ? ` ${side}` : ''} {cn !== undefined ? `L${cn}` : `r${curI + 1}`}:C{ccol + 1}]{' '}
		</Text>
	) : null;
	const mcpChip = <Text color={th.view}>[mcp: {ms.running ? 'on' : 'off'}] </Text>;
	const rowsT = height + 3;
	const mw = Math.min(cols, Math.max(20, Math.floor(cols * 0.7)));
	const mh = Math.min(rowsT, Math.max(5, Math.min(files.length + 4, Math.floor(rowsT * 0.6))));

	return (
		<ThemeContext value={th}>
			<Box flexDirection="column" width={cols} height={rowsT}>
				<Text wrap="truncate">
					{noChanges && browsePath === undefined ? (
						<>
							<Text color={th.mode}>[{mode}] </Text>
							{mcpChip}
							<Text>No changes (m cycles mode, F search, q quits)</Text>
						</>
					) : browsePath !== undefined ? (
						<>
							<Text color={th.mode}>[browse] </Text>
							<Text color={th.view}>[{theme}] </Text>
							{mcpChip}
							{curTag}
							<Text color={th.file}>{file.path}</Text>
						</>
					) : (
						<>
							<Text color={th.mode}>[{mode}] </Text>
							<Text color={th.mode}>[{full ? 'full' : 'changes'}] </Text>
							<Text color={th.view}>[{split ? 'split' : 'unified'}] </Text>
							<Text color={th.view}>[{theme}] </Text>
							{mcpChip}
							<Text bold>
								[{idx + 1}/{files.length}]{' '}
							</Text>
							{curTag}
							<Text color={th.file}>
								{file.from ? `${file.from} -> ` : ''}
								{file.path}
							</Text>
							<Text color={th.adds}> +{file.adds}</Text>
							<Text color={th.dels}> -{file.dels}</Text>
						</>
					)}
				</Text>
				<Text color={th.dim}>{'─'.repeat(Math.max(1, (stdout.columns || 80) - 1))}</Text>
				<DiffView
					file={file}
					rows={rows}
					offset={off}
					height={height}
					cols={cols}
					name={theme}
					single={browsePath !== undefined}
					cur={curOn ? Math.min(cur, last) : -1}
					ask={ask ? {text: askText, pos: askPos, mode: editId || fuId ? undefined : sendMode} : undefined}
					askSel={askSel}
					col={ccol}
					sel={vsel}
					hoff={hoff}
					skip={skip}
					sent={sentAt}
					side={side}
				/>
				<Text color={th.dim} wrap="truncate">
					{note ? `${note} | ` : ''}
					{split && !eff ? 'too narrow for split | ' : ''}({Math.min(rows.length, off + 1)}-
					{Math.min(rows.length, off + height)}/{rows.length}){' '}
					{footerFor({
						cursor: curOn,
						visual: !!anchor,
						split: canSide,
						focused: fo >= 0,
						ask,
						edit: !!editId || !!fuId,
					})}
				</Text>
				{dmodal && (
					<Box position="absolute" width="100%" height="100%" alignItems="center" justifyContent="center">
						<DeleteModal />
					</Box>
				)}
				{qmodal && (
					<Box position="absolute" width="100%" height="100%" alignItems="center" justifyContent="center">
						<QuitModal />
					</Box>
				)}
				{help && (
					<Box position="absolute" width="100%" height="100%" alignItems="center" justifyContent="center">
						<HelpModal width={mw} height={Math.min(rowsT, helpHeight)} />
					</Box>
				)}
				{mmodal && (
					<Box position="absolute" width="100%" height="100%" alignItems="center" justifyContent="center">
						<McpModal
							state={ms}
							sel={msel}
							confirm={mconfirm}
							preview={mprev}
							note={mnote}
							width={Math.min(cols, 64)}
						/>
					</Box>
				)}
				{cmodal && (
					<Box position="absolute" width="100%" height="100%" alignItems="center" justifyContent="center">
						<ConfigModal
							sel={csel}
							cur={ccur}
							state={{theme: tCommit, mode, split, full, confirmQuit, mcpAutostart}}
							width={Math.min(cols, 56)}
						/>
					</Box>
				)}
				{srch && (
					<Box position="absolute" width="100%" height="100%" alignItems="center" justifyContent="center">
						<SearchModal
							query={query}
							hits={hits}
							sel={ssel}
							height={Math.min(rowsT, Math.max(8, Math.floor(rowsT * 0.7)))}
							width={mw}
						/>
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
