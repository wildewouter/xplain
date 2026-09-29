//! Box drawing: frames, hints, comment and editor boxes as styled lines (F-COMMENT-01/04, F-COMMENT-10).

use super::body::thread_body;
use super::text::{BodyKind, BodyLine};
use super::window::{display_off, focus_room, focused_height, unfocused_height, window_body};
use super::{ASK_H, ASK_MAX, BoxLine, COPY_BTN, SENT_MAX, SPINNER, Span, ThreadBox, Tone, box_width};
use crate::comments::{self, AnswerStatus, Comment};
use crate::state::{EditorKind, EditorState, Overlay, State};

pub(super) fn sp(text: impl Into<String>, tone: Tone) -> Span {
    Span { text: text.into(), tone, lang: None }
}

/// Cut spans to `inner` chars; too long: `inner-1` chars plus `…` (Ink `truncate`).
pub(super) fn fit(spans: Vec<Span>, inner: usize) -> Vec<Span> {
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

pub(super) struct Frame {
    tl: &'static str,
    h: &'static str,
    tr: &'static str,
    v: &'static str,
    bl: &'static str,
    br: &'static str,
}

pub(super) const ROUND: Frame = Frame { tl: "╭", h: "─", tr: "╮", v: "│", bl: "╰", br: "╯" };
pub(super) const HEAVY: Frame = Frame { tl: "┏", h: "━", tr: "┓", v: "┃", bl: "┗", br: "┛" };

pub(super) fn frame_top(f: &Frame, width: usize) -> BoxLine {
    let mid = f.h.repeat(width.saturating_sub(2));
    BoxLine { spans: vec![sp(format!("{}{mid}{}", f.tl, f.tr), Tone::Border)] }
}

pub(super) fn frame_bottom(f: &Frame, width: usize) -> BoxLine {
    let mid = f.h.repeat(width.saturating_sub(2));
    BoxLine { spans: vec![sp(format!("{}{mid}{}", f.bl, f.br), Tone::Border)] }
}

/// Content row between vertical borders, padded to the inner width.
pub(super) fn frame_row(f: &Frame, width: usize, content: Vec<Span>) -> BoxLine {
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

pub(super) fn body_spans(state: &State, l: &BodyLine, picked: Option<usize>, inner: usize) -> Vec<Span> {
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
pub(super) fn editor_sel(state: &State, editor: &EditorState) -> Option<(String, Vec<String>)> {
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
pub(super) fn ask_extra(n: usize) -> usize {
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
pub(super) fn editor_height(state: &State, editor: &EditorState) -> usize {
    ASK_H + editor_sel(state, editor).map_or(0, |(_, q)| ask_extra(q.len()))
}

/// Whether the open editor is drawn under `row` (the cursor row).
pub(super) fn editor_here(state: &State, row: usize) -> Option<&EditorState> {
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
                focused_height(state, c, width)
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
    use crate::comments::AnswerStatus;
    use crate::comments::Origin;
    use crate::comments::testutil::{answer, comment, ctx, state_with};
    use crate::screen::Size;
    use crate::state::EditorState;
    use crate::thread_layout::fixtures::*;
    use crate::thread_layout::*;

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
        let ed = EditorState { kind: EditorKind::New, text: "abc".into(), caret: 1, ask_mode: false };
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
        let ed = EditorState { kind: EditorKind::New, text: "x".repeat(100), caret: 100, ask_mode: false };
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
        let ed = EditorState { kind: EditorKind::New, text: String::new(), caret: 0, ask_mode: false };
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
        });
        assert_eq!(row_extra_height(&s, 1), 4 + 5 + 4);
        let b = boxes_at(&s, 1);
        assert_eq!(b.len(), 3);
        assert!(b[2].id.is_empty(), "editor last");
        assert_eq!(row_extra_height(&s, 0), 0);
    }

    /// Arithmetic heights (`row_extra_height` inputs) must equal the line counts of the boxes actually drawn.
    #[test]
    fn f_nav_09_height_fns_equal_box_line_counts() {
        let bodies = [
            "a".to_string(),
            "a\nb\nc".to_string(),
            (1..=40).map(|i| format!("m{i}")).collect::<Vec<_>>().join("\n"),
            "x\n```rs\nlet a = 1;\nlet b = 2;\n```\ny".to_string(),
            "word ".repeat(60),
        ];
        for size in [Size { cols: 80, rows: 24 }, Size { cols: 30, rows: 10 }, Size { cols: 120, rows: 50 }] {
            for body in &bodies {
                for quoted_to in [1u32, 2, 6] {
                    let mut s = st();
                    s.size = size;
                    s.comments.push(comment("q1", 1, quoted_to, body));
                    s.comments.push(comment("q2", 1, 1, "other\nbox"));
                    let width = box_width(&s);
                    let c = s.comments[0].clone();
                    assert_eq!(
                        focused_height(&s, &c, width),
                        comment_box(&s, &c, width, true).lines.len(),
                        "focused {size:?} {quoted_to}"
                    );
                    assert_eq!(
                        unfocused_height(&c, width),
                        comment_box(&s, &c, width, false).lines.len(),
                        "unfocused {size:?} {quoted_to}"
                    );
                    for kind in [
                        EditorKind::New,
                        EditorKind::Edit { id: "q1".into() },
                        EditorKind::FollowUp { id: "q1".into() },
                    ] {
                        let ed = EditorState { kind, text: "t".into(), caret: 1, ask_mode: false };
                        assert_eq!(
                            editor_height(&s, &ed),
                            editor_box(&s, &ed, width).lines.len(),
                            "editor {size:?}"
                        );
                    }
                }
            }
        }
    }
}
