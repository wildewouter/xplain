//! Ported from `e2e/scenarios/u12-ask`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use regex::Regex;
use serde_json::{Value, json};
use xplain_sim::{CellExpect as C, Http, Sim};

fn j(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|e| panic!("bad json {text}: {e}"))
}

/// JSON subset match of the e2e runner: objects need only the listed keys, arrays match element-wise (same
/// length), scalars are equal. An object of `$` operators (`$contains`, `$matches`, `$len`, `$exists`,
/// `$not_contains`) checks the actual value instead.
#[track_caller]
fn check(actual: &Value, want: &Value, path: &str) {
    match want {
        Value::Object(m) if !m.is_empty() && m.keys().all(|k| k.starts_with('$')) => {
            for (op, arg) in m {
                match op.as_str() {
                    "$len" => {
                        let n = actual.as_array().map(Vec::len).or_else(|| actual.as_str().map(str::len));
                        assert_eq!(n, arg.as_u64().map(|n| n as usize), "{path}: length of {actual}");
                    }
                    "$matches" => {
                        let t = actual.as_str().unwrap_or_else(|| panic!("{path}: not a string: {actual}"));
                        let r = Regex::new(arg.as_str().unwrap_or_default()).unwrap();
                        assert!(r.is_match(t), "{path}: {t:?} does not match /{arg}/");
                    }
                    "$not_contains" => {
                        let t = actual.as_str().unwrap_or_else(|| panic!("{path}: not a string: {actual}"));
                        assert!(
                            !t.contains(arg.as_str().unwrap_or_default()),
                            "{path}: {t:?} contains {arg}"
                        );
                    }
                    "$exists" => assert_eq!(!actual.is_null(), arg.as_bool().unwrap_or(true), "{path}"),
                    "$contains" => match actual {
                        Value::String(t) => {
                            assert!(t.contains(arg.as_str().unwrap_or_default()), "{path}: {t:?}")
                        }
                        Value::Array(a) => assert!(
                            a.iter().any(|x| std::panic::catch_unwind(|| check(x, arg, path)).is_ok()),
                            "{path}: no element matches {arg} in {actual}"
                        ),
                        _ => panic!("{path}: $contains on {actual}"),
                    },
                    _ => panic!("{path}: unknown operator {op}"),
                }
            }
        }
        Value::Object(m) => {
            for (k, w) in m {
                let a = actual.get(k).unwrap_or_else(|| panic!("{path}.{k} missing in {actual}"));
                check(a, w, &format!("{path}.{k}"));
            }
        }
        Value::Array(w) => {
            let a = actual.as_array().unwrap_or_else(|| panic!("{path}: not an array: {actual}"));
            assert_eq!(a.len(), w.len(), "{path}: array length, got {actual}");
            for (i, (a, w)) in a.iter().zip(w).enumerate() {
                check(a, w, &format!("{path}[{i}]"));
            }
        }
        _ => assert_eq!(actual, want, "{path}"),
    }
}

