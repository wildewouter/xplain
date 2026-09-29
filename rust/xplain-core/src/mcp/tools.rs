//! MCP tool table and tool calls: `next_question`, `answer`, `annotate`, `get_questions`, `files_changed`.
//!
//! Spec: F-MCPSRV-05..10 (schemas, argument validation and clamps, result texts, `closed`), F-ASK-09
//! (answer arrival), F-COMMENT-10 (annotate). Oracle: `src/mcp/tools.ts`, `src/mcp/hub.ts` (sanitize).
//! Owner: component `agent` (E). Must not: do HTTP framing (rpc.rs).

use serde_json::{Value, json};

use super::hub::{PollResult, cap_chars};
use super::{ConnId, HubEvent, McpOutput, McpState};
use crate::comments::PaneSide;

/// Server instructions text and version constants (F-MCPSRV-04).
pub const SERVER_VERSION: &str = "0.1.0";
pub const PROTOCOL_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];
/// Default and max `wait_seconds` (F-MCPSRV-06).
pub const DEFAULT_WAIT_S: u64 = 45;
pub const MAX_WAIT_S: u64 = 120;
/// Max chars of agent text (F-MCPSRV-07).
pub const MAX_TEXT: usize = 20000;

const AGAIN: &str = "Call next_question again immediately.";
const CODE_HINT: &str = "Put code samples in markdown fenced blocks (```lang ... ```): xplain highlights them and gives the user a copy button.";

/// Names of the five tools, in `tools/list` order.
pub const TOOL_NAMES: [&str; 5] = ["next_question", "answer", "get_questions", "annotate", "files_changed"];

/// `tools/list` result array (F-MCPSRV-04).
pub fn tool_list() -> Value {
    json!([
        {
            "name": "next_question",
            "description": "Long-poll for the next question a human asked in the xplain code-review UI. Returns {status:\"question\", thread_id, turn, follow_up, question}: answer it with the answer tool. If follow_up is true it continues an earlier thread: use `previous` (earlier questions and your answers) for reference and answer with the SAME thread_id. A follow-up can also be the user replying to a note you added with annotate: the question then names the note (file, line, text). If status is \"no_question_yet\", call next_question again immediately. Keep looping: after every answer, call next_question again immediately, until status is \"closed\".",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "wait_seconds": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": MAX_WAIT_S,
                        "default": DEFAULT_WAIT_S,
                        "description": "Max seconds to wait for a question (1-120)."
                    }
                },
                "additionalProperties": false
            }
        },
        {
            "name": "answer",
            "description": format!("Send your answer for a question received from next_question, using its thread_id (for a follow-up, the same thread_id as before). Plain text or markdown. {CODE_HINT} Then call next_question again immediately."),
            "inputSchema": {
                "type": "object",
                "properties": {
                    "thread_id": {"type": "string", "description": "thread_id from next_question."},
                    "text": {"type": "string", "description": format!("The answer text. {CODE_HINT}")}
                },
                "required": ["thread_id", "text"],
                "additionalProperties": false
            }
        },
        {
            "name": "get_questions",
            "description": "List questions (and follow-ups, marked follow_up) still waiting to be delivered, without consuming them. Does not replace the next_question loop.",
            "inputSchema": {"type": "object", "properties": {}, "additionalProperties": false}
        },
        {
            "name": "annotate",
            "description": "Attach a note to a line of a file in the xplain diff view. The user may reply to it; replies arrive via next_question as follow-ups. Then continue the next_question loop.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file": {"type": "string", "description": "File path as shown in the diff."},
                    "line": {"type": "number", "description": "1-based line number."},
                    "text": {"type": "string", "description": format!("Annotation text. {CODE_HINT}")},
                    "side": {"type": "string", "enum": ["old", "new"], "description": "Diff side; default new."},
                    "number": {
                        "type": "integer",
                        "description": "Optional order label shown on the note (e.g. step 1, 2, 3); the user jumps between numbered notes in order."
                    }
                },
                "required": ["file", "line", "text"],
                "additionalProperties": false
            }
        },
        {
            "name": "files_changed",
            "description": "REQUIRED after every file change: you MUST call this after each edit, creation, rename or deletion of any file on disk (also right after a batch of edits, before replying). It makes the xplain diff view reload; without it the user sees stale content. Pass the changed paths in `paths`. Then continue the next_question loop.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "paths": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Paths of the files you edited, created or deleted (relative to the repo root)."
                    }
                },
                "additionalProperties": false
            }
        }
    ])
}

