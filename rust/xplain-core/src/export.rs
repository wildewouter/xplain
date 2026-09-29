//! Review export: markdown rendering, file name, `E` key.
//!
//! Spec: F-EXPORT-01 (`E`: file `xplain-review-<local timestamp>.md` in cwd, notes, `WriteExport` effect,
//! `export failed` note), F-EXPORT-02 (exact markdown format). Oracle: `src/ask/export.ts`, `exportReview`
//! in `src/app.tsx`. Owner: component `agent` (E). Must not: do IO (effect only).

use crate::comments::{Answer, AnswerStatus, Comment, Origin, PaneSide};
use crate::effect::{Effect, Fx};
use crate::errors::IoReason;
use crate::event::ReqId;
use crate::keys::{Key, KeyEvent};
use crate::state::{Now, Pending, State};

/// Inputs of the markdown header (F-EXPORT-02).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportMeta {
    pub cwd: String,
    pub mode: String,
    pub args: Vec<String>,
    pub now: Now,
}

/// Civil date from days since 1970-01-01 (proleptic Gregorian).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// `(year, month, day, hour, minute, second)` of a unix time in seconds.
fn ymdhms(secs: i64) -> (i64, u32, u32, u32, u32, u32) {
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    let t = secs.rem_euclid(86_400) as u32;
    (y, m, d, t / 3600, (t % 3600) / 60, t % 60)
}

/// `xplain-review-YYYYMMDD-HHMMSS.md` from local time (`unix_ms + utc_offset`) (F-EXPORT-01).
pub fn export_name(now: &Now) -> String {
    let local = now.unix_ms.div_euclid(1000) + i64::from(now.utc_offset_secs);
    let (y, mo, d, h, mi, s) = ymdhms(local);
    format!("xplain-review-{y:04}{mo:02}{d:02}-{h:02}{mi:02}{s:02}.md")
}

/// ISO 8601 UTC with milliseconds and `Z`.
fn iso_utc(unix_ms: i64) -> String {
    let (y, mo, d, h, mi, s) = ymdhms(unix_ms.div_euclid(1000));
    let ms = unix_ms.rem_euclid(1000);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}.{ms:03}Z")
}

/// Backtick fence longer than any backtick run in `s` (min 3).
fn fence(s: &str) -> String {
    let mut longest = 0usize;
    let mut run = 0usize;
    for c in s.chars() {
        if c == '`' {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    "`".repeat(3.max(longest + 1))
}

fn block(s: &str) -> String {
    let f = fence(s);
    format!("{f}\n{s}\n{f}")
}

fn quote(s: &str) -> String {
    s.split('\n')
        .map(|l| if l.is_empty() { ">".to_string() } else { format!("> {l}") })
        .collect::<Vec<_>>()
        .join("\n")
}

fn inline(s: &str) -> String {
    let n = fence(s).len() - 2;
    let f = "`".repeat(n);
    if s.starts_with('`') || s.ends_with('`') { format!("{f} {s} {f}") } else { format!("{f}{s}{f}") }
}

fn side(c: &Comment) -> &'static str {
    if c.side == PaneSide::Old { "old" } else { "new" }
}

fn status_word(s: AnswerStatus) -> &'static str {
    match s {
        AnswerStatus::Pending => "pending",
        AnswerStatus::Streaming => "streaming",
        AnswerStatus::Done => "done",
        AnswerStatus::Error => "error",
        AnswerStatus::Cancelled => "cancelled",
    }
}

fn state_of(c: &Comment) -> &'static str {
    match c.turns.last().and_then(|t| t.answer.as_ref()) {
        None => "saved",
        Some(a) if a.status == AnswerStatus::Done => "answered",
        Some(a) => status_word(a.status),
    }
}