/// F-ASK-01: asked comments are queued as pending turn-1 questions q1, q2 in creation order with message and context
#[test]
fn f_ask_01_enqueue() {
    let mut s = Sim::builder().config_json(json!({"mcp": {"autostart": true}})).build();
    s.assert_row_contains(0, "[mcp: on]");
    let r = s.mcp_call("get_questions", j(r"{}"));
    check(&r.tool_result(), &j(r#"{"questions": {"$len": 0}}"#), "result");
    s.keys("a");
    s.keys("first q<Enter>");
    s.assert_row_contains(-1, "question sent to agent");
    s.assert_contains("first q");
    s.assert_contains("─ answer · agent · waiting… ─");
    s.assert_contains(" ⠿ waiting for agent…");
    s.keys("j");
    s.keys("a");
    s.keys("second q<Enter>");
    s.assert_contains("second q");
    s.assert_matches(r"waiting…[\s\S]*waiting…");
    // both queued, undelivered, in creation order
    let r = s.mcp_call("get_questions", j(r"{}"));
    check(
        &r.tool_result(),
        &j(
            r#"{"questions": [{"thread_id": "q1", "turn": 1, "follow_up": false, "preview": "first q"}, {"thread_id": "q2", "turn": 1, "follow_up": false, "preview": "second q"}]}"#,
        ),
        "result",
    );
    let r = s.mcp_call("next_question", j(r#"{"wait_seconds": 5}"#));
    check(
        &r.tool_result(),
        &j(
            r#"{"status": "question", "thread_id": "q1", "turn": 1, "follow_up": false, "question": {"$matches": "^first q\\n\\nFile: README\\.md\\n[\\s\\S]*```\\nhello\\n```"}}"#,
        ),
        "result",
    );
    let r = s.mcp_call("next_question", j(r#"{"wait_seconds": 5}"#));
    check(
        &r.tool_result(),
        &j(
            r#"{"status": "question", "thread_id": "q2", "turn": 1, "follow_up": false, "question": {"$matches": "^second q\\n\\nFile: README\\.md\\n[\\s\\S]*```\\nhello world\\n```"}}"#,
        ),
        "result",
    );
    let r = s.mcp_call("get_questions", j(r"{}"));
    check(&r.tool_result(), &j(r#"{"questions": {"$len": 0}}"#), "result");
}

/// F-ASK-01: thread id is the comment id; saved comments and agent notes consume ids too
#[test]
fn f_ask_01_ids_shared() {
    let mut s = Sim::builder().config_json(json!({"mcp": {"autostart": true}})).build();
    s.assert_row_contains(0, "[mcp: on]");
    // comment 1 saved (not asked)
    s.keys("a");
    s.keys("<Tab>");
    s.assert_contains("[save] enter send");
    s.keys("saved one<Enter>");
    s.assert_row_contains(-1, "question saved (1)");
    // comment 2 is an agent note
    let r = s.mcp_call("annotate", j(r#"{"file": "README.md", "line": 3, "text": "agent says hi"}"#));
    check(&r.tool_result(), &j(r#"{"ok": true}"#), "result");
    s.assert_contains("agent says hi");
    // comment 3 asked
    s.keys("jj");
    s.keys("a");
    s.assert_contains("[save] enter send");
    s.keys("<Tab>");
    s.assert_contains("[ask] enter send");
    s.keys("asked three<Enter>");
    s.assert_row_contains(-1, "question sent to agent");
    let r = s.mcp_call("get_questions", j(r"{}"));
    check(
        &r.tool_result(),
        &j(
            r#"{"questions": [{"thread_id": "q3", "turn": 1, "follow_up": false, "preview": "asked three"}]}"#,
        ),
        "result",
    );
    // asking the saved comment later keeps its own id q1
    s.keys("J");
    s.assert_contains("▸ sent  line L2");
    s.keys("a");
    s.assert_row_contains(-1, "question queued");
    let r = s.mcp_call("get_questions", j(r"{}"));
    check(
        &r.tool_result(),
        &j(
            r#"{"questions": [{"thread_id": "q3", "turn": 1, "follow_up": false, "preview": "asked three"}, {"thread_id": "q1", "turn": 1, "follow_up": false, "preview": "saved one"}]}"#,
        ),
        "result",
    );
    let r = s.mcp_call("next_question", j(r#"{"wait_seconds": 5}"#));
    check(
        &r.tool_result(),
        &j(
            r#"{"status": "question", "thread_id": "q3", "turn": 1, "question": {"$matches": "^asked three\\n\\nFile: README\\.md\\n"}}"#,
        ),
        "result",
    );
    let r = s.mcp_call("next_question", j(r#"{"wait_seconds": 5}"#));
    check(
        &r.tool_result(),
        &j(
            r#"{"status": "question", "thread_id": "q1", "turn": 1, "question": {"$matches": "^saved one\\n\\nFile: README\\.md\\n"}}"#,
        ),
        "result",
    );
}

/// F-ASK-02: a on an agent note without reply opens the follow-up editor; with a reply pending it notes still waiting
#[test]
fn f_ask_02_agent_note() {
    let mut s = Sim::builder().config_json(json!({"mcp": {"autostart": true}})).build();
    let r = s.mcp_call("annotate", j(r#"{"file": "README.md", "line": 2, "text": "my note"}"#));
    check(&r.tool_result(), &j(r#"{"ok": true}"#), "result");
    s.keys("J");
    s.assert_contains("▸ sent  agent note L2");
    s.assert_not_contains("enter send");
    s.keys("a");
    s.assert_contains("enter send  esc cancel");
    s.assert_matches("^│ follow-up +│$");
    s.keys("reply<Enter>");
    s.assert_row_contains(-1, "follow-up queued");
    s.assert_contains("follow-up: reply");
    s.assert_not_contains("enter send");
    s.keys("a");
    s.assert_row_contains(-1, "still waiting for the agent");
    s.assert_not_contains("enter send");
}

/// F-ASK-02: a on a saved comment notes MCP is off while off, queues it once MCP runs
#[test]
fn f_ask_02_askable() {
    let mut s = Sim::builder().build();
    s.assert_row_contains(0, "[mcp: off]");
    s.keys("a");
    s.keys("why?<Enter>");
    s.assert_row_contains(-1, "question saved (1)");
    s.assert_contains("saved · not asked");
    s.keys("K");
    s.keys("a");
    s.assert_row_contains(-1, "MCP is off (M to start)");
    s.assert_contains("saved · not asked");
    s.assert_not_contains("waiting…");
    s.keys("M<Enter><Esc>");
    s.assert_row_contains(0, "[mcp: on]");
    let r = s.mcp_call("get_questions", j(r"{}"));
    check(&r.tool_result(), &j(r#"{"questions": []}"#), "result");
    s.keys("K");
    s.keys("a");
    s.assert_row_contains(-1, "question queued");
    s.assert_contains("─ answer · agent · waiting… ─");
    s.assert_contains(" ⠿ waiting for agent…");
    s.assert_not_contains("saved · not asked");
    let r = s.mcp_call("get_questions", j(r"{}"));
    check(
        &r.tool_result(),
        &j(r#"{"questions": [{"thread_id": "q1", "turn": 1, "follow_up": false, "preview": "why?"}]}"#),
        "result",
    );
}

/// F-ASK-02: a on a human thread whose follow-up was cancelled notes can't retry a follow-up yet
#[test]
fn f_ask_02_cant_retry() {
    let mut s = Sim::builder().config_json(json!({"mcp": {"autostart": true}})).build();
    let p_poll = s.http_start(Http::tool("next_question", j(r#"{"wait_seconds": 60}"#)));
    s.keys("a");
    s.keys("why?<Enter>");
    s.http_await(&p_poll);
    s.mcp_call("answer", j(r#"{"thread_id": "q1", "text": "because"}"#));
    s.keys("K");
    s.keys("a");
    s.keys("and?<Enter>");
    s.assert_row_contains(-1, "follow-up queued");
    // stop MCP; the queued follow-up becomes cancelled
    s.keys("M<Enter><Esc>");
    s.assert_row_contains(0, "[mcp: off]");
    s.assert_contains("follow-up: and?");
    s.assert_contains("─ answer · agent · cancelled ─");
    s.assert_contains("MCP stopped");
    // restart MCP so the MCP-off note cannot be the reason
    s.keys("M<Enter><Esc>");
    s.assert_row_contains(0, "[mcp: on]");
    s.keys("K");
    s.keys("a");
    s.assert_row_contains(-1, "can't retry a follow-up yet");
    s.assert_not_contains("enter send");
    let r = s.mcp_call("get_questions", j(r"{}"));
    check(&r.tool_result(), &j(r#"{"questions": []}"#), "result");
}

/// F-ASK-02: a on an answered question opens the follow-up editor
#[test]
fn f_ask_02_done_followup() {
    let mut s = Sim::builder().config_json(json!({"mcp": {"autostart": true}})).build();
    let p_poll = s.http_start(Http::tool("next_question", j(r#"{"wait_seconds": 60}"#)));
    s.keys("a");
    s.keys("why?<Enter>");
    let r = s.http_await(&p_poll);
    check(&r.tool_result(), &j(r#"{"status": "question", "thread_id": "q1"}"#), "result");
    let r = s.mcp_call("answer", j(r#"{"thread_id": "q1", "text": "because"}"#));
    check(&r.tool_result(), &j(r#"{"ok": true}"#), "result");
    s.keys("K");
    s.assert_contains("▸ sent  line L2");
    s.assert_contains("─ answer · agent · done ─");
    s.assert_contains("because");
    s.assert_contains("a follow up");
    s.assert_not_contains("enter send");
    s.keys("a");
    s.assert_contains("enter send  esc cancel");
    s.assert_not_contains("tab save/ask");
    s.assert_matches("^│ follow-up +│$");
    s.keys("<Esc>");
    s.assert_contains("▸ sent  line L2");
    s.assert_not_contains("enter send");
    s.assert_not_contains("│ follow-up");
    let r = s.mcp_call("get_questions", j(r"{}"));
    check(&r.tool_result(), &j(r#"{"questions": []}"#), "result");
}

/// F-ASK-02: a on a one-turn question cancelled by MCP stop re-asks it
#[test]
fn f_ask_02_retry_cancelled() {
    let mut s = Sim::builder().config_json(json!({"mcp": {"autostart": true}})).build();
    s.keys("a");
    s.keys("why?<Enter>");
    let r = s.mcp_call("next_question", j(r#"{"wait_seconds": 5}"#));
    check(&r.tool_result(), &j(r#"{"status": "question", "thread_id": "q1", "turn": 1}"#), "result");
    s.keys("M<Enter><Esc>");
    s.assert_row_contains(0, "[mcp: off]");
    s.assert_contains("─ answer · agent · cancelled ─");
    s.assert_contains("MCP stopped");
    s.keys("M<Enter><Esc>");
    s.assert_row_contains(0, "[mcp: on]");
    s.keys("K");
    s.keys("a");
    s.assert_row_contains(-1, "question queued");
    s.assert_contains("· waiting… ─");
    s.assert_not_contains("MCP stopped");
    let r = s.mcp_call("next_question", j(r#"{"wait_seconds": 5}"#));
    check(
        &r.tool_result(),
        &j(
            r#"{"status": "question", "thread_id": "q1", "turn": 1, "follow_up": false, "question": {"$matches": "^why\\?\\n\\nFile: README\\.md"}}"#,
        ),
        "result",
    );
}

/// F-ASK-02: a on a pending or streaming question notes still waiting and does not requeue
#[test]
fn f_ask_02_waiting() {
    let mut s = Sim::builder().config_json(json!({"mcp": {"autostart": true}})).build();
    s.assert_row_contains(0, "[mcp: on]");
    s.keys("a");
    s.keys("why?<Enter>");
    s.keys("K");
    s.assert_contains("▸ sent  line L2");
    s.assert_contains("· waiting… ─");
    // pending
    s.keys("a");
    s.assert_row_contains(-1, "still waiting for the agent");
    let r = s.mcp_call("get_questions", j(r"{}"));
    check(&r.tool_result(), &j(r#"{"questions": [{"thread_id": "q1", "turn": 1}]}"#), "result");
    let r = s.mcp_call("next_question", j(r#"{"wait_seconds": 5}"#));
    check(&r.tool_result(), &j(r#"{"status": "question", "thread_id": "q1", "turn": 1}"#), "result");
    s.assert_contains("· streaming… ─");
    s.assert_not_contains("· waiting… ─");
    // other note in between so the next one is fresh
    s.keys("A");
    s.assert_row_contains(-1, "nothing to ask");
    // streaming
    s.keys("a");
    s.assert_row_contains(-1, "still waiting for the agent");
    s.assert_not_contains("follow-up");
    let r = s.mcp_call("get_questions", j(r"{}"));
    check(&r.tool_result(), &j(r#"{"questions": []}"#), "result");
}

/// F-ASK-03: sending a follow-up scrolls the focused thread to its bottom
#[test]
fn f_ask_03_follows_bottom() {
    let mut s = Sim::builder().config_json(json!({"mcp": {"autostart": true}})).size(60, 16).build();
    s.keys("a");
    s.keys("why?<Enter>");
    s.mcp_call("next_question", j(r#"{"wait_seconds": 5}"#));
    s.mcp_call("answer", j(r#"{"thread_id": "q1", "text": "l01\nl02\nl03\nl04\nl05\nl06\nl07\nl08\nl09\nl10\nl11\nl12\nl13\nl14\nl15\nl16\nl17\nl18\nl19\nl20"}"#));
    s.keys("K");
    s.assert_row_contains(4, "↕ 1-8/22");
    s.assert_contains("┃ why? ");
    s.assert_not_contains("┃ l20 ");
    s.keys("a");
    s.keys("more<Enter>");
    s.assert_row_contains(4, "↕ 18-25/25");
    s.assert_row_contains(-1, "follow-up queued");
    s.assert_contains("┃ l20 ");
    s.assert_contains("┃ follow-up: more ");
    s.assert_contains("┃ ⠿ waiting for agent… ");
    s.assert_not_contains("┃ why? ");
}

/// F-ASK-03: follow-up Enter queues turn 2, closes the editor, shows the follow-up line; agent gets it with previous turns
#[test]
fn f_ask_03_followup() {
    let mut s = Sim::builder().config_json(json!({"mcp": {"autostart": true}})).build();
    let p_poll = s.http_start(Http::tool("next_question", j(r#"{"wait_seconds": 60}"#)));
    s.keys("a");
    s.keys("why?<Enter>");
    let r = s.http_await(&p_poll);
    check(&r.tool_result(), &j(r#"{"thread_id": "q1", "turn": 1}"#), "result");
    s.mcp_call("answer", j(r#"{"thread_id": "q1", "text": "because"}"#));
    s.keys("K");
    s.keys("a");
    s.keys("and?");
    s.assert_contains("│ follow-up ");
    s.assert_contains(" and?");
    s.assert_contains("enter send  esc cancel");
    s.assert_not_contains("follow-up: and?");
    s.keys("<Enter>");
    s.assert_row_contains(-1, "follow-up queued");
    s.assert_not_contains("enter send");
    s.assert_not_contains("│ follow-up ");
    s.assert_matches(
        r"because +┃\n┃ follow-up: and\? +┃\n┃─ answer · agent · waiting… ─+┃\n┃ ⠿ waiting for agent… +┃",
    );
    let r = s.mcp_call("get_questions", j(r"{}"));
    check(
        &r.tool_result(),
        &j(r#"{"questions": [{"thread_id": "q1", "turn": 2, "follow_up": true}]}"#),
        "result",
    );
    let r = s.mcp_call("next_question", j(r#"{"wait_seconds": 5}"#));
    check(
        &r.tool_result(),
        &j(
            r#"{"status": "question", "thread_id": "q1", "turn": 2, "follow_up": true, "previous": [{"turn": 1, "question": "why?", "answer": "because"}], "question": "Follow-up to your earlier answer (thread q1, turn 2): and?"}"#,
        ),
        "result",
    );
    s.assert_matches(r"follow-up: and\? +┃\n┃─ answer · agent · streaming… ─+┃");
}

/// F-ASK-03: follow-up Enter with MCP off keeps the editor open and notes MCP is off
#[test]
fn f_ask_03_mcp_off() {
    let mut s = Sim::builder().config_json(json!({"mcp": {"autostart": true}})).build();
    let p_poll = s.http_start(Http::tool("next_question", j(r#"{"wait_seconds": 60}"#)));
    s.keys("a");
    s.keys("why?<Enter>");
    s.http_await(&p_poll);
    s.mcp_call("answer", j(r#"{"thread_id": "q1", "text": "because"}"#));
    s.keys("M<Enter><Esc>");
    s.assert_row_contains(0, "[mcp: off]");
    s.assert_contains("· done ─");
    s.keys("K");
    s.keys("a");
    s.keys("and?<Enter>");
    s.assert_row_contains(-1, "MCP is off (M to start)");
    s.assert_contains("│ follow-up ");
    s.assert_contains(" and?");
    s.assert_contains("enter send  esc cancel");
    s.assert_not_contains("follow-up: and?");
}

/// F-ASK-03: reply to an agent note is queued as follow-up turn 2 carrying the note context
#[test]
fn f_ask_03_note_reply() {
    let mut s = Sim::builder().config_json(json!({"mcp": {"autostart": true}})).build();
    s.mcp_call("annotate", j(r#"{"file": "README.md", "line": 2, "text": "check this", "number": 3}"#));
    s.keys("J");
    s.keys("a");
    s.keys("ok why<Enter>");
    s.assert_row_contains(-1, "follow-up queued");
    s.assert_not_contains("enter send");
    s.assert_matches(r"check this +┃\n┃ follow-up: ok why +┃\n┃─ answer · agent · waiting… ─+┃");
    let r = s.mcp_call("next_question", j(r#"{"wait_seconds": 5}"#));
    check(
        &r.tool_result(),
        &j(
            r#"{"status": "question", "thread_id": "q1", "turn": 2, "follow_up": true, "previous": [{"turn": 1, "question": "(note you added with annotate)", "answer": "check this"}], "question": "Follow-up to your earlier answer (thread q1, turn 2): ok why\n\nReply to your annotate note #3:\nFile: README.md\nSide: new\nLine: 2\n\nYour note:\ncheck this"}"#,
        ),
        "result",
    );
    let r = s.mcp_call("answer", j(r#"{"thread_id": "q1", "text": "see above"}"#));
    check(&r.tool_result(), &j(r#"{"ok": true}"#), "result");
    s.assert_matches(r"follow-up: ok why +┃\n┃─ answer · agent · done ─+┃\n┃ see above +┃");
}

/// F-ASK-04: A on a focused comment queues every askable comment of the current file only
#[test]
fn f_ask_04_all() {
    let mut s = Sim::builder().build();
    s.assert_row_matches(0, r"\[mcp: off\] \[1/4\] .* README\.md ");
    // q1 saved on L2 (old), q2 saved on L3
    s.keys("a");
    s.keys("s one<Enter>");
    s.keys("jj");
    s.keys("a");
    s.keys("s two<Enter>");
    s.assert_row_contains(-1, "question saved (2)");
    // q3 saved in another file
    s.keys("<Tab>");
    s.assert_row_contains(0, "[2/4]");
    s.keys("a");
    s.keys("other file<Enter>");
    s.assert_row_contains(-1, "question saved (3)");
    s.keys("<S-Tab>");
    s.assert_row_matches(0, r"\[1/4\] .* README\.md ");
    s.keys("M<Enter><Esc>");
    s.assert_row_contains(0, "[mcp: on]");
    // q4 asked directly on L2 (new)
    s.keys("gjjj");
    s.assert_row_contains(0, "[cursor L2:C1]");
    s.keys("a");
    s.keys("asked<Enter>");
    s.assert_row_contains(-1, "question sent to agent");
    let r = s.mcp_call("get_questions", j(r"{}"));
    check(&r.tool_result(), &j(r#"{"questions": [{"thread_id": "q4"}]}"#), "result");
    s.keys("J");
    s.assert_contains("▸ sent  line L2");
    s.keys("A");
    s.assert_row_contains(-1, "queued 2 questions");
    s.assert_not_contains("saved · not asked");
    let r = s.mcp_call("get_questions", j(r"{}"));
    check(
        &r.tool_result(),
        &j(
            r#"{"questions": [{"thread_id": {"$matches": "^q[124]$"}, "turn": 1, "follow_up": false}, {"thread_id": {"$matches": "^q[124]$"}, "turn": 1, "follow_up": false}, {"thread_id": {"$matches": "^q[124]$"}, "turn": 1, "follow_up": false}]}"#,
        ),
        "result",
    );
    check(&r.json(), &j(r#"{"result": {"content": [{"text": {"$not_contains": "\"q3\""}}]}}"#), "body");
    let r = s.mcp_call("get_questions", j(r"{}"));
    check(
        &r.tool_result(),
        &j(r#"{"questions": {"$contains": {"thread_id": "q1", "preview": "s one"}}}"#),
        "result",
    );
    let r = s.mcp_call("get_questions", j(r"{}"));
    check(
        &r.tool_result(),
        &j(r#"{"questions": {"$contains": {"thread_id": "q2", "preview": "s two"}}}"#),
        "result",
    );
    // the other file's comment is still only saved
    s.keys("<Esc><Tab>");
    s.assert_row_contains(0, "[2/4]");
    s.assert_contains("saved · not asked");
}

/// F-ASK-04: A with MCP off notes MCP is off and queues nothing
#[test]
fn f_ask_04_mcp_off() {
    let mut s = Sim::builder().build();
    s.keys("a");
    s.keys("s one<Enter>");
    s.keys("J");
    s.keys("A");
    s.assert_row_contains(-1, "MCP is off (M to start)");
    s.assert_contains("saved · not asked");
    s.keys("M<Enter><Esc>");
    s.assert_row_contains(0, "[mcp: on]");
    s.assert_contains("saved · not asked");
    let r = s.mcp_call("get_questions", j(r"{}"));
    check(&r.tool_result(), &j(r#"{"questions": []}"#), "result");
}

/// F-ASK-04: A with nothing askable in the file notes nothing to ask
#[test]
fn f_ask_04_nothing() {
    let mut s = Sim::builder().config_json(json!({"mcp": {"autostart": true}})).build();
    s.keys("a");
    s.keys("asked<Enter>");
    let r = s.mcp_call("next_question", j(r#"{"wait_seconds": 5}"#));
    check(&r.tool_result(), &j(r#"{"thread_id": "q1"}"#), "result");
    s.mcp_call("answer", j(r#"{"thread_id": "q1", "text": "done it"}"#));
    s.keys("K");
    s.assert_contains("▸ sent  line L2");
    s.assert_contains("done it");
    s.keys("A");
    s.assert_row_contains(-1, "nothing to ask");
    let r = s.mcp_call("get_questions", j(r"{}"));
    check(&r.tool_result(), &j(r#"{"questions": []}"#), "result");
}

/// F-ASK-04: A with one askable comment notes queued 1 question
#[test]
fn f_ask_04_one() {
    let mut s = Sim::builder().build();
    s.keys("a");
    s.keys("lonely<Enter>");
    s.assert_row_contains(-1, "question saved (1)");
    s.keys("M<Enter><Esc>");
    s.assert_row_contains(0, "[mcp: on]");
    s.keys("J");
    s.keys("A");
    s.assert_row_contains(-1, "queued 1 question");
    s.assert_contains("· waiting… ─");
    s.assert_not_contains("queued 1 questions");
    s.assert_not_contains("saved · not asked");
    let r = s.mcp_call("get_questions", j(r"{}"));
    check(
        &r.tool_result(),
        &j(r#"{"questions": [{"thread_id": "q1", "turn": 1, "follow_up": false, "preview": "lonely"}]}"#),
        "result",
    );
}

/// F-ASK-04: A without a focused comment does nothing; with focus it queues
#[test]
fn f_ask_04_unfocused() {
    let mut s = Sim::builder().config_json(json!({"mcp": {"autostart": true}})).build();
    s.keys("a");
    s.keys("<Tab>s one<Enter>");
    s.assert_row_contains(-1, "question saved (1)");
    s.assert_not_contains("▸ sent");
    s.keys("A");
    s.assert_row_contains(-1, "question saved (1)");
    s.assert_contains("saved · not asked");
    s.assert_not_contains("queued");
    s.assert_not_contains("▸ sent");
    let r = s.mcp_call("get_questions", j(r"{}"));
    check(&r.tool_result(), &j(r#"{"questions": []}"#), "result");
    s.keys("J");
    s.keys("A");
    s.assert_row_contains(-1, "queued 1 question");
    s.assert_not_contains("saved · not asked");
    let r = s.mcp_call("get_questions", j(r"{}"));
    check(&r.tool_result(), &j(r#"{"questions": [{"thread_id": "q1", "preview": "s one"}]}"#), "result");
}

/// F-ASK-05: client without a usable name (non-string clientInfo.name) is shown as agent in the divider
#[test]
fn f_ask_05_agent_fallback() {
    let mut s = Sim::builder().config_json(json!({"mcp": {"autostart": true}})).size(40, 30).build();
    s.keys("a");
    s.keys("why?<Enter>");
    let r = s.http(Http::json(&j(r#"[{"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "clientInfo": {"name": 42}}}, {"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "next_question", "arguments": {"wait_seconds": 5}}}]"#)));
    check(
        &r.json(),
        &j(
            r#"[{"id": 1}, {"id": 2, "result": {"content": [{"text": {"$contains": "\"thread_id\":\"q1\""}}]}}]"#,
        ),
        "body",
    );
    s.assert_not_contains("unknown");
    s.assert_not_contains("waiting…");
    s.assert_matches("^│─ answer · agent · streaming… ─{7}│$");
    s.mcp_call("answer", j(r#"{"thread_id": "q1", "text": "ok"}"#));
    s.assert_not_contains("unknown");
    s.assert_matches(r"^│─ answer · agent · done ─{13}│\n│ ok +│$");
}

/// F-ASK-05: MCP stop turns pending and streaming answers into cancelled with text MCP stopped; done answers stay
#[test]
fn f_ask_05_cancelled() {
    let mut s = Sim::builder().config_json(json!({"mcp": {"autostart": true}})).size(40, 40).build();
    s.keys("a");
    s.keys("one<Enter>");
    s.keys("j");
    s.keys("a");
    s.keys("two<Enter>");
    s.keys("j");
    s.keys("a");
    s.keys("three<Enter>");
    let r = s.mcp_call("next_question", j(r#"{"wait_seconds": 5}"#));
    check(&r.tool_result(), &j(r#"{"thread_id": "q1"}"#), "result");
    s.mcp_call("answer", j(r#"{"thread_id": "q1", "text": "first answer"}"#));
    let r = s.mcp_call("next_question", j(r#"{"wait_seconds": 5}"#));
    check(&r.tool_result(), &j(r#"{"thread_id": "q2"}"#), "result");
    s.assert_matches(r"│ one +│\n│─ answer · agent · done ─+│\n│ first answer +│");
    s.assert_matches(r"│ two +│\n│─ answer · agent · streaming… ─+│\n│ . agent working… +│");
    s.assert_matches(r"│ three +│\n│─ answer · agent · waiting… ─+│\n│ ⠿ waiting for agent… +│");
    s.keys("M<Enter><Esc>");
    s.assert_not_contains("streaming…");
    s.assert_not_contains("waiting…");
    s.assert_not_contains("agent working");
    s.assert_not_contains("waiting for agent");
    s.assert_matches(r"│ one +│\n│─ answer · agent · done ─{13}│\n│ first answer +│");
    s.assert_matches(r"│ two +│\n│─ answer · agent · cancelled ─{8}│\n│ MCP stopped +│\n╰");
    s.assert_matches(r"│ three +│\n│─ answer · agent · cancelled ─{8}│\n│ MCP stopped +│\n╰");
}

/// F-ASK-05: answer divider shows client name and status waiting, streaming, done; padded with rules to width-2 in accent
#[test]
fn f_ask_05_divider_status() {
    let mut s = Sim::builder().config_json(json!({"mcp": {"autostart": true}})).size(40, 30).build();
    s.keys("a");
    s.keys("why?<Enter>");
    // pending, no client yet
    s.assert_matches(r"^│ why\? +│\n│─ answer · agent · waiting… ─{9}│\n│ ⠿ waiting for agent… +│\n╰");
    s.assert_text_cell("answer", 0, C::new().fg("#cb4b16"));
    s.assert_text_cell("─ answer", 0, C::new().fg("#cb4b16"));
    s.assert_text_cell("waiting…", 9, C::new().ch('─').fg("#cb4b16"));
    // initialize names the client; next_question in the same batch uses that session
    let r = s.http(Http::json(&j(r#"[{"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "clientInfo": {"name": "tester", "version": "1.2"}}}, {"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "next_question", "arguments": {"wait_seconds": 5}}}]"#)));
    check(
        &r.json(),
        &j(
            r#"[{"id": 1, "result": {"serverInfo": {"name": "xplain"}}}, {"id": 2, "result": {"content": [{"text": {"$matches": "\"status\":\"question\".*\"thread_id\":\"q1\""}}]}}]"#,
        ),
        "body",
    );
    s.assert_not_contains("waiting for agent");
    s.assert_not_contains("waiting…");
    s.assert_matches(
        r"^│ why\? +│\n│─ answer · tester · streaming… ─{6}│\n│ [⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏] agent working… +│\n╰",
    );
    let r = s.mcp_call("answer", j(r#"{"thread_id": "q1", "text": "all good"}"#));
    check(&r.tool_result(), &j(r#"{"ok": true}"#), "result");
    s.assert_not_contains("agent working");
    s.assert_not_contains("streaming…");
    s.assert_matches(r"^│ why\? +│\n│─ answer · tester · done ─{12}│\n│ all good +│\n╰");
    s.assert_text_cell("answer", 0, C::new().fg("#cb4b16"));
}

/// F-ASK-05: thread body order - message, kept answers, follow-up line (accent, wrapped with its prefix), its answer
#[test]
fn f_ask_05_turns() {
    let mut s = Sim::builder().config_json(json!({"mcp": {"autostart": true}})).size(40, 40).build();
    s.keys("a");
    s.keys("why?<Enter>");
    s.mcp_call("next_question", j(r#"{"wait_seconds": 5}"#));
    s.mcp_call("answer", j(r#"{"thread_id": "q1", "text": "ans one"}"#));
    s.mcp_call("answer", j(r#"{"thread_id": "q1", "text": "ans two"}"#));
    s.keys("K");
    s.keys("a");
    s.keys("aaaa bbbb cccc dddd eeee ffff gggg<Enter>");
    s.assert_row_contains(-1, "follow-up queued");
    let r = s.mcp_call("next_question", j(r#"{"wait_seconds": 5}"#));
    check(
        &r.tool_result(),
        &j(r#"{"thread_id": "q1", "turn": 2, "previous": [{"turn": 1, "answer": "ans one\n\nans two"}]}"#),
        "result",
    );
    s.mcp_call("answer", j(r#"{"thread_id": "q1", "text": "ans three"}"#));
    s.assert_matches(r"┃ why\? +┃\n┃─ answer · agent · done ─+┃\n┃ ans one +┃\n┃─ answer · agent · done ─+┃\n┃ ans two +┃\n┃ follow-up: aaaa bbbb cccc dddd eeee +┃\n┃ ffff gggg +┃\n┃─ answer · agent · done ─+┃\n┃ ans three +┃\n┃ e edit");
    s.assert_text_cell("follow-up: aaaa", 0, C::new().fg("#cb4b16"));
    s.assert_text_cell("ffff gggg", 0, C::new().fg("#cb4b16"));
}

/// F-ASK-05: message and answer wrap at box width-3 (word break, hard cut, blank lines, tabs, CR, trimmed continuation)
#[test]
fn f_ask_05_wrap() {
    let mut s = Sim::builder().config_json(json!({"mcp": {"autostart": true}})).size(40, 40).build();
    s.keys("a");
    s.keys("aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii<Enter>");
    // 36 columns of text per line (box 39 wide)
    s.assert_matches(r"^│ aaaa bbbb cccc dddd eeee ffff gggg  │\n│ hhhh iiii +│$");
    s.mcp_call("next_question", j(r#"{"wait_seconds": 5}"#));
    s.mcp_call("answer", j(r#"{"thread_id": "q1", "text": "word1 word2 word3 word4 word5 word6      word7\n\nxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\n\ttab\r\nend\n\n\n"}"#));
    s.assert_matches(r"^│─ answer · agent · done ─+│\n│ word1 word2 word3 word4 word5 word6 │\n│ word7 +│\n│ +│\n│ x{36}│\n│ x{14} +│\n│   tab +│\n│ end +│\n╰─+╯$");
}

/// F-ASK-06: fence rules - tilde fences, indented open, close needs same char and length, whitespace only, no close runs to end
#[test]
fn f_ask_06_fences() {
    let mut s = Sim::builder().config_json(json!({"mcp": {"autostart": true}})).size(40, 40).build();
    s.keys("a");
    s.keys("q<Enter>");
    s.mcp_call("next_question", j(r#"{"wait_seconds": 5}"#));
    s.mcp_call("answer", j(r#"{"thread_id": "q1", "text": "p1\n~~~~\nin ``` tilde\n~~~\n~~~~\np2\n  ```py\ncode\n``` x\n~~~\n  ```  \np3\n```\nopen to end"}"#));
    s.keys("K");
    s.assert_not_contains("~~~~");
    s.assert_not_contains("```py");
    s.assert_matches(r"┃ p1 +┃\n┃ \[ copy \] +┃\n┃ │ in ``` tilde +┃\n┃ │ ~~~ +┃\n┃ p2 +┃\n┃ \[ copy \] py +┃\n┃ │ code +┃\n┃ │ ``` x +┃\n┃ │ ~~~ +┃\n┃ p3 +┃\n┃ \[ copy \] +┃\n┃ │ open to end +┃\n┃ e edit");
}

/// F-ASK-06: fenced code in an agent note body renders as a code block
#[test]
fn f_ask_06_note() {
    let mut s = Sim::builder().config_json(json!({"mcp": {"autostart": true}})).build();
    let r = s.mcp_call(
        "annotate",
        j(r#"{"file": "README.md", "line": 2, "text": "try this:\n```sh\necho hi\n```"}"#),
    );
    check(&r.tool_result(), &j(r#"{"ok": true}"#), "result");
    s.assert_not_contains("```");
    s.assert_matches(r"agent note L2 +│\n│ try this: +│\n│ \[ copy \] sh +│\n│ │ echo hi +│\n╰");
}

/// F-ASK-06: fenced code in an answer - fences hidden, copy button with lang, gutter lines, long lines split, tabs, empty lines
#[test]
fn f_ask_06_render() {
    let mut s = Sim::builder().config_json(json!({"mcp": {"autostart": true}})).size(40, 40).build();
    s.keys("a");
    s.keys("q<Enter>");
    s.mcp_call("next_question", j(r#"{"wait_seconds": 5}"#));
    s.mcp_call("answer", j(r#"{"thread_id": "q1", "text": "intro\n```ts title\nconst a = 1;\n\tif (x) {}\n\nabcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJ\n```\noutro"}"#));
    s.keys("K");
    // code width = box width-5 = 34
    s.assert_not_contains("```");
    s.assert_not_contains("title");
    s.assert_matches(r"┃─ answer · agent · done ─+┃\n┃ intro +┃\n┃ \[ copy \] ts +┃\n┃ │ const a = 1; +┃\n┃ │   if \(x\) \{\} +┃\n┃ │ +┃\n┃ │ abcdefghijklmnopqrstuvwxyz01234567┃\n┃ │ 89ABCDEFGHIJ +┃\n┃ outro +┃");
    s.assert_text_cell("[ copy ]", 0, C::new().fg("#cb4b16"));
    s.assert_text_cell("[ copy ] ts", 9, C::new().ch('t').fg("#586e75"));
    s.assert_text_cell("│ const", 0, C::new().ch('│').fg("#586e75"));
}

/// F-ASK-07: a live thread follows its bottom; k scrolls away, G returns; answer arrival keeps the bottom, else the window start
#[test]
fn f_ask_07_follow() {
    let mut s = Sim::builder().config_json(json!({"mcp": {"autostart": true}})).size(60, 16).build();
    s.keys("a");
    s.keys("why?<Enter>");
    s.mcp_call("next_question", j(r#"{"wait_seconds": 5}"#));
    s.mcp_call("answer", j(r#"{"thread_id": "q1", "text": "l01\nl02\nl03\nl04\nl05\nl06\nl07\nl08\nl09\nl10\nl11\nl12\nl13\nl14\nl15\nl16\nl17\nl18\nl19\nl20"}"#));
    s.keys("K");
    s.assert_row_contains(4, "↕ 1-8/22");
    s.keys("a");
    s.keys("more<Enter>");
    // live follow-up turn, view at the bottom
    s.assert_row_contains(4, "↕ 18-25/25");
    s.assert_contains("┃ follow-up: more ");
    s.assert_contains("┃ ⠿ waiting for agent… ");
    let r = s.mcp_call("next_question", j(r#"{"wait_seconds": 5}"#));
    check(&r.tool_result(), &j(r#"{"thread_id": "q1", "turn": 2}"#), "result");
    s.assert_row_contains(4, "↕ 18-25/25");
    s.assert_contains("· streaming… ─");
    s.assert_contains("agent working…");
    s.keys("k");
    s.assert_row_contains(4, "↕ 17-24/25");
    s.assert_not_contains("agent working…");
    s.keys("G");
    s.assert_row_contains(4, "↕ 18-25/25");
    s.assert_contains("agent working…");
    // done answer arrives while the window shows the last line (after G) -> window ends at the new last line
    s.mcp_call("answer", j(r#"{"thread_id": "q1", "text": "a1\na2\na3\na4"}"#));
    s.assert_row_contains(4, "↕ 21-28/28");
    s.assert_contains("· done ─");
    s.assert_contains("a4");
    s.assert_not_contains("agent working…");
    // next follow-up, followed without any scrolling; its answer keeps the bottom too
    s.keys("a");
    s.keys("again<Enter>");
    s.assert_row_contains(4, "↕ 24-31/31");
    s.assert_contains("┃ ⠿ waiting for agent… ");
    let r = s.mcp_call("next_question", j(r#"{"wait_seconds": 5}"#));
    check(&r.tool_result(), &j(r#"{"thread_id": "q1", "turn": 3}"#), "result");
    s.mcp_call("answer", j(r#"{"thread_id": "q1", "text": "b1\nb2\nb3"}"#));
    s.assert_row_contains(4, "↕ 26-33/33");
    s.assert_contains("b3");
    // window scrolled up (last line not shown) when an answer arrives -> window start kept
    s.keys("kk");
    s.assert_row_contains(4, "↕ 24-31/33");
    s.mcp_call("answer", j(r#"{"thread_id": "q1", "text": "c1\nc2"}"#));
    s.assert_row_contains(4, "↕ 24-31/36");
}

/// F-ASK-07: j/k/d/u/g/G scroll a focused thread body that exceeds its window
#[test]
fn f_ask_07_scroll() {
    let mut s = Sim::builder().config_json(json!({"mcp": {"autostart": true}})).size(60, 16).build();
    s.keys("a");
    s.keys("why?<Enter>");
    s.mcp_call("next_question", j(r#"{"wait_seconds": 5}"#));
    s.mcp_call("answer", j(r#"{"thread_id": "q1", "text": "l01\nl02\nl03\nl04\nl05\nl06\nl07\nl08\nl09\nl10\nl11\nl12\nl13\nl14\nl15\nl16\nl17\nl18\nl19\nl20"}"#));
    s.keys("K");
    // body = message + divider + 20 answer lines = 22; window 8
    s.assert_row_matches(4, "▸ sent  line L2  ↕ 1-8/22 ");
    s.assert_contains("┃ why? ");
    s.assert_contains("┃ l06 ");
    s.assert_contains("j/k scroll");
    s.assert_not_contains("┃ l07 ");
    s.keys("j");
    s.assert_row_contains(4, "↕ 2-9/22");
    s.assert_contains("┃ l07 ");
    s.assert_not_contains("┃ why? ");
    s.keys("k");
    s.assert_row_contains(4, "↕ 1-8/22");
    s.assert_contains("┃ why? ");
    // k at top stays
    s.keys("k");
    s.assert_row_contains(4, "↕ 1-8/22");
    // d = +floor(8/2)
    s.keys("d");
    s.assert_row_contains(4, "↕ 5-12/22");
    s.assert_contains("┃ l03 ");
    s.assert_contains("┃ l10 ");
    s.assert_not_contains("┃ l02 ");
    s.assert_not_contains("┃ l11 ");
    s.keys("d");
    s.assert_row_contains(4, "↕ 9-16/22");
    s.keys("u");
    s.assert_row_contains(4, "↕ 5-12/22");
    s.keys("G");
    s.assert_row_contains(4, "↕ 15-22/22");
    s.assert_contains("┃ l13 ");
    s.assert_contains("┃ l20 ");
    s.assert_not_contains("┃ l12 ");
    // j at bottom stays
    s.keys("j");
    s.assert_row_contains(4, "↕ 15-22/22");
    s.keys("g");
    s.assert_row_contains(4, "↕ 1-8/22");
    s.assert_contains("┃ why? ");
    // scroll keys keep focus (cursor does not move away)
    s.assert_row_contains(2, "-▶hello");
    s.assert_contains("▸ sent  line L2");
}

/// F-ASK-08: Down/Up pick code buttons with wrap, Enter copies raw block via OSC 52 with line count note, Esc unpicks
#[test]
fn f_ask_08_copy() {
    let mut s = Sim::builder().config_json(json!({"mcp": {"autostart": true}})).size(60, 40).build();
    s.keys("a");
    s.keys("q<Enter>");
    s.mcp_call("next_question", j(r#"{"wait_seconds": 5}"#));
    s.mcp_call("answer", j(r#"{"thread_id": "q1", "text": "A:\n```ts\nconst a = 1;\n\tif (x) {}\n\nlast\n```\nB:\n```sh\necho one\n```\nC:\n```\n```"}"#));
    s.keys("K");
    s.assert_contains("┃ [ copy ] ts ");
    s.assert_contains("┃ [ copy ] sh ");
    s.assert_contains("↑/↓ code");
    s.assert_not_contains("enter copy  esc cancel");
    // first Down picks the first button
    s.keys("<Down>");
    s.assert_not_contains("[ copy ] sh  enter copy");
    s.assert_matches(r"┃ \[ copy \] ts  enter copy  esc cancel +┃");
    s.assert_text_cell("[ copy ] ts", 0, C::new().bold(true).reverse(true));
    s.assert_text_cell("[ copy ] sh", 0, C::new().reverse(false));
    s.assert_text_cell("enter copy", 0, C::new().fg("#586e75"));
    s.assert_text_cell("│ const", 0, C::new().ch('│').fg("#cb4b16"));
    s.assert_text_cell("│ echo", 0, C::new().ch('│').fg("#586e75"));
    s.keys("<Enter>");
    assert_eq!(s.clipboard_all().len(), 1);
    assert_eq!(s.clipboard(), Some("const a = 1;\n\tif (x) {}\n\nlast"));
    s.assert_row_contains(-1, "copied 4 lines");
    s.keys("<Down>");
    s.assert_not_contains("[ copy ] ts  enter copy");
    s.assert_matches(r"┃ \[ copy \] sh  enter copy  esc cancel +┃");
    s.assert_text_cell("│ echo", 0, C::new().ch('│').fg("#cb4b16"));
    s.assert_text_cell("│ const", 0, C::new().ch('│').fg("#586e75"));
    s.keys("<Enter>");
    assert_eq!(s.clipboard_all().len(), 2);
    assert_eq!(s.clipboard(), Some("echo one"));
    s.assert_row_contains(-1, "copied 1 line");
    s.keys("<Down>");
    s.assert_matches(r"┃ C: +┃\n┃ \[ copy \]  enter copy  esc cancel +┃");
    // empty block copies empty text, counted as 1 line
    s.keys("<Enter>");
    assert_eq!(s.clipboard_all().len(), 3);
    assert_eq!(s.clipboard(), Some(""));
    s.assert_row_contains(-1, "copied 1 line");
    // Down wraps from last to first
    s.keys("<Down>");
    s.assert_contains("[ copy ] ts  enter copy");
    // Up wraps from first to last
    s.keys("<Up>");
    s.assert_not_contains("[ copy ] ts  enter copy");
    s.assert_matches(r"┃ \[ copy \]  enter copy  esc cancel +┃");
    s.keys("<Up>");
    s.assert_contains("[ copy ] sh  enter copy");
    s.keys("<Esc>");
    s.assert_contains("▸ sent  line L2");
    s.assert_not_contains("enter copy  esc cancel");
    s.assert_text_cell("[ copy ] sh", 0, C::new().reverse(false));
    s.assert_text_cell("│ echo", 0, C::new().ch('│').fg("#586e75"));
}

/// F-ASK-08: picking a button outside the thread window scrolls it into view
#[test]
fn f_ask_08_scroll_into_view() {
    let mut s = Sim::builder().config_json(json!({"mcp": {"autostart": true}})).size(60, 16).build();
    s.keys("a");
    s.keys("q<Enter>");
    s.mcp_call("next_question", j(r#"{"wait_seconds": 5}"#));
    s.mcp_call("answer", j(r#"{"thread_id": "q1", "text": "l01\nl02\nl03\nl04\nl05\nl06\nl07\nl08\nl09\nl10\nl11\nl12\nl13\nl14\nl15\nl16\nl17\nl18\nl19\nl20\n```sh\nbottom code\n```"}"#));
    s.keys("K");
    s.assert_row_contains(4, "↕ 1-8/");
    s.assert_not_contains("[ copy ]");
    s.assert_not_contains("bottom code");
    s.keys("<Down>");
    assert!(!s.row(4).contains("↕ 1-8/"), "row 4 contains {:?}\n{}", "↕ 1-8/", s.dump());
    s.assert_contains("[ copy ] sh  enter copy");
    s.keys("<Enter>");
    assert_eq!(s.clipboard_all().len(), 1);
    assert_eq!(s.clipboard(), Some("bottom code"));
}

/// F-ASK-08: buttons are numbered across the whole thread (note, then answer); follow-up lines are never code blocks
#[test]
fn f_ask_08_thread() {
    let mut s = Sim::builder().config_json(json!({"mcp": {"autostart": true}})).size(60, 40).build();
    s.mcp_call("annotate", j(r#"{"file": "README.md", "line": 2, "text": "note:\n```ts\nfrom note\n```"}"#));
    s.keys("J");
    s.keys("a");
    s.keys("```sh ls<Enter>");
    s.assert_row_contains(-1, "follow-up queued");
    s.assert_not_contains("[ copy ] sh");
    s.assert_matches("┃ follow-up: ```sh ls +┃");
    // only the note block is pickable; Down twice wraps onto it again
    s.keys("<Down>");
    s.assert_contains("[ copy ] ts  enter copy");
    s.keys("<Down>");
    s.assert_contains("[ copy ] ts  enter copy");
    s.keys("<Enter>");
    assert_eq!(s.clipboard_all().len(), 1);
    assert_eq!(s.clipboard(), Some("from note"));
    s.keys("<Esc>");
    let r = s.mcp_call("next_question", j(r#"{"wait_seconds": 5}"#));
    check(&r.tool_result(), &j(r#"{"thread_id": "q1", "turn": 2}"#), "result");
    s.mcp_call("answer", j(r#"{"thread_id": "q1", "text": "answer:\n```py\nfrom answer\n```"}"#));
    s.assert_contains("┃ [ copy ] ts ");
    s.assert_contains("┃ [ copy ] py ");
    s.assert_not_contains("enter copy");
    s.keys("<Down>");
    s.assert_contains("[ copy ] ts  enter copy");
    s.assert_not_contains("[ copy ] py  enter copy");
    s.keys("<Down>");
    s.assert_contains("[ copy ] py  enter copy");
    s.assert_not_contains("[ copy ] ts  enter copy");
    s.keys("<Enter>");
    assert_eq!(s.clipboard_all().len(), 2);
    assert_eq!(s.clipboard(), Some("from answer"));
    s.assert_row_contains(-1, "copied 1 line");
}

/// F-ASK-08: first Up picks the last button
#[test]
fn f_ask_08_up_first() {
    let mut s = Sim::builder().config_json(json!({"mcp": {"autostart": true}})).size(60, 40).build();
    s.keys("a");
    s.keys("q<Enter>");
    s.mcp_call("next_question", j(r#"{"wait_seconds": 5}"#));
    s.mcp_call(
        "answer",
        j(r#"{"thread_id": "q1", "text": "```ts\none\n```\n```sh\ntwo\n```\n```py\nthree\n```"}"#),
    );
    s.keys("K");
    s.assert_contains("[ copy ] py");
    s.assert_not_contains("enter copy");
    s.keys("<Up>");
    s.assert_contains("[ copy ] py  enter copy");
    s.assert_not_contains("[ copy ] ts  enter copy");
    s.assert_not_contains("[ copy ] sh  enter copy");
    s.keys("<Enter>");
    assert_eq!(s.clipboard_all().len(), 1);
    assert_eq!(s.clipboard(), Some("three"));
}

/// F-ASK-09: answer marks the delivered turn done with its text; a second answer on the done turn is appended
#[test]
fn f_ask_09_arrival() {
    let mut s = Sim::builder().config_json(json!({"mcp": {"autostart": true}})).size(60, 40).build();
    s.keys("a");
    s.keys("why?<Enter>");
    let r = s.mcp_call("next_question", j(r#"{"wait_seconds": 5}"#));
    check(&r.tool_result(), &j(r#"{"thread_id": "q1"}"#), "result");
    s.assert_contains("· streaming… ─");
    s.assert_not_contains("· done");
    s.assert_not_contains("first reply");
    let r = s.mcp_call("answer", j(r#"{"thread_id": "q1", "text": "first reply"}"#));
    check(
        &r.tool_result(),
        &j(r#"{"ok": true, "note": "Answer delivered. Call next_question again immediately."}"#),
        "result",
    );
    s.assert_not_contains("streaming…");
    s.assert_matches(r"│ why\? +│\n│─ answer · agent · done ─+│\n│ first reply +│\n╰");
    let r = s.mcp_call("answer", j(r#"{"thread_id": "q1", "text": "second reply"}"#));
    check(&r.tool_result(), &j(r#"{"ok": true}"#), "result");
    s.assert_matches(r"│ why\? +│\n│─ answer · agent · done ─+│\n│ first reply +│\n│─ answer · agent · done ─+│\n│ second reply +│\n╰");
}

/// F-ASK-09: answer after a follow-up is delivered goes to the latest delivered turn; turn 1 answer untouched
#[test]
fn f_ask_09_latest_turn() {
    let mut s = Sim::builder().config_json(json!({"mcp": {"autostart": true}})).size(60, 40).build();
    s.keys("a");
    s.keys("why?<Enter>");
    s.mcp_call("next_question", j(r#"{"wait_seconds": 5}"#));
    s.mcp_call("answer", j(r#"{"thread_id": "q1", "text": "turn one answer"}"#));
    s.keys("K");
    s.keys("a");
    s.keys("and?<Enter>");
    let r = s.mcp_call("next_question", j(r#"{"wait_seconds": 5}"#));
    check(&r.tool_result(), &j(r#"{"thread_id": "q1", "turn": 2, "follow_up": true}"#), "result");
    s.assert_matches(r"┃ turn one answer +┃\n┃ follow-up: and\? +┃\n┃─ answer · agent · streaming… ─+┃");
    let r = s.mcp_call("answer", j(r#"{"thread_id": "q1", "text": "turn two answer"}"#));
    check(&r.tool_result(), &j(r#"{"ok": true}"#), "result");
    s.assert_not_contains("streaming…");
    s.assert_matches(r"┃ why\? +┃\n┃─ answer · agent · done ─+┃\n┃ turn one answer +┃\n┃ follow-up: and\? +┃\n┃─ answer · agent · done ─+┃\n┃ turn two answer +┃");
}