/// Instructions string of `initialize` (F-MCPSRV-04).
pub fn instructions() -> &'static str {
    "xplain shows the user a live diff of the working tree. REQUIRED: after EVERY edit, create, rename or delete of a file on disk, call the `files_changed` tool (pass the changed paths). Without this call the xplain view stays stale and the user does not see your changes. Batch several edits into one call, but always call it before you answer or go idle. Questions from the user arrive via `next_question`; answer with `answer`."
}

/// Result of one call: either finished with a JSON-RPC `result` value, or parked (long poll).
#[derive(Debug, Clone, PartialEq)]
pub enum ToolOutcome {
    Done {
        result: Value,
        events: Vec<HubEvent>,
    },
    /// `next_question` waiting: caller must arm `TimerId::PollTimeout(conn)` (background) for `wait_ms`.
    Parked {
        wait_ms: u64,
    },
}

/// Clamp `wait_seconds`: non-number 45, floored, 1..=120.
pub fn clamp_wait(v: Option<&Value>) -> u64 {
    let n = match v.and_then(Value::as_f64) {
        Some(f) if f.is_finite() => f.floor().clamp(1.0, MAX_WAIT_S as f64) as u64,
        _ => DEFAULT_WAIT_S,
    };
    n.clamp(1, MAX_WAIT_S)
}

fn done(text: String, is_error: bool) -> ToolOutcome {
    ToolOutcome::Done { result: text_result(&text, is_error), events: Vec::new() }
}

fn q(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| String::from("\"\""))
}

/// Text of a finished long poll (F-MCPSRV-06), keys in spec order.
pub fn poll_text(r: &PollResult) -> String {
    match r {
        PollResult::Closed => {
            "{\"status\":\"closed\",\"note\":\"xplain closed the session. Stop.\"}".to_string()
        }
        PollResult::NoQuestion => format!(
            "{{\"status\":\"no_question_yet\",\"call_again\":true,\"note\":\"No question yet. {AGAIN}\"}}"
        ),
        PollResult::Question(qn) => {
            let mut s = format!(
                "{{\"status\":\"question\",\"thread_id\":{},\"turn\":{},\"follow_up\":{}",
                q(&qn.thread_id),
                qn.turn,
                qn.follow_up
            );
            if qn.follow_up {
                let items: Vec<String> = qn
                    .previous
                    .iter()
                    .map(|(t, question, answer)| {
                        format!("{{\"turn\":{t},\"question\":{},\"answer\":{}}}", q(question), q(answer))
                    })
                    .collect();
                s.push_str(&format!(",\"previous\":[{}]", items.join(",")));
            }
            s.push_str(&format!(",\"question\":{}}}", q(&qn.question)));
            s
        }
    }
}

