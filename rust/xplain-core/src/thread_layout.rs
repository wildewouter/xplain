//! Layout of comment boxes and the editor box under code rows, as toolkit-neutral styled lines.
//!
//! Spec: F-COMMENT-04 (box render), F-COMMENT-10 (agent note), F-ASK-05 (thread body, status, spinner text),
//! F-ASK-06 (code blocks), F-ASK-07 (thread scroll window, `… +N more`), F-ASK-08 (copy buttons),
//! F-ASK-09 (answer arrival display), F-NAV-09 (heights for viewport). Oracle: `src/components/answerView.ts`
//! (wrapText, answerView, richLines, threadBody, windowBody) and `src/components/AskBox.tsx` (askH, sentH).
//! Owner: component `comments` (D). Pure: state -> lines. `view::thread_box` (component `viewrows`) only maps
//! tones to theme colors and draws; `nav::viewport` only uses the heights.
//! Must not: know theme colors or the canvas.
//!
//! Tone conventions for the view (`view::thread_box`):
//! - `Border`: dim border or gutter glyph; in a `focused` box: accent + bold. The editor box (empty `id`) uses
//!   the modal border color and modal background/foreground.
//! - `Bold`: accent + bold (focused head). `Accent`: accent. `Dim`: dim. `Error`: the deletions color.
//! - `Button`: accent; `ButtonSelected`: accent, reverse, bold. `Code`: code text, highlighted by `lang`.
//! - `Caret`: inverse cell. Spinner glyphs are already resolved from `state.spinner`.
//!
//! Every line is exactly `width` chars wide (borders included), padded with spaces.

use crate::comments::{self, Answer, AnswerStatus, Comment};
use crate::state::{EditorKind, EditorState, Overlay, State};

/// Semantic style of a span; the view maps it to theme colors/attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Normal,
    Dim,
    Bold,
    Accent,
    Error,
    /// Code block content (syntax highlighted when `Span::lang` is set).
    Code,
    Button,
    ButtonSelected,
    /// Editor caret cell (drawn inverse).
    Caret,
    /// Box border / gutter glyphs.
    Border,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub tone: Tone,
    /// Fence language for `Tone::Code` (highlight.rs `language_for_fence`).
    pub lang: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BoxLine {
    pub spans: Vec<Span>,
}

/// One rendered box (comment thread or editor) with its exact lines, already truncated/wrapped to `width`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ThreadBox {
    /// Comment id, or empty for the editor box.
    pub id: String,
    pub focused: bool,
    pub lines: Vec<BoxLine>,
}

/// Editor rows without the selection preview: borders, input, hint.
pub const ASK_H: usize = 4;
/// Selected lines shown in the editor before `… +N more` (F-COMMENT-01).
pub const ASK_MAX: usize = 5;
/// Quoted selection lines shown in a comment box (F-COMMENT-04).
pub const SENT_MAX: usize = 3;
/// Answer lines shown when capped (`answerView`), unfocused / focused.
pub const ANS_MAX: usize = 12;
pub const ANS_MAX_FOCUS: usize = 30;
/// Unfocused body lines before `… +N more` (F-COMMENT-04).
pub const BODY_CAP: usize = 14;
pub const COPY_BTN: &str = "[ copy ]";
/// `│ ` before each code line.
pub const CODE_GUTTER: usize = 2;
const SPINNER: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// Box width: max(10, cols-1) (F-COMMENT-01/04).
pub fn box_width(state: &State) -> usize {
    (state.size.cols as usize).saturating_sub(1).max(10)
}

// ---------------------------------------------------------------- text wrapping

/// Wrap `text` to `width` cells (words, hard-break long words), keeping blank lines (`wrapText`).
/// Tabs become 2 spaces, CR removed, trailing blank lines dropped (keeping at least one line).
pub fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let w = width.max(1);
    let clean = text.replace('\r', "").replace('\t', "  ");
    let mut out: Vec<String> = Vec::new();
    for raw in clean.split('\n') {
        let mut l: Vec<char> = raw.chars().collect();
        if l.is_empty() {
            out.push(String::new());
            continue;
        }
        while l.len() > w {
            // lastIndexOf(' ', w): last space at index <= w
            let mut k = (0..=w).rev().find(|&i| l.get(i) == Some(&' ')).unwrap_or(0);
            if k == 0 {
                k = w;
            }
            let head: String = l[..k].iter().collect();
            out.push(head.trim_end().to_string());
            let rest: String = l[k..].iter().collect();
            l = rest.trim_start().chars().collect();
        }
        out.push(l.into_iter().collect());
    }
    while out.len() > 1 && out.last().is_some_and(String::is_empty) {
        out.pop();
    }
    out
}

// ---------------------------------------------------------------- thread body model

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyKind {
    Msg,
    Fu,
    Div,
    Ans,
    Btn,
    Code,
}

/// One display line of a thread body (`BodyLine` in answerView.ts).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyLine {
    pub text: String,
    pub kind: BodyKind,
    pub err: bool,
    /// Placeholder line of a live answer without text (spinner prefix).
    pub live: Option<AnswerStatus>,
    /// Button / code lines: fence language.
    pub lang: Option<String>,
    /// Button / code lines: code block index within the thread.
    pub blk: usize,
    /// Button lines: raw block text (copied).
    pub code: Option<String>,
}

fn plain_line(text: String, kind: BodyKind) -> BodyLine {
    BodyLine { text, kind, err: false, live: None, lang: None, blk: 0, code: None }
}

/// Fence open: optional leading whitespace, 3+ backticks or tildes, optional lang (first word).
fn fence_open(line: &str) -> Option<(char, usize, Option<String>)> {
    let t = line.trim_start();
    let c = t.chars().next()?;
    if c != '`' && c != '~' {
        return None;
    }
    let n = t.chars().take_while(|&x| x == c).count();
    if n < 3 {
        return None;
    }
    let rest: String = t.chars().skip(n).collect();
    let lang: String = rest.trim_start().chars().take_while(|x| !x.is_whitespace() && *x != '`').collect();
    Some((c, n, (!lang.is_empty()).then_some(lang)))
}

/// Fence close: same char, length >= open, only whitespace around.
fn fence_close(line: &str, c: char, n: usize) -> bool {
    let t = line.trim();
    t.chars().count() >= n && t.chars().all(|x| x == c)
}