fn where_of(c: &Comment) -> String {
    let s = side(c);
    if let Some(sel) = &c.selection {
        if let (Some(a), Some(b)) = (sel.start_line, sel.end_line) {
            let r = if a == b { format!("line {a}") } else { format!("lines {a}-{b}") };
            return format!("{s} side, selection {r}, cols {}-{}", sel.start_col, sel.end_col);
        }
    }
    match c.line {
        Some(l) => format!("{s} side, line {l}"),
        None => format!("{s} side, row {}", c.row + 1),
    }
}

fn answer_head(a: &Answer) -> String {
    let agent = a.agent.as_deref().filter(|x| !x.is_empty()).map(|x| format!(" ({x})")).unwrap_or_default();
    format!("Answer{agent} - {}", status_word(a.status))
}

fn render_comment(c: &Comment, n: usize) -> String {
    let mut out: Vec<String> = vec![format!("### {n}. {}", where_of(c)), String::new()];
    let origin = if c.origin == Origin::Agent { "agent" } else { "human" };
    out.push(format!("- origin: {origin}"));
    out.push(format!("- state: {}", state_of(c)));
    out.push(String::new());
    if !c.text.is_empty() {
        let has_sel = c.selection.as_ref().is_some_and(|s| s.start_line.is_some());
        out.push(if has_sel { "Selected text:" } else { "Line:" }.to_string());
        out.push(String::new());
        out.push(block(&c.text));
        out.push(String::new());
    }
    if !c.context.is_empty() {
        out.push("Context:".to_string());
        out.push(String::new());
        out.push(block(&c.context.join("\n")));
        out.push(String::new());
    }
    for (i, t) in c.turns.iter().enumerate() {
        out.push(match (i, c.origin == Origin::Agent) {
            (0, true) => "**Note:**".to_string(),
            (0, false) => "**Comment:**".to_string(),
            _ => format!("**Follow-up {i}:**"),
        });
        out.push(String::new());
        out.push(quote(&t.message));
        out.push(String::new());
        for a in t.prior.iter().chain(t.answer.iter()) {
            out.push(format!("**{}**", answer_head(a)));
            out.push(String::new());
            if !a.text.is_empty() {
                out.push(quote(&a.text));
                out.push(String::new());
            }
        }
    }
    out.join("\n")
}

fn sort_key(c: &Comment) -> i64 {
    c.selection.as_ref().and_then(|s| s.start_line).or(c.line).map_or(c.row as i64, i64::from)
}

/// Whole markdown document (F-EXPORT-02).
pub fn render_markdown(comments: &[Comment], meta: &ExportMeta) -> String {
    let mut files: Vec<&str> = Vec::new();
    for c in comments {
        if !files.contains(&c.file.as_str()) {
            files.push(&c.file);
        }
    }
    files.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    let diff = std::iter::once(meta.mode.as_str())
        .chain(meta.args.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" ");
    let mut out: Vec<String> = vec![
        "# xplain review".to_string(),
        String::new(),
        format!("- repo: {}", inline(&meta.cwd)),
        format!("- diff: {}", inline(&diff)),
        format!("- date: {}", iso_utc(meta.now.unix_ms)),
        format!("- comments: {}", comments.len()),
        String::new(),
    ];
    if comments.is_empty() {
        out.push("No comments.".to_string());
        out.push(String::new());
    }
    let mut n = 0;
    for file in files {
        out.push(format!("## {}", inline(file)));
        out.push(String::new());
        let mut list: Vec<&Comment> = comments.iter().filter(|c| c.file == file).collect();
        list.sort_by_key(|c| (sort_key(c), c.seq));
        for c in list {
            n += 1;
            out.push(render_comment(c, n));
            out.push(String::new());
        }
    }
    let joined = out.join("\n");
    let mut collapsed = String::with_capacity(joined.len());
    let mut newlines = 0;
    for ch in joined.chars() {
        if ch == '\n' {
            newlines += 1;
            if newlines > 2 {
                continue;
            }
        } else {
            newlines = 0;
        }
        collapsed.push(ch);
    }
    let mut s = collapsed.trim_end().to_string();
    s.push('\n');
    s
}