/// Run tool `name` for client `client_id` (session id or `anon:<port>`). Unknown tool handled by rpc.rs.
pub fn call(
    state: &mut McpState,
    client_id: &str,
    conn: ConnId,
    name: &str,
    args: &Value,
    _now_ms: i64,
) -> ToolOutcome {
    let empty = serde_json::Map::new();
    let a = args.as_object().unwrap_or(&empty);
    match name {
        "next_question" => {
            let wait_ms = clamp_wait(a.get("wait_seconds")) * 1000;
            match state.poll(client_id, conn) {
                Some(r) => done(poll_text(&r), false),
                None => ToolOutcome::Parked { wait_ms },
            }
        }
        "answer" => {
            let (Some(Value::String(thread)), Some(Value::String(text))) =
                (a.get("thread_id"), a.get("text"))
            else {
                return done("thread_id and text (strings) are required".to_string(), true);
            };
            let Some(turn) = state.delivered_turn(thread) else {
                return done(format!("Unknown thread_id: {}", cap_chars(thread, 100)), true);
            };
            let ev = HubEvent::Answer { thread_id: thread.clone(), turn, text: sanitize(text) };
            ToolOutcome::Done {
                result: text_result(
                    &format!("{{\"ok\":true,\"note\":\"Answer delivered. {AGAIN}\"}}"),
                    false,
                ),
                events: vec![ev],
            }
        }
        "get_questions" => {
            let items: Vec<String> = state
                .queue
                .iter()
                .map(|qn| {
                    format!(
                        "{{\"thread_id\":{},\"turn\":{},\"follow_up\":{},\"preview\":{}}}",
                        q(&qn.thread_id),
                        qn.turn,
                        qn.follow_up,
                        q(&state.preview_of(qn))
                    )
                })
                .collect();
            done(format!("{{\"questions\":[{}]}}", items.join(",")), false)
        }
        "annotate" => {
            let (Some(Value::String(file)), Some(Value::String(text)), Some(line)) =
                (a.get("file"), a.get("text"), a.get("line").filter(|v| v.is_number()))
            else {
                return done("file (string), line (number) and text (string) are required".to_string(), true);
            };
            let side = match a.get("side").and_then(Value::as_str) {
                Some("old") => PaneSide::Old,
                _ => PaneSide::New,
            };
            let number = a.get("number").and_then(as_integer).and_then(|n| u32::try_from(n).ok());
            let line = match line.as_f64() {
                Some(f) if f.is_finite() => f.floor().clamp(1.0, f64::from(u32::MAX)) as u32,
                _ => 1,
            };
            let ev = HubEvent::Annotate { file: sanitize(file), line, text: sanitize(text), side, number };
            ToolOutcome::Done { result: text_result("{\"ok\":true}", false), events: vec![ev] }
        }
        "files_changed" => {
            let paths: Vec<String> = match a.get("paths") {
                Some(Value::Array(v)) => v
                    .iter()
                    .filter_map(Value::as_str)
                    .take(100)
                    .map(|p| cap_chars(&sanitize(p), 500))
                    .collect(),
                _ => Vec::new(),
            };
            ToolOutcome::Done {
                result: text_result("{\"ok\":true}", false),
                events: vec![HubEvent::FilesChanged { paths }],
            }
        }
        other => done(format!("Unknown tool: {}", cap_chars(other, 100)), true),
    }
}

/// JSON integer value (`3`, `3.0`) as i64.
fn as_integer(v: &Value) -> Option<i64> {
    if let Some(i) = v.as_i64() {
        return Some(i);
    }
    let f = v.as_f64()?;
    (f.is_finite() && f.fract() == 0.0 && f.abs() < 9.0e15).then_some(f as i64)
}

/// Helper for tests/rpc: wraps text into `{content:[{type:"text",text}], isError?}`.
pub fn text_result(text: &str, is_error: bool) -> Value {
    let mut v = json!({"content": [{"type": "text", "text": text}]});
    if is_error {
        v["isError"] = Value::Bool(true);
    }
    v
}

/// Strip ANSI escapes and control chars (keep `\n`, `\t`), cap 20000 chars (F-MCPSRV-07/08).
pub fn sanitize(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\u{1b}' {
            i += esc_len(&chars[i..]);
            continue;
        }
        let cp = c as u32;
        let ctrl = matches!(cp, 0x00..=0x08 | 0x0b..=0x1f | 0x7f..=0x9f);
        if !ctrl {
            out.push(c);
        }
        i += 1;
    }
    cap_chars(&out, MAX_TEXT)
}

/// Length of the escape sequence starting at `c[0] == ESC` (OSC, CSI, two-char escape, lone ESC).
fn esc_len(c: &[char]) -> usize {
    match c.get(1) {
        Some(']') => {
            // OSC: body up to BEL or ESC \ ; otherwise ESC ] alone (two-char escape)
            let mut j = 2;
            while j < c.len() && c[j] != '\u{7}' && c[j] != '\u{1b}' {
                j += 1;
            }
            match (c.get(j), c.get(j + 1)) {
                (Some('\u{7}'), _) => j + 1,
                (Some('\u{1b}'), Some('\\')) => j + 2,
                _ => 2,
            }
        }
        Some('[') => {
            let mut j = 2;
            while j < c.len() && ('\u{30}'..='\u{3f}').contains(&c[j]) {
                j += 1;
            }
            while j < c.len() && ('\u{20}'..='\u{2f}').contains(&c[j]) {
                j += 1;
            }
            if j < c.len() && ('\u{40}'..='\u{7e}').contains(&c[j]) { j + 1 } else { 1 }
        }
        Some(x) if ('@'..='Z').contains(x) || ('\\'..='_').contains(x) => 2,
        _ => 1,
    }
}