/// Prose wrapped as usual; each fenced block becomes a copy button line plus hard-cut code lines
/// (fences not shown) (F-ASK-06). `blk` numbers blocks across the whole thread.
pub fn rich_lines(text: &str, width: usize, kind: BodyKind, blk: &mut usize) -> Vec<BodyLine> {
    let mut out = Vec::new();
    let mut prose: Vec<String> = Vec::new();
    let flush = |prose: &mut Vec<String>, out: &mut Vec<BodyLine>| {
        if !prose.is_empty() {
            for t in wrap_text(&prose.join("\n"), width) {
                out.push(plain_line(t, kind));
            }
        }
        prose.clear();
    };
    let src: Vec<String> = text.replace('\r', "").split('\n').map(str::to_string).collect();
    let mut i = 0;
    while i < src.len() {
        let Some((fc, n, lang)) = fence_open(&src[i]) else {
            prose.push(src[i].clone());
            i += 1;
            continue;
        };
        flush(&mut prose, &mut out);
        let mut code: Vec<&str> = Vec::new();
        i += 1;
        while i < src.len() && !fence_close(&src[i], fc, n) {
            code.push(&src[i]);
            i += 1;
        }
        i += 1; // closing fence (or past the end)
        let idx = *blk;
        *blk += 1;
        let label = match &lang {
            Some(l) => format!("{COPY_BTN} {l}"),
            None => COPY_BTN.to_string(),
        };
        out.push(BodyLine {
            text: label,
            kind: BodyKind::Btn,
            err: false,
            live: None,
            lang: lang.clone(),
            blk: idx,
            code: Some(code.join("\n")),
        });
        let cw = width.saturating_sub(CODE_GUTTER).max(1);
        for raw in code {
            let chars: Vec<char> = raw.replace('\t', "  ").chars().collect();
            let mut j = 0;
            while j == 0 || j < chars.len() {
                let from = j.min(chars.len());
                let end = (j + cw).min(chars.len());
                out.push(BodyLine {
                    text: chars[from..end].iter().collect(),
                    kind: BodyKind::Code,
                    err: false,
                    live: None,
                    lang: lang.clone(),
                    blk: idx,
                    code: None,
                });
                j += cw;
            }
        }
    }
    flush(&mut prose, &mut out);
    out
}

/// Status label in the answer divider (F-ASK-05).
pub fn status_label(s: AnswerStatus) -> &'static str {
    match s {
        AnswerStatus::Pending => "waiting…",
        AnswerStatus::Streaming => "streaming…",
        AnswerStatus::Done => "done",
        AnswerStatus::Error => "error",
        AnswerStatus::Cancelled => "cancelled",
    }
}

/// Divider text without the leading `─ ` (F-ASK-05): `answer · <agent> · <status>`.
pub fn answer_head(a: &Answer) -> String {
    let agent = a.agent.as_deref().filter(|n| !n.is_empty()).unwrap_or("agent");
    format!("answer · {agent} · {}", status_label(a.status))
}

/// All turns flattened to display lines (`threadBody`): turn 1 message (with code blocks), answers, follow-ups.
pub fn thread_body(c: &Comment, width: usize) -> Vec<BodyLine> {
    let mut out = Vec::new();
    let mut blk = 0usize;
    let fallback = [comments::Turn { message: c.message.clone(), answer: None, prior: Vec::new() }];
    let turns: &[comments::Turn] = if c.turns.is_empty() { &fallback } else { &c.turns };
    for (i, tu) in turns.iter().enumerate() {
        if i == 0 {
            out.extend(rich_lines(&c.message, width, BodyKind::Msg, &mut blk));
        } else {
            for l in wrap_text(&format!("follow-up: {}", tu.message), width) {
                out.push(plain_line(l, BodyKind::Fu));
            }
        }
        for a in tu.prior.iter().chain(tu.answer.iter()) {
            let live = comments::is_live(a);
            let err = a.status == AnswerStatus::Error;
            let bare = a.text.is_empty() && live;
            let body: String = if err {
                if a.text.is_empty() { "failed".to_string() } else { a.text.clone() }
            } else if bare {
                let t = if a.status == AnswerStatus::Pending {
                    "waiting for agent…"
                } else {
                    "agent working…"
                };
                t.to_string()
            } else {
                a.text.clone()
            };
            out.push(BodyLine { err, ..plain_line(answer_head(a), BodyKind::Div) });
            if body.is_empty() {
                continue;
            }
            if err || bare {
                for l in wrap_text(&body, width) {
                    out.push(BodyLine {
                        err,
                        live: bare.then_some(a.status),
                        ..plain_line(l, BodyKind::Ans)
                    });
                }
            } else {
                out.extend(rich_lines(&body, width, BodyKind::Ans, &mut blk));
            }
        }
    }
    out
}

/// Visible part of a thread body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BodyWin {
    pub start: usize,
    pub end: usize,
    /// Hidden lines behind `… +N more` (unfocused only).
    pub more: usize,
    pub total: usize,
    /// 1-based first/last shown line.
    pub from: usize,
    pub to: usize,
    pub max_off: usize,
    pub off: usize,
}

/// Unfocused: first `BODY_CAP` lines and a `more` count; focused: window of `v` lines at `off` (clamped).
pub fn window_body(total: usize, focused: bool, v: usize, off: usize) -> BodyWin {
    if !focused {
        let n = total.min(BODY_CAP);
        return BodyWin { start: 0, end: n, more: total - n, total, from: 1, to: n, max_off: 0, off: 0 };
    }
    let vv = v.max(1);
    let max_off = total.saturating_sub(vv);
    let o = off.min(max_off);
    let end = total.min(o + vv);
    BodyWin { start: o, end, more: 0, total, from: o + 1, to: end, max_off, off: o }
}

// ---------------------------------------------------------------- geometry shared with thread.rs

fn quoted_rows(n: usize) -> usize {
    if n == 0 { 0 } else { n.min(SENT_MAX) + usize::from(n > SENT_MAX) }
}

/// Rows of a comment box without its body lines (`sentBase`): borders + head + quoted (+ hint when focused).
fn sent_base(quoted: usize, focused: bool) -> usize {
    3 + quoted_rows(quoted) + usize::from(focused)
}

fn unfocused_height(c: &Comment, width: usize) -> usize {
    let body = thread_body(c, width.saturating_sub(3).max(1));
    let w = window_body(body.len(), false, 0, 0);
    sent_base(comments::quoted_lines(c).len(), false) + (w.end - w.start) + usize::from(w.more > 0)
}