/// `E` in cursor context. True when consumed.
pub fn on_key(state: &mut State, key: KeyEvent, fx: &mut Fx) -> bool {
    if key.key != Key::Char('E') || key.mods.ctrl || key.mods.alt {
        return false;
    }
    if state.comments.is_empty() {
        state.note = Some("no comments to export".to_string());
        return true;
    }
    let cwd = state.env.abs_cwd.clone();
    let path = format!("{}/{}", cwd.trim_end_matches('/'), export_name(&state.clock));
    let meta = ExportMeta {
        cwd,
        mode: state.settings.mode.as_str().to_string(),
        args: state.options.git_args.clone(),
        now: state.clock,
    };
    let contents = render_markdown(&state.comments, &meta);
    let req = state.alloc_req();
    state.loader.pending.insert(req, Pending::Export { path: path.clone(), count: state.comments.len() });
    fx.push(Effect::WriteExport { req, path, contents });
    true
}

/// `Event::ExportWritten`: success/failed note (F-EXPORT-01).
pub fn on_written(state: &mut State, req: ReqId, result: Result<(), IoReason>) {
    let Some(Pending::Export { path, count }) = state.loader.pending.remove(&req) else { return };
    state.note = Some(match result {
        Ok(()) => format!("exported {count} comment{} -> {path}", if count == 1 { "" } else { "s" }),
        Err(reason) => crate::messages::export_failed(&path, reason),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comments::testutil::{answer, comment, state_with};
    use crate::comments::{SelectionInfo, Turn};

    fn meta() -> ExportMeta {
        ExportMeta {
            cwd: "/repo".into(),
            mode: "all".into(),
            args: vec![],
            now: Now { unix_ms: 1_700_000_000_123, utc_offset_secs: 3600 },
        }
    }

    #[test]
    fn f_export_01_name_local_time() {
        // 2023-11-14T22:13:20Z, +1h -> 23:13:20
        assert_eq!(export_name(&meta().now), "xplain-review-20231114-231320.md");
        let n = Now { unix_ms: 1_700_000_000_000, utc_offset_secs: 2 * 3600 };
        assert_eq!(export_name(&n), "xplain-review-20231115-001320.md");
        let n = Now { unix_ms: 0, utc_offset_secs: -3600 };
        assert_eq!(export_name(&n), "xplain-review-19691231-230000.md");
        let n = Now { unix_ms: 951_782_400_000, utc_offset_secs: 0 }; // 2000-02-29
        assert_eq!(export_name(&n), "xplain-review-20000229-000000.md");
    }

    #[test]
    fn f_export_02_iso_date() {
        assert_eq!(iso_utc(1_700_000_000_123), "2023-11-14T22:13:20.123Z");
        assert_eq!(iso_utc(0), "1970-01-01T00:00:00.000Z");
    }

    #[test]
    fn f_export_02_golden_simple_comment() {
        let mut c = comment("q1", 1, 3, "why?");
        c.text = "let x = 1;".into();
        c.context = vec!["a".into(), "let x = 1;".into()];
        let md = render_markdown(&[c], &meta());
        let want = "# xplain review\n\n- repo: `/repo`\n- diff: `all`\n- date: 2023-11-14T22:13:20.123Z\n- comments: 1\n\n## `a.rs`\n\n### 1. new side, line 3\n\n- origin: human\n- state: saved\n\nLine:\n\n```\nlet x = 1;\n```\n\nContext:\n\n```\na\nlet x = 1;\n```\n\n**Comment:**\n\n> why?\n";
        assert_eq!(md, want);
    }

    #[test]
    fn f_export_02_golden_thread_with_answers() {
        let mut c = comment("q1", 1, 3, "why?");
        c.text = "x".into();
        let mut a1 = answer(AnswerStatus::Done, "b2\n\nline");
        a1.agent = Some("bot".into());
        c.turns[0].prior = vec![{
            let mut p = answer(AnswerStatus::Done, "b1");
            p.agent = Some("bot".into());
            p
        }];
        c.turns[0].answer = Some(a1);
        c.turns.push(Turn {
            message: "and?".into(),
            answer: Some(answer(AnswerStatus::Cancelled, "MCP stopped")),
            prior: vec![],
        });
        let md = render_markdown(&[c], &meta());
        let tail = md.split("Line:").nth(1).expect("tail");
        assert_eq!(
            tail,
            "\n\n```\nx\n```\n\n**Comment:**\n\n> why?\n\n**Answer (bot) - done**\n\n> b1\n\n**Answer (bot) - done**\n\n> b2\n>\n> line\n\n**Follow-up 1:**\n\n> and?\n\n**Answer - cancelled**\n\n> MCP stopped\n"
        );
        assert!(md.contains("- state: cancelled\n"));
    }

    #[test]
    fn f_export_02_states() {
        let mut c = comment("q1", 1, 1, "m");
        assert_eq!(state_of(&c), "saved");
        c.turns[0].answer = Some(answer(AnswerStatus::Done, "x"));
        assert_eq!(state_of(&c), "answered");
        c.turns[0].answer = Some(answer(AnswerStatus::Pending, ""));
        assert_eq!(state_of(&c), "pending");
        c.turns[0].answer = Some(answer(AnswerStatus::Streaming, ""));
        assert_eq!(state_of(&c), "streaming");
    }

    #[test]
    fn f_export_02_where_forms() {
        let mut c = comment("q1", 1, 3, "m");
        assert_eq!(where_of(&c), "new side, line 3");
        c.side = PaneSide::Old;
        c.selection =
            Some(SelectionInfo { start_line: Some(3), end_line: Some(3), start_col: 1, end_col: 5 });
        assert_eq!(where_of(&c), "old side, selection line 3, cols 1-5");
        c.selection =
            Some(SelectionInfo { start_line: Some(3), end_line: Some(6), start_col: 2, end_col: 9 });
        assert_eq!(where_of(&c), "old side, selection lines 3-6, cols 2-9");
        c.selection = None;
        c.line = None;
        c.row = 4;
        assert_eq!(where_of(&c), "old side, row 5");
    }

    #[test]
    fn f_export_02_selection_agent_note_and_order() {
        let mut sel = comment("q1", 1, 9, "sel");
        sel.text = "a\nb".into();
        sel.selection =
            Some(SelectionInfo { start_line: Some(2), end_line: Some(3), start_col: 1, end_col: 4 });
        let mut note = comment("q2", 2, 5, "note text");
        note.origin = Origin::Agent;
        let mut other = comment("q3", 3, 1, "other file");
        other.file = "B.rs".into();
        let mut first = comment("q4", 4, 1, "first");
        first.file = "a.rs".into();
        let md = render_markdown(&[sel, note, other, first], &meta());
        // files: code-unit order, uppercase first
        let p_b = md.find("## `B.rs`").expect("B");
        let p_a = md.find("## `a.rs`").expect("a");
        assert!(p_b < p_a);
        // within a.rs: line 1 (q4), selection start 2 (q1), line 5 (note)
        let p1 = md.find("### 2. new side, line 1").expect("first");
        let p2 = md.find("### 3. new side, selection lines 2-3, cols 1-4").expect("sel");
        let p3 = md.find("### 4. new side, line 5").expect("note");
        assert!(p1 < p2 && p2 < p3);
        assert!(md.contains("- origin: agent\n- state: saved\n\n**Note:**\n\n> note text\n"));
        assert!(md.contains("Selected text:\n\n```\na\nb\n```"));
    }

    #[test]
    fn f_export_02_fences_and_inline() {
        assert_eq!(fence("no ticks"), "```");
        assert_eq!(fence("a ``` b"), "````");
        assert_eq!(inline("plain"), "`plain`");
        assert_eq!(inline("`x"), "` `x `");
        assert_eq!(inline("x`"), "` x` `");
        let mut c = comment("q1", 1, 1, "m");
        c.text = "```rs\ncode\n```".into();
        let md = render_markdown(&[c], &meta());
        assert!(md.contains("````\n```rs\ncode\n```\n````"));
    }

    #[test]
    fn f_export_02_quote_and_collapse() {
        assert_eq!(quote("a\n\nb"), "> a\n>\n> b");
        let mut c = comment("q1", 1, 1, "a\n\n\n\nb");
        c.turns[0].message = "a\n\n\n\nb".into();
        let md = render_markdown(&[c], &meta());
        assert!(md.contains("> a\n>\n>\n>\n> b"));
        assert!(!md.contains("\n\n\n"));
        assert!(md.ends_with("> b\n"));
    }

    #[test]
    fn f_export_02_diff_args_in_header() {
        let mut m = meta();
        m.mode = "staged".into();
        m.args = vec!["main..HEAD".into(), "--stat".into()];
        let md = render_markdown(&[], &m);
        assert!(md.contains("- diff: `staged main..HEAD --stat`\n"));
        assert!(md.contains("- comments: 0\n\nNo comments.\n"));
    }

    #[test]
    fn f_export_01_e_key_effect_and_notes() {
        let mut st = state_with(Vec::new());
        st.env.abs_cwd = "/work/repo".into();
        st.clock = meta().now;
        let mut fx = Vec::new();
        assert!(!on_key(&mut st, KeyEvent::ch('e'), &mut fx));
        assert!(on_key(&mut st, KeyEvent::ch('E'), &mut fx));
        assert_eq!(st.note.as_deref(), Some("no comments to export"));
        assert!(fx.is_empty());
        st.comments.push(comment("q1", 1, 1, "m"));
        assert!(on_key(&mut st, KeyEvent::ch('E'), &mut fx));
        let path = "/work/repo/xplain-review-20231114-231320.md".to_string();
        let req = match &fx[0] {
            Effect::WriteExport { req, path: p, contents } => {
                assert_eq!(*p, path);
                assert!(contents.starts_with("# xplain review\n"));
                *req
            }
            other => panic!("{other:?}"),
        };
        assert_eq!(st.loader.pending.get(&req), Some(&Pending::Export { path: path.clone(), count: 1 }));
        on_written(&mut st, req, Ok(()));
        assert_eq!(st.note.as_deref(), Some(format!("exported 1 comment -> {path}").as_str()));
        assert!(st.loader.pending.is_empty());
    }

    #[test]
    fn f_export_01_written_failure_and_plural() {
        let mut st = state_with(Vec::new());
        let req = st.alloc_req();
        st.loader.pending.insert(req, Pending::Export { path: "/p/x.md".into(), count: 2 });
        on_written(&mut st, req, Err(IoReason::PermissionDenied));
        assert_eq!(st.note.as_deref(), Some("export failed: /p/x.md: permission denied"));
        let req = st.alloc_req();
        st.loader.pending.insert(req, Pending::Export { path: "/p/y.md".into(), count: 2 });
        on_written(&mut st, req, Ok(()));
        assert_eq!(st.note.as_deref(), Some("exported 2 comments -> /p/y.md"));
        // stale request: ignored
        st.note = None;
        on_written(&mut st, ReqId(999), Ok(()));
        assert!(st.note.is_none());
    }

    #[test]
    fn f_export_01_root_cwd() {
        let mut st = state_with(Vec::new());
        st.env.abs_cwd = "/".into();
        st.comments.push(comment("q1", 1, 1, "m"));
        let mut fx = Vec::new();
        on_key(&mut st, KeyEvent::ch('E'), &mut fx);
        assert!(matches!(&fx[0], Effect::WriteExport { path, .. } if path.starts_with("/xplain-review-")));
    }
}