/// Emit an `McpOutput` with no effects for a finished call (used by tests).
pub fn events_only(events: Vec<HubEvent>) -> McpOutput {
    McpOutput { effects: Vec::new(), events }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::OutQuestion;

    fn text_of(o: &ToolOutcome) -> (String, bool) {
        match o {
            ToolOutcome::Done { result, .. } => (
                result["content"][0]["text"].as_str().unwrap_or("").to_string(),
                result.get("isError").is_some(),
            ),
            ToolOutcome::Parked { .. } => ("<parked>".into(), false),
        }
    }

    fn run(s: &mut McpState, name: &str, args: Value) -> ToolOutcome {
        call(s, "c1", ConnId(1), name, &args, 0)
    }

    #[test]
    fn f_mcpsrv_04_tool_list_shape() {
        let l = tool_list();
        let names: Vec<&str> = l.as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(names, TOOL_NAMES);
        assert_eq!(l[0]["inputSchema"]["properties"]["wait_seconds"]["maximum"], 120);
        assert_eq!(l[1]["inputSchema"]["required"], json!(["thread_id", "text"]));
        assert!(instructions().starts_with("xplain shows the user a live diff"));
    }

    #[test]
    fn f_mcpsrv_06_clamp_wait() {
        assert_eq!(clamp_wait(None), 45);
        assert_eq!(clamp_wait(Some(&json!("x"))), 45);
        assert_eq!(clamp_wait(Some(&json!(0))), 1);
        assert_eq!(clamp_wait(Some(&json!(2.9))), 2);
        assert_eq!(clamp_wait(Some(&json!(500))), 120);
        assert_eq!(clamp_wait(Some(&json!(-5))), 1);
    }

    #[test]
    fn f_mcpsrv_07_sanitize() {
        assert_eq!(sanitize("a\u{1b}[31mred\u{1b}[0m\tb\nc\rd\u{7}e"), "ared\tb\ncde");
        assert_eq!(sanitize("x\u{1b}]0;title\u{7}y"), "xy");
        assert_eq!(sanitize("x\u{1b}]0;title\u{1b}\\y"), "xy");
        assert_eq!(sanitize("x\u{1b}]open"), "xopen");
        assert_eq!(sanitize("x\u{1b}[31"), "x[31");
        assert_eq!(sanitize("a\u{1b}Mb"), "ab");
        assert_eq!(sanitize("a\u{1b}"), "a");
        assert_eq!(sanitize("a\u{85}\u{9f}b\u{a0}"), "ab\u{a0}");
        assert_eq!(sanitize(&"x".repeat(25000)).len(), 20000);
    }

    #[test]
    fn f_mcpsrv_07_answer_results() {
        let mut s = McpState::default();
        let (t, e) = text_of(&run(&mut s, "answer", json!({"thread_id": 1, "text": "x"})));
        assert_eq!((t.as_str(), e), ("thread_id and text (strings) are required", true));
        let (t, e) = text_of(&run(&mut s, "answer", json!({"thread_id": "qx", "text": "x"})));
        assert_eq!((t.as_str(), e), ("Unknown thread_id: qx", true));
        s.inner.turns.push(("q1".into(), 2));
        match run(&mut s, "answer", json!({"thread_id": "q1", "text": "hi\u{1b}[1m"})) {
            ToolOutcome::Done { result, events } => {
                assert_eq!(
                    result["content"][0]["text"],
                    "{\"ok\":true,\"note\":\"Answer delivered. Call next_question again immediately.\"}"
                );
                assert!(result.get("isError").is_none());
                assert_eq!(
                    events,
                    vec![HubEvent::Answer { thread_id: "q1".into(), turn: 2, text: "hi".into() }]
                );
            }
            ToolOutcome::Parked { .. } => panic!("parked"),
        }
        let long = "y".repeat(150);
        let (t, _) = text_of(&run(&mut s, "answer", json!({"thread_id": long, "text": "x"})));
        assert_eq!(t, format!("Unknown thread_id: {}", "y".repeat(100)));
    }

    #[test]
    fn f_mcpsrv_08_annotate() {
        let mut s = McpState::default();
        let (t, e) = text_of(&run(&mut s, "annotate", json!({"file": "a", "line": "1", "text": "t"})));
        assert_eq!((t.as_str(), e), ("file (string), line (number) and text (string) are required", true));
        match run(
            &mut s,
            "annotate",
            json!({"file": "a.rs", "line": 0.7, "text": "n", "side": "bogus", "number": 1.5}),
        ) {
            ToolOutcome::Done { result, events } => {
                assert_eq!(result["content"][0]["text"], "{\"ok\":true}");
                assert_eq!(
                    events,
                    vec![HubEvent::Annotate {
                        file: "a.rs".into(),
                        line: 1,
                        text: "n".into(),
                        side: PaneSide::New,
                        number: None
                    }]
                );
            }
            ToolOutcome::Parked { .. } => panic!("parked"),
        }
        match run(
            &mut s,
            "annotate",
            json!({"file": "a.rs", "line": 7.9, "text": "n", "side": "old", "number": 3}),
        ) {
            ToolOutcome::Done { events, .. } => assert_eq!(
                events,
                vec![HubEvent::Annotate {
                    file: "a.rs".into(),
                    line: 7,
                    text: "n".into(),
                    side: PaneSide::Old,
                    number: Some(3)
                }]
            ),
            ToolOutcome::Parked { .. } => panic!("parked"),
        }
    }

    #[test]
    fn f_mcpsrv_10_files_changed() {
        let mut s = McpState::default();
        let paths: Vec<Value> = (0..120).map(|i| json!(format!("f{i}"))).chain([json!(5)]).collect();
        match run(&mut s, "files_changed", json!({"paths": paths})) {
            ToolOutcome::Done { events, .. } => match &events[0] {
                HubEvent::FilesChanged { paths } => {
                    assert_eq!(paths.len(), 100);
                    assert_eq!(paths[0], "f0");
                }
                other => panic!("{other:?}"),
            },
            ToolOutcome::Parked { .. } => panic!("parked"),
        }
        match run(&mut s, "files_changed", json!({"paths": "x"})) {
            ToolOutcome::Done { events, .. } => {
                assert_eq!(events, vec![HubEvent::FilesChanged { paths: vec![] }])
            }
            ToolOutcome::Parked { .. } => panic!("parked"),
        }
    }

    #[test]
    fn f_mcpsrv_09_get_questions() {
        let mut s = McpState::default();
        s.queue.push(OutQuestion {
            thread_id: "q1".into(),
            turn: 1,
            question: "why?\n\nFile: a".into(),
            follow_up: false,
            previous: vec![],
        });
        s.inner.previews.push(("q1".into(), 1, "why?".into()));
        let (t, _) = text_of(&run(&mut s, "get_questions", json!({})));
        assert_eq!(
            t,
            "{\"questions\":[{\"thread_id\":\"q1\",\"turn\":1,\"follow_up\":false,\"preview\":\"why?\"}]}"
        );
        assert_eq!(s.queue.len(), 1);
    }

    #[test]
    fn f_mcpsrv_06_poll_texts() {
        assert_eq!(
            poll_text(&PollResult::Closed),
            "{\"status\":\"closed\",\"note\":\"xplain closed the session. Stop.\"}"
        );
        assert_eq!(
            poll_text(&PollResult::NoQuestion),
            "{\"status\":\"no_question_yet\",\"call_again\":true,\"note\":\"No question yet. Call next_question again immediately.\"}"
        );
        let mut oq = OutQuestion {
            thread_id: "q1".into(),
            turn: 1,
            question: "hi".into(),
            follow_up: false,
            previous: vec![],
        };
        assert_eq!(
            poll_text(&PollResult::Question(oq.clone())),
            "{\"status\":\"question\",\"thread_id\":\"q1\",\"turn\":1,\"follow_up\":false,\"question\":\"hi\"}"
        );
        oq.turn = 2;
        oq.follow_up = true;
        oq.previous = vec![(1, "why?".into(), "because".into())];
        oq.question = "Follow-up to your earlier answer (thread q1, turn 2): and?".into();
        assert_eq!(
            poll_text(&PollResult::Question(oq)),
            "{\"status\":\"question\",\"thread_id\":\"q1\",\"turn\":2,\"follow_up\":true,\"previous\":[{\"turn\":1,\"question\":\"why?\",\"answer\":\"because\"}],\"question\":\"Follow-up to your earlier answer (thread q1, turn 2): and?\"}"
        );
    }

    #[test]
    fn f_mcpsrv_05_unknown_and_non_object_args() {
        let mut s = McpState::default();
        let (t, e) = text_of(&run(&mut s, "nope", json!({})));
        assert_eq!((t.as_str(), e), ("Unknown tool: nope", true));
        let (t, e) = text_of(&run(&mut s, "get_questions", json!("junk")));
        assert_eq!((t.as_str(), e), ("{\"questions\":[]}", false));
    }
}