fn follow_up_open(state: &State) -> bool {
    matches!(&state.overlay, Overlay::Editor(e) if matches!(e.kind, EditorKind::FollowUp { .. }))
}

/// Body lines the focused box may show: what the viewport has left on its row.
fn focus_room(state: &State, c: &Comment, width: usize) -> usize {
    let h = crate::nav::body_height(state.size) as isize;
    let others: isize = comments::row_of(state, c)
        .map(|r| {
            comments::anchored_at(state, r)
                .iter()
                .filter(|x| x.id != c.id)
                .map(|x| unfocused_height(x, width) as isize)
                .sum()
        })
        .unwrap_or(0);
    let base = sent_base(comments::quoted_lines(c).len(), true) as isize;
    let fu = if follow_up_open(state) { ASK_H as isize } else { 0 };
    (h - 1 - base - others - fu).max(1) as usize
}

/// Window offset used for display: following a live thread pins the bottom.
fn display_off(state: &State, c: &Comment) -> usize {
    let sc = state.thread.scrolls.get(&c.id);
    let live = comments::latest_answer(c).is_some_and(comments::is_live);
    let follow = sc.is_none_or(|s| s.follow);
    if live && follow { usize::MAX } else { sc.map_or(0, |s| s.off) }
}

/// Geometry of the focused thread window (`info` in app.tsx).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadInfo {
    /// Window height.
    pub v: usize,
    pub off: usize,
    pub max_off: usize,
    pub total: usize,
    /// Copy buttons: `(body line index, raw code)`.
    pub btns: Vec<(usize, String)>,
}

/// Window geometry of `c` as if focused, at the current scroll state and size.
pub fn thread_info(state: &State, c: &Comment) -> ThreadInfo {
    let width = box_width(state);
    let body = thread_body(c, width.saturating_sub(3).max(1));
    let v = focus_room(state, c, width);
    let w = window_body(body.len(), true, v, display_off(state, c));
    let btns = body
        .iter()
        .enumerate()
        .filter(|(_, l)| l.kind == BodyKind::Btn)
        .map(|(i, l)| (i, l.code.clone().unwrap_or_default()))
        .collect();
    ThreadInfo { v, off: w.off, max_off: w.max_off, total: w.total, btns }
}

// ---------------------------------------------------------------- box drawing helpers

fn sp(text: impl Into<String>, tone: Tone) -> Span {
    Span { text: text.into(), tone, lang: None }
}

/// Cut spans to `inner` chars; too long: `inner-1` chars plus `…` (Ink `truncate`).
fn fit(spans: Vec<Span>, inner: usize) -> Vec<Span> {
    let total: usize = spans.iter().map(|s| s.text.chars().count()).sum();
    if total <= inner {
        return spans;
    }
    let mut keep = inner.saturating_sub(1);
    let mut out = Vec::new();
    for mut s in spans {
        if keep == 0 {
            break;
        }
        let n = s.text.chars().count();
        if n > keep {
            s.text = s.text.chars().take(keep).collect();
            keep = 0;
        } else {
            keep -= n;
        }
        out.push(s);
    }
    if inner > 0 {
        let tone = out.last().map_or(Tone::Normal, |s| s.tone);
        out.push(sp("…", tone));
    }
    out
}

struct Frame {
    tl: &'static str,
    h: &'static str,
    tr: &'static str,
    v: &'static str,
    bl: &'static str,
    br: &'static str,
}

const ROUND: Frame = Frame { tl: "╭", h: "─", tr: "╮", v: "│", bl: "╰", br: "╯" };
const HEAVY: Frame = Frame { tl: "┏", h: "━", tr: "┓", v: "┃", bl: "┗", br: "┛" };

fn frame_top(f: &Frame, width: usize) -> BoxLine {
    let mid = f.h.repeat(width.saturating_sub(2));
    BoxLine { spans: vec![sp(format!("{}{mid}{}", f.tl, f.tr), Tone::Border)] }
}

fn frame_bottom(f: &Frame, width: usize) -> BoxLine {
    let mid = f.h.repeat(width.saturating_sub(2));
    BoxLine { spans: vec![sp(format!("{}{mid}{}", f.bl, f.br), Tone::Border)] }
}

/// Content row between vertical borders, padded to the inner width.
fn frame_row(f: &Frame, width: usize, content: Vec<Span>) -> BoxLine {
    let inner = width.saturating_sub(2);
    let mut spans = vec![sp(f.v, Tone::Border)];
    let content = fit(content, inner);
    let used: usize = content.iter().map(|s| s.text.chars().count()).sum();
    spans.extend(content);
    if used < inner {
        spans.push(sp(" ".repeat(inner - used), Tone::Normal));
    }
    spans.push(sp(f.v, Tone::Border));
    BoxLine { spans }
}

/// Focused hint row of a comment box (F-COMMENT-04): first option fitting `width-3`.
pub fn sent_hint(width: usize, can_ask: bool, can_follow: bool, scroll: bool, codes: bool) -> String {
    let room = width.saturating_sub(3);
    let a = if can_follow {
        "  a follow up"
    } else if can_ask {
        "  a ask"
    } else {
        ""
    };
    let sc = if scroll { "  j/k scroll" } else { "" };
    let cb = if codes { "  ↑/↓ code" } else { "" };
    let opts = [
        format!("e edit  D delete{a}{sc}{cb}  esc back"),
        format!("e edit  D delete{a}  esc back"),
        format!("e edit  D delete{a}"),
        "e edit  D delete".to_string(),
        "e edit".to_string(),
    ];
    opts.into_iter().find(|h| h.chars().count() <= room).unwrap_or_default()
}

/// Hint row of the editor (F-COMMENT-01): `mode` (`true` = ask) only for new comments.
pub fn ask_hint(width: usize, mode: Option<bool>) -> String {
    let room = width.saturating_sub(3);
    let opts: Vec<String> = match mode {
        Some(ask) => vec![
            format!("[{}] enter send  tab save/ask  esc cancel", if ask { "ask" } else { "save" }),
            "enter send  tab save/ask  esc cancel".to_string(),
            "enter send  esc cancel".to_string(),
            "enter send".to_string(),
        ],
        None => vec!["enter send  esc cancel".to_string(), "enter send".to_string()],
    };
    match opts.iter().find(|h| h.chars().count() <= room) {
        Some(h) => h.clone(),
        None => opts.last().map(|h| h.chars().take(room).collect()).unwrap_or_default(),
    }
}

fn body_spans(state: &State, l: &BodyLine, picked: Option<usize>, inner: usize) -> Vec<Span> {
    match l.kind {
        BodyKind::Div => {
            // `─ <head> ` padded with `─` to the inner width
            let tone = if l.err { Tone::Error } else { Tone::Accent };
            let text = format!("─ {} ", l.text);
            let pad = inner.max(1).saturating_sub(text.chars().count());
            vec![sp(format!("{text}{}", "─".repeat(pad)), tone)]
        }
        BodyKind::Btn => {
            let sel = picked == Some(l.blk);
            let mut v = vec![
                sp(" ", Tone::Normal),
                sp(COPY_BTN, if sel { Tone::ButtonSelected } else { Tone::Button }),
            ];
            if let Some(lang) = &l.lang {
                v.push(sp(format!(" {lang}"), Tone::Dim));
            }
            if sel {
                v.push(sp("  enter copy  esc cancel", Tone::Dim));
            }
            v
        }
        BodyKind::Code => {
            let sel = picked == Some(l.blk);
            vec![
                sp(" ", Tone::Normal),
                sp("│ ", if sel { Tone::Accent } else { Tone::Dim }),
                Span { text: l.text.clone(), tone: Tone::Code, lang: l.lang.clone() },
            ]
        }
        _ => {
            let tone = if l.err {
                Tone::Error
            } else if l.kind == BodyKind::Fu {
                Tone::Accent
            } else {
                Tone::Normal
            };
            let mut v = vec![sp(" ", Tone::Normal)];
            if let Some(st) = l.live {
                let g = if st == AnswerStatus::Streaming {
                    SPINNER[state.spinner % SPINNER.len()]
                } else {
                    '⠿'
                };
                v.push(sp(g.to_string(), Tone::Accent));
                v.push(sp(" ", Tone::Normal));
            }
            v.push(sp(l.text.clone(), tone));
            v
        }
    }
}

/// Box of one comment at `width` (F-COMMENT-04, F-ASK-05..08, F-COMMENT-10). Uses thread scroll from
/// `state.thread` when focused.
pub fn comment_box(state: &State, comment: &Comment, width: usize, focused: bool) -> ThreadBox {
    let bw = width.saturating_sub(3).max(1);
    let body = thread_body(comment, bw);
    let quoted = comments::quoted_lines(comment);
    let win = if focused {
        window_body(body.len(), true, focus_room(state, comment, width), display_off(state, comment))
    } else {
        window_body(body.len(), false, 0, 0)
    };
    let btn_count = body.iter().filter(|l| l.kind == BodyKind::Btn).count();
    let picked = state
        .thread
        .picked_block
        .as_ref()
        .filter(|(id, i)| focused && *id == comment.id && *i < btn_count)
        .map(|(_, i)| *i);
    let agent = comments::is_agent_note(comment);
    let multi = comments::turn_count(comment) > 1;
    let an = comments::latest_answer(comment);
    let saved = an.is_none() && !agent && !multi;
    let can_follow =
        an.is_some_and(|a| a.status == AnswerStatus::Done) || (agent && comments::can_follow_up(comment));
    let scroll = focused && win.max_off > 0;

    let f = if focused { &HEAVY } else { &ROUND };
    let inner = width.saturating_sub(2);
    let mut lines = vec![frame_top(f, width)];
    let mut head = vec![sp(
        format!("{}{}", if focused { " ▸ sent  " } else { " sent  " }, comments::head(state, comment)),
        Tone::Accent,
    )];
    if saved {
        head.push(sp("  saved · not asked", Tone::Dim));
    }
    if scroll {
        head.push(sp(format!("  ↕ {}-{}/{}", win.from, win.to, win.total), Tone::Dim));
    }
    lines.push(frame_row(f, width, head));
    for q in quoted.iter().take(SENT_MAX) {
        lines.push(frame_row(f, width, vec![sp(format!(" > {q}"), Tone::Dim)]));
    }
    if quoted.len() > SENT_MAX {
        lines.push(frame_row(f, width, vec![sp(format!(" … +{} more", quoted.len() - SENT_MAX), Tone::Dim)]));
    }
    for l in &body[win.start..win.end] {
        lines.push(frame_row(f, width, body_spans(state, l, picked, inner)));
    }
    if win.more > 0 {
        lines.push(frame_row(f, width, vec![sp(format!(" … +{} more", win.more), Tone::Dim)]));
    }
    if focused {
        let hint = sent_hint(width, comments::can_ask(comment), can_follow, scroll, btn_count > 0);
        lines.push(frame_row(f, width, vec![sp(format!(" {hint}"), Tone::Dim)]));
    }
    lines.push(frame_bottom(f, width));
    ThreadBox { id: comment.id.clone(), focused, lines }
}

/// Head and quoted lines the editor shows above the input (`AskSel`).
fn editor_sel(state: &State, editor: &EditorState) -> Option<(String, Vec<String>)> {
    match &editor.kind {
        EditorKind::New => {
            let sel = crate::nav::visual::selection(state)?;
            let text = crate::nav::visual::selection_text(state, &sel);
            let head = format!("selection {}", crate::nav::visual::sel_tag(state, &sel));
            Some((head, text.split('\n').map(str::to_string).collect()))
        }
        EditorKind::Edit { id } => {
            let c = comments::find(state, id)?;
            Some((format!("edit {}", comments::head(state, c)), comments::quoted_lines(c)))
        }
        EditorKind::FollowUp { .. } => Some(("follow-up".to_string(), Vec::new())),
    }
}

/// Rows the editor's selection preview adds above the input (`askExtra`).
fn ask_extra(n: usize) -> usize {
    1 + n.min(ASK_MAX) + usize::from(n > ASK_MAX)
}

/// Editor box for the open editor (F-COMMENT-01/02 render: prompt, text, caret, hint line).
pub fn editor_box(state: &State, editor: &EditorState, width: usize) -> ThreadBox {
    let f = &ROUND;
    let mut lines = vec![frame_top(f, width)];
    if let Some((head, quoted)) = editor_sel(state, editor) {
        lines.push(frame_row(f, width, vec![sp(format!(" {head}"), Tone::Accent)]));
        for q in quoted.iter().take(ASK_MAX) {
            lines.push(frame_row(f, width, vec![sp(format!(" > {q}"), Tone::Dim)]));
        }
        if quoted.len() > ASK_MAX {
            lines.push(frame_row(
                f,
                width,
                vec![sp(format!(" … +{} more", quoted.len() - ASK_MAX), Tone::Dim)],
            ));
        }
    }
    let inner = width.saturating_sub(4).max(4);
    let chars: Vec<char> = editor.text.chars().collect();
    let start = (editor.caret + 1).saturating_sub(inner);
    let shown: Vec<char> = chars.iter().skip(start).take(inner).copied().collect();
    let p = editor.caret.saturating_sub(start).min(shown.len());
    let before: String = shown[..p].iter().collect();
    let at: String = shown.get(p).map_or(" ".to_string(), |c| c.to_string());
    let after: String = shown.iter().skip(p + 1).collect();
    lines.push(frame_row(
        f,
        width,
        vec![sp(format!(" {before}"), Tone::Normal), sp(at, Tone::Caret), sp(after, Tone::Normal)],
    ));
    let mode = matches!(editor.kind, EditorKind::New).then(|| crate::editor::effective_ask(state, editor));
    lines.push(frame_row(f, width, vec![sp(format!(" {}", ask_hint(width, mode)), Tone::Dim)]));
    lines.push(frame_bottom(f, width));
    ThreadBox { id: String::new(), focused: false, lines }
}

/// Height of the editor box (`askH`).
fn editor_height(state: &State, editor: &EditorState) -> usize {
    ASK_H + editor_sel(state, editor).map_or(0, |(_, q)| ask_extra(q.len()))
}

/// Whether the open editor is drawn under `row` (the cursor row).
fn editor_here(state: &State, row: usize) -> Option<&EditorState> {
    match &state.overlay {
        Overlay::Editor(ed) if row < state.rows.rows.len() && crate::nav::cursor_row(state) == row => {
            Some(ed)
        }
        _ => None,
    }
}

/// All boxes drawn under row `row` of the current view: comment boxes in creation order, then the editor
/// box when it is anchored here (the cursor row). Width = `max(10, cols-1)`.
pub fn boxes_at(state: &State, row: usize) -> Vec<ThreadBox> {
    let width = box_width(state);
    let mut out: Vec<ThreadBox> = comments::anchored_at(state, row)
        .into_iter()
        .map(|c| comment_box(state, c, width, state.nav.focused_comment.as_deref() == Some(c.id.as_str())))
        .collect();
    if let Some(ed) = editor_here(state, row) {
        out.push(editor_box(state, ed, width));
    }
    out
}

/// Sum of box line counts under row `row` (0 when none). Used by `nav::viewport` (F-NAV-09).
pub fn row_extra_height(state: &State, row: usize) -> usize {
    let width = box_width(state);
    let mut n: usize = comments::anchored_at(state, row)
        .into_iter()
        .map(|c| {
            if state.nav.focused_comment.as_deref() == Some(c.id.as_str()) {
                comment_box(state, c, width, true).lines.len()
            } else {
                unfocused_height(c, width)
            }
        })
        .sum();
    if let Some(ed) = editor_here(state, row) {
        n += editor_height(state, ed);
    }
    n
}

/// Number of copy-button code blocks in a comment's thread (F-ASK-08).
pub fn block_count(state: &State, comment: &Comment) -> usize {
    let bw = box_width(state).saturating_sub(3).max(1);
    thread_body(comment, bw).iter().filter(|l| l.kind == BodyKind::Btn).count()
}

/// Text of code block `index` in the thread of `comment` (copied by the button).
pub fn block_text(comment: &Comment, index: usize) -> Option<String> {
    thread_body(comment, 80)
        .into_iter()
        .find(|l| l.kind == BodyKind::Btn && l.blk == index)
        .and_then(|l| l.code)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::comments::testutil::{answer, comment, ctx, state_with};
    use crate::comments::{Origin, Turn};
    use crate::state::EditorState;

    fn text(l: &BoxLine) -> String {
        l.spans.iter().map(|s| s.text.as_str()).collect()
    }

    fn texts(b: &ThreadBox) -> Vec<String> {
        b.lines.iter().map(text).collect()
    }

    fn st() -> State {
        state_with((1..=5).map(ctx).collect())
    }

    #[test]
    fn f_ask_05_wrap_text_golden() {
        assert_eq!(wrap_text("hello world foo", 11), ["hello world", "foo"]);
        assert_eq!(wrap_text("abcdefghij", 4), ["abcd", "efgh", "ij"]);
        assert_eq!(wrap_text("a\n\n\nb\n\n", 5), ["a", "", "", "b"]);
        assert_eq!(wrap_text("a\tb", 10), ["a  b"]);
        assert_eq!(wrap_text("aaaa bbbb", 4), ["aaaa", "bbbb"]);
        assert_eq!(wrap_text("x\r\ny", 10), ["x", "y"]);
        assert_eq!(wrap_text("aaa   bbb", 4), ["aaa", "bbb"], "trailing/leading spaces trimmed at the break");
        assert_eq!(wrap_text("", 5), [""]);
        assert_eq!(wrap_text("\n\n", 5), [""]);
        assert_eq!(wrap_text("ab cd", 0), ["a", "b", "c", "d"], "width clamps to 1");
    }

    #[test]
    fn f_ask_06_code_blocks_button_and_gutter_lines() {
        let mut n = 0;
        let l = rich_lines("intro\n```ts title\nlet a = 1;\n\n\tx\n```\nafter", 30, BodyKind::Msg, &mut n);
        let t: Vec<_> = l.iter().map(|x| (x.kind, x.text.as_str())).collect();
        assert_eq!(
            t,
            [
                (BodyKind::Msg, "intro"),
                (BodyKind::Btn, "[ copy ] ts"),
                (BodyKind::Code, "let a = 1;"),
                (BodyKind::Code, ""),
                (BodyKind::Code, "  x"),
                (BodyKind::Msg, "after"),
            ]
        );
        assert_eq!(l[1].code.as_deref(), Some("let a = 1;\n\n\tx"), "raw text, tabs kept");
        assert_eq!(l[2].lang.as_deref(), Some("ts"));
        assert_eq!(n, 1);
    }

    #[test]
    fn f_ask_06_fence_rules_close_length_tilde_and_unclosed() {
        let mut n = 0;
        let l = rich_lines("````\na\n```\nb\n````\nz", 30, BodyKind::Ans, &mut n);
        let code: Vec<_> = l.iter().filter(|x| x.kind == BodyKind::Code).map(|x| x.text.as_str()).collect();
        assert_eq!(code, ["a", "```", "b"], "shorter fence does not close");
        let mut n = 0;
        let l = rich_lines("~~~\nx\n~~~ \ny", 30, BodyKind::Ans, &mut n);
        assert_eq!(l.iter().filter(|x| x.kind == BodyKind::Btn).count(), 1);
        assert_eq!(l.last().map(|x| x.text.as_str()), Some("y"));
        let mut n = 0;
        let l = rich_lines("```\nopen\nforever", 30, BodyKind::Ans, &mut n);
        assert_eq!(l.iter().filter(|x| x.kind == BodyKind::Code).count(), 2, "no close: to the end");
        let mut n = 0;
        let l = rich_lines("``not a fence", 30, BodyKind::Ans, &mut n);
        assert!(l.iter().all(|x| x.kind == BodyKind::Ans));
    }

    #[test]
    fn f_ask_06_long_code_line_split_not_wrapped() {
        let mut n = 0;
        let l = rich_lines("```\n0123456789abc def\n```", 10, BodyKind::Msg, &mut n);
        let code: Vec<_> = l.iter().filter(|x| x.kind == BodyKind::Code).map(|x| x.text.as_str()).collect();
        assert_eq!(code, ["01234567", "89abc de", "f"], "width-2 chars per line");
    }

    #[test]
    fn f_ask_06_blocks_numbered_across_thread() {
        let mut c = comment("q1", 1, 1, "```\na\n```");
        c.turns[0].answer = Some(answer(AnswerStatus::Done, "```\nb\n```"));
        let body = thread_body(&c, 40);
        let blks: Vec<_> = body.iter().filter(|l| l.kind == BodyKind::Btn).map(|l| l.blk).collect();
        assert_eq!(blks, [0, 1]);
        assert_eq!(block_text(&c, 1).as_deref(), Some("b"));
        assert_eq!(block_text(&c, 2), None);
        assert_eq!(block_count(&st(), &c), 2);
    }

    #[test]
    fn f_ask_05_thread_body_turns_dividers_and_live_lines() {
        let mut c = comment("q1", 1, 1, "question");
        c.turns[0].prior.push(answer(AnswerStatus::Done, "first"));
        c.turns[0].answer =
            Some(Answer { agent: Some("bot".into()), ..answer(AnswerStatus::Done, "second") });
        c.turns.push(Turn {
            message: "and more".into(),
            answer: Some(answer(AnswerStatus::Pending, "")),
            prior: vec![],
        });
        c.turns.push(Turn {
            message: "again".into(),
            answer: Some(answer(AnswerStatus::Streaming, "")),
            prior: vec![],
        });
        let t: Vec<_> = thread_body(&c, 40).into_iter().map(|l| (l.kind, l.text, l.live)).collect();
        assert_eq!(
            t,
            [
                (BodyKind::Msg, "question".to_string(), None),
                (BodyKind::Div, "answer · agent · done".to_string(), None),
                (BodyKind::Ans, "first".to_string(), None),
                (BodyKind::Div, "answer · bot · done".to_string(), None),
                (BodyKind::Ans, "second".to_string(), None),
                (BodyKind::Fu, "follow-up: and more".to_string(), None),
                (BodyKind::Div, "answer · agent · waiting…".to_string(), None),
                (BodyKind::Ans, "waiting for agent…".to_string(), Some(AnswerStatus::Pending)),
                (BodyKind::Fu, "follow-up: again".to_string(), None),
                (BodyKind::Div, "answer · agent · streaming…".to_string(), None),
                (BodyKind::Ans, "agent working…".to_string(), Some(AnswerStatus::Streaming)),
            ]
        );
    }

    #[test]
    fn f_ask_05_follow_up_lines_never_code_blocks() {
        let mut c = comment("q1", 1, 1, "m");
        c.turns.push(Turn { message: "```\nx\n```".into(), answer: None, prior: vec![] });
        assert!(thread_body(&c, 40).iter().all(|l| l.kind != BodyKind::Btn));
    }

    #[test]
    fn f_ask_07_window_body_unfocused_cap_and_focused_clamp() {
        let w = window_body(20, false, 0, 0);
        assert_eq!((w.end, w.more, w.from, w.to), (BODY_CAP, 6, 1, 14));
        let w = window_body(5, false, 0, 0);
        assert_eq!((w.end, w.more), (5, 0));
        let w = window_body(30, true, 10, 5);
        assert_eq!((w.start, w.end, w.max_off, w.from, w.to), (5, 15, 20, 6, 15));
        let w = window_body(30, true, 10, usize::MAX);
        assert_eq!((w.off, w.to), (20, 30));
        let w = window_body(3, true, 10, 4);
        assert_eq!((w.off, w.max_off, w.end), (0, 0, 3));
        assert_eq!(window_body(3, true, 0, 0).end, 1, "window at least 1");
    }

    #[test]
    fn f_comment_04_unfocused_box_layout_and_widths() {
        let s = st();
        let c = comment("q1", 1, 2, "hello");
        let b = comment_box(&s, &c, 40, false);
        let t = texts(&b);
        assert_eq!(t.len(), 4);
        assert_eq!(t[0], format!("╭{}╮", "─".repeat(38)));
        assert_eq!(t[1], format!("│ sent  line L2  saved · not asked{}│", " ".repeat(38 - 33)));
        assert_eq!(t[2], format!("│ hello{}│", " ".repeat(38 - 6)));
        assert_eq!(t[3], format!("╰{}╯", "─".repeat(38)));
        assert!(t.iter().all(|l| l.chars().count() == 40));
        assert!(!b.focused && b.id == "q1");
    }

    #[test]
    fn f_comment_04_answered_box_has_no_saved_suffix_and_divider_pads() {
        let s = st();
        let mut c = comment("q1", 1, 2, "hello");
        c.turns[0].answer = Some(answer(AnswerStatus::Done, "fine"));
        let t = texts(&comment_box(&s, &c, 40, false));
        assert!(!t[1].contains("saved"));
        assert_eq!(t[3], format!("│─ answer · agent · done {}│", "─".repeat(38 - 24)));
        assert_eq!(t[4], format!("│ fine{}│", " ".repeat(38 - 5)));
    }

    #[test]
    fn f_comment_04_unfocused_body_capped_with_more_row() {
        let s = st();
        let msg = (1..=20).map(|i| format!("m{i}")).collect::<Vec<_>>().join("\n");
        let c = comment("q1", 1, 2, &msg);
        let t = texts(&comment_box(&s, &c, 40, false));
        assert_eq!(t.len(), 2 + 1 + 14 + 1);
        assert!(t[t.len() - 2].starts_with("│ … +6 more"));
    }

    #[test]
    fn f_comment_04_quoted_selection_lines_max_three() {
        let s = st();
        let mut c = comment("q1", 1, 2, "m");
        c.selection = Some(comments::SelectionInfo {
            start_line: Some(1),
            end_line: Some(5),
            start_col: 1,
            end_col: 2,
        });
        c.text = "a\nb\nc\nd\ne".into();
        let t = texts(&comment_box(&s, &c, 40, false));
        assert!(t[2].starts_with("│ > a") && t[4].starts_with("│ > c"));
        assert!(t[5].starts_with("│ … +2 more"));
        assert_eq!(t.len(), 2 + 1 + 4 + 1);
    }

    #[test]
    fn f_comment_04_focused_box_heavy_border_head_and_hint() {
        let mut s = st();
        s.nav.focused_comment = Some("q1".into());
        let c = comment("q1", 1, 2, "hello");
        let b = comment_box(&s, &c, 60, true);
        let t = texts(&b);
        assert!(t[0].starts_with('┏') && t[0].ends_with('┓'));
        assert!(t[1].starts_with("┃ ▸ sent  line L2"));
        assert_eq!(t[1].chars().count(), 60);
        assert!(t[t.len() - 2].contains("e edit  D delete  a ask  esc back"));
        assert!(t.last().is_some_and(|l| l.starts_with('┗')));
        assert_eq!(b.lines[1].spans[1].tone, Tone::Accent);
    }

    #[test]
    fn f_comment_04_hint_options_by_width() {
        assert_eq!(sent_hint(80, false, false, false, false), "e edit  D delete  esc back");
        assert_eq!(
            sent_hint(80, true, false, true, true),
            "e edit  D delete  a ask  j/k scroll  ↑/↓ code  esc back"
        );
        assert_eq!(sent_hint(80, false, true, false, false), "e edit  D delete  a follow up  esc back");
        assert_eq!(sent_hint(28, false, false, false, false), "e edit  D delete");
        assert_eq!(sent_hint(15, false, false, false, false), "e edit");
        assert_eq!(sent_hint(8, false, false, false, false), "");
    }

    #[test]
    fn f_ask_07_focused_window_scroll_suffix_and_pos() {
        let mut s = st();
        s.size = crate::screen::Size { cols: 80, rows: 12 };
        s.nav.focused_comment = Some("q1".into());
        let mut c = comment("q1", 1, 1, "m");
        c.turns[0].answer = Some(answer(
            AnswerStatus::Done,
            &(1..=30).map(|i| format!("l{i}")).collect::<Vec<_>>().join("\n"),
        ));
        s.comments.push(c.clone());
        let info = thread_info(&s, &c);
        assert_eq!(info.total, 32);
        assert!(info.max_off > 0);
        let t = texts(&comment_box(&s, &c, 79, true));
        assert!(t[1].contains(&format!("↕ 1-{}/32", info.v)));
        assert!(t[t.len() - 2].contains("j/k scroll"));
        assert_eq!(t.len(), info.v + 4);
    }

    #[test]
    fn f_ask_05_spinner_and_waiting_lines() {
        let mut s = st();
        let mut c = comment("q1", 1, 1, "m");
        c.turns[0].answer = Some(answer(AnswerStatus::Pending, ""));
        let t = texts(&comment_box(&s, &c, 40, false));
        assert!(t[4].starts_with("│ ⠿ waiting for agent…"), "{t:?}");
        c.turns[0].answer = Some(answer(AnswerStatus::Streaming, ""));
        s.spinner = 3;
        let t = texts(&comment_box(&s, &c, 40, false));
        assert!(t[4].starts_with("│ ⠸ agent working…"), "{t:?}");
        assert!(t[3].contains("streaming…"));
    }

    #[test]
    fn f_comment_10_agent_note_box_never_saved_suffix() {
        let s = st();
        let mut c = comment("q1", 1, 1, "look here");
        c.origin = Origin::Agent;
        c.number = Some(2);
        let t = texts(&comment_box(&s, &c, 50, false));
        assert!(t[1].starts_with("│ sent  #2 agent note L1"));
        assert!(!t[1].contains("saved"));
    }

    #[test]
    fn f_ask_08_selected_button_and_gutter_tones() {
        let mut s = st();
        s.nav.focused_comment = Some("q1".into());
        s.thread.picked_block = Some(("q1".into(), 0));
        let c = comment("q1", 1, 1, "```rs\nfn x()\n```");
        let b = comment_box(&s, &c, 60, true);
        let btn = &b.lines[2];
        assert!(text(btn).contains("[ copy ] rs  enter copy  esc cancel"));
        assert!(btn.spans.iter().any(|x| x.tone == Tone::ButtonSelected));
        let code = &b.lines[3];
        assert!(code.spans.iter().any(|x| x.text == "│ " && x.tone == Tone::Accent));
        let sp = code.spans.iter().find(|x| x.tone == Tone::Code).unwrap();
        assert_eq!((sp.text.as_str(), sp.lang.as_deref()), ("fn x()", Some("rs")));
        s.thread.picked_block = None;
        let b = comment_box(&s, &c, 60, true);
        assert!(!text(&b.lines[2]).contains("enter copy"));
    }

    #[test]
    fn f_comment_01_editor_box_layout() {
        let mut s = st();
        let ed = EditorState {
            kind: EditorKind::New,
            text: "abc".into(),
            caret: 1,
            ask_mode: false,
            ext: Default::default(),
        };
        s.overlay = Overlay::Editor(ed.clone());
        let b = editor_box(&s, &ed, 60);
        let t = texts(&b);
        assert_eq!(t.len(), 4);
        assert_eq!(t[1], format!("│ abc{}│", " ".repeat(58 - 4)));
        assert_eq!(b.lines[1].spans[2].text, "b");
        assert_eq!(b.lines[1].spans[2].tone, Tone::Caret);
        assert!(t[2].starts_with("│ [save] enter send  tab save/ask  esc cancel"));
        assert!(b.id.is_empty());
    }

    #[test]
    fn f_comment_01_editor_caret_at_end_is_space_and_long_text_windows() {
        let s = st();
        let ed = EditorState {
            kind: EditorKind::New,
            text: "x".repeat(100),
            caret: 100,
            ask_mode: false,
            ext: Default::default(),
        };
        let b = editor_box(&s, &ed, 30);
        let input = &b.lines[1];
        assert_eq!(
            input.spans[1].text.chars().count(),
            26,
            "leading space + width-4 chars, start = caret-(width-4)+1"
        );
        assert_eq!(input.spans[2].text, " ");
        assert_eq!(text(input).chars().count(), 30);
    }

    #[test]
    fn f_comment_01_editor_hint_fallbacks() {
        assert_eq!(ask_hint(80, Some(true)), "[ask] enter send  tab save/ask  esc cancel");
        assert_eq!(ask_hint(40, Some(true)), "enter send  tab save/ask  esc cancel");
        assert_eq!(ask_hint(30, Some(false)), "enter send  esc cancel");
        assert_eq!(ask_hint(15, Some(false)), "enter send");
        assert_eq!(ask_hint(80, None), "enter send  esc cancel");
        assert_eq!(ask_hint(12, None), "enter sen");
        assert_eq!(ask_hint(8, None), "enter");
    }

    #[test]
    fn f_comment_01_editor_heads_edit_and_follow_up() {
        let mut s = st();
        s.comments.push(comment("q1", 1, 2, "orig"));
        let fu = EditorState {
            kind: EditorKind::FollowUp { id: "q1".into() },
            text: String::new(),
            caret: 0,
            ask_mode: false,
            ext: Default::default(),
        };
        let t = texts(&editor_box(&s, &fu, 60));
        assert_eq!(t.len(), 5);
        assert!(t[1].starts_with("│ follow-up"));
        assert!(t[3].contains("enter send  esc cancel") && !t[3].contains("tab"));
        let ed = EditorState { kind: EditorKind::Edit { id: "q1".into() }, ..fu };
        let t = texts(&editor_box(&s, &ed, 60));
        assert!(t[1].starts_with("│ edit line L2"));
    }

    #[test]
    fn f_comment_01_editor_selection_preview_quoted_max_five() {
        let mut s = state_with((1..=9).map(ctx).collect());
        s.nav.row = 7;
        s.nav.selection = Some(crate::state::Selection {
            kind: crate::state::SelectionKind::Line,
            anchor_row: 0,
            anchor_col: 0,
        });
        let ed = EditorState {
            kind: EditorKind::New,
            text: String::new(),
            caret: 0,
            ask_mode: false,
            ext: Default::default(),
        };
        let t = texts(&editor_box(&s, &ed, 60));
        assert_eq!(t.len(), 4 + 1 + 5 + 1);
        assert!(t[1].starts_with("│ selection L1-8"));
        assert!(t[2].starts_with("│ > l1") && t[7].contains("… +3 more"));
    }

    #[test]
    fn f_nav_09_row_extra_height_and_boxes_at() {
        let mut s = st();
        assert_eq!(row_extra_height(&s, 1), 0);
        assert!(boxes_at(&s, 1).is_empty());
        s.comments.push(comment("q1", 1, 2, "a"));
        s.comments.push(comment("q2", 2, 2, "b\nc"));
        assert_eq!(row_extra_height(&s, 1), 4 + 5);
        let ids: Vec<_> = boxes_at(&s, 1).into_iter().map(|b| b.id).collect();
        assert_eq!(ids, ["q1", "q2"]);
        s.nav.row = 1;
        s.overlay = Overlay::Editor(EditorState {
            kind: EditorKind::New,
            text: String::new(),
            caret: 0,
            ask_mode: false,
            ext: Default::default(),
        });
        assert_eq!(row_extra_height(&s, 1), 4 + 5 + 4);
        let b = boxes_at(&s, 1);
        assert_eq!(b.len(), 3);
        assert!(b[2].id.is_empty(), "editor last");
        assert_eq!(row_extra_height(&s, 0), 0);
    }

    #[test]
    fn f_nav_09_focused_height_matches_box_lines() {
        let mut s = st();
        s.comments.push(comment("q1", 1, 2, "a"));
        s.nav.focused_comment = Some("q1".into());
        let n = boxes_at(&s, 1)[0].lines.len();
        assert_eq!(row_extra_height(&s, 1), n);
        assert_eq!(n, 5, "borders + head + body + hint");
    }

    #[test]
    fn f_ask_07_window_room_shrinks_by_other_boxes_and_follow_up_editor() {
        let mut s = st();
        s.size = crate::screen::Size { cols: 80, rows: 24 };
        let big = (1..=40).map(|i| format!("m{i}")).collect::<Vec<_>>().join("\n");
        s.comments.push(comment("q1", 1, 2, &big));
        s.comments.push(comment("q2", 2, 2, "small"));
        let c = s.comments[0].clone();
        let h = crate::nav::body_height(s.size);
        let base = thread_info(&s, &c).v;
        assert_eq!(base, h - 1 - 4 - 4, "other box height 4, own base 4");
        s.overlay = Overlay::Editor(EditorState {
            kind: EditorKind::FollowUp { id: "q1".into() },
            text: String::new(),
            caret: 0,
            ask_mode: false,
            ext: Default::default(),
        });
        assert_eq!(thread_info(&s, &c).v, base - ASK_H);
    }

    #[test]
    fn f_ask_07_live_answer_follows_bottom_unless_scrolled_up() {
        let mut s = st();
        s.size = crate::screen::Size { cols: 80, rows: 12 };
        let mut c = comment("q1", 1, 2, &(1..=40).map(|i| format!("m{i}")).collect::<Vec<_>>().join("\n"));
        c.turns[0].answer = Some(answer(AnswerStatus::Streaming, ""));
        s.comments.push(c.clone());
        let i = thread_info(&s, &c);
        assert_eq!(i.off, i.max_off, "follow initially on");
        s.thread.scrolls.insert("q1".into(), crate::thread::ThreadScroll { off: 2, follow: false });
        assert_eq!(thread_info(&s, &c).off, 2);
    }
}
