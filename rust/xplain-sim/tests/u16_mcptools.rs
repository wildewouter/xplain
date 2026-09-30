//! Ported from `e2e/scenarios/u16-mcptools` (MCP tools: next_question, answer, annotate, get_questions,
//! files_changed, modal counters).
//!
//! Concurrent clients without a session are told apart by their TCP remote port in the real app; here every
//! parked request that must be a separate connection gets its own `remote_port`.

use regex::Regex;
use serde_json::{Value, json};
use xplain_sim::{Http, HttpReply, Sim, SimBuilder};

/// Autostarted MCP server (config `mcp.autostart`).
fn autostart() -> SimBuilder {
    Sim::builder().config_json(json!({"mcp": {"autostart": true}}))
}

/// JSON subset match: objects need only the listed keys, arrays match element-wise (same length).
fn sub(got: &Value, want: &Value) -> bool {
    match (got, want) {
        (Value::Object(g), Value::Object(w)) => w.iter().all(|(k, v)| g.get(k).is_some_and(|x| sub(x, v))),
        (Value::Array(g), Value::Array(w)) => g.len() == w.len() && g.iter().zip(w).all(|(a, b)| sub(a, b)),
        (a, b) => a == b,
    }
}

#[track_caller]
fn expect_sub(got: &Value, want: Value) {
    assert!(sub(got, &want), "got:\n{got:#}\nwant (subset):\n{want:#}");
}

#[track_caller]
fn re_match(pattern: &str, text: &str) {
    assert!(Regex::new(pattern).is_ok_and(|r| r.is_match(text)), "/{pattern}/ does not match {text:?}");
}

/// Raw `result.content[0].text` of a JSON-RPC body item.
fn text_of(item: &Value) -> String {
    item["result"]["content"][0]["text"].as_str().unwrap_or_default().to_string()
}

/// Raw text of a single tool reply.
fn raw_text(r: &HttpReply) -> String {
    text_of(&r.json())
}

#[track_caller]
fn no_error_flag(r: &HttpReply) {
    let b = r.json();
    assert!(b["result"].get("isError").is_none(), "isError set: {b}");
}

#[track_caller]
fn tool_error(r: &HttpReply, msg: &str) {
    assert_eq!(r.status, 200);
    let b = r.json();
    assert_eq!(b["result"]["isError"], true, "{b}");
    assert_eq!(b["result"]["content"][0]["type"], "text");
    assert_eq!(text_of(&b), msg);
}

#[track_caller]
fn none(s: &Sim, texts: &[&str]) {
    for t in texts {
        s.assert_not_contains(t);
    }
}

#[track_caller]
fn all(s: &Sim, patterns: &[&str]) {
    for p in patterns {
        s.assert_matches(p);
    }
}

fn batch(v: Value) -> Http {
    Http::json(&v)
}

fn init_call(id: i64, name: &str, version: Option<&str>) -> Value {
    let mut info = json!({"name": name});
    if let Some(v) = version {
        info["version"] = json!(v);
    }
    json!({"jsonrpc": "2.0", "id": id, "method": "initialize",
           "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": info}})
}

fn poll_call(id: i64, wait: i64) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": "tools/call",
           "params": {"name": "next_question", "arguments": {"wait_seconds": wait}}})
}

const NO_QUESTION: &str = r#"{"status":"no_question_yet","call_again":true,"note":"No question yet. Call next_question again immediately."}"#;

// ---------------------------------------------------------------------------------------------------------------
// F-MCPSRV-06: next_question

/// F-MCPSRV-06: autostarted MCP delivers a TUI question to a waiting `next_question` long poll; the answer shows
/// inline; stopping MCP from the modal answers a waiting long poll at once with `closed`.
#[test]
fn f_mcpsrv_06_poll() {
    let mut s = Sim::builder().config_json(json!({"mcp": {"autostart": true}})).build();
    s.assert_row_contains(0, "[mcp: on]");
    let token = s.file("${STATE}/xplain/mcp.json");
    let token = serde_json::from_str::<serde_json::Value>(&token).unwrap_or_default();
    assert!(token["token"].as_str().is_some_and(|t| t.len() >= 16), "token file: {token}");

    let poll = s.http_start(Http::tool("next_question", json!({"wait_seconds": 60})));
    assert!(s.http_reply(&poll).is_none(), "long poll must park");

    s.keys("a");
    s.keys("why is this here?<Enter>");
    s.assert_row_contains(-1, "question sent to agent");

    let r = s.http_await(&poll);
    assert_eq!(r.status, 200);
    let q = r.tool_result();
    assert_eq!(q["status"], "question");
    assert_eq!(q["turn"], 1);
    assert_eq!(q["follow_up"], false);
    assert!(q["question"].as_str().is_some_and(|t| t.contains("why is this here?")), "{q}");
    let thread = q["thread_id"].as_str().unwrap_or_default().to_string();
    assert!(!thread.is_empty());

    let r = s.mcp_call("answer", json!({"thread_id": thread, "text": "because of reasons"}));
    assert_eq!(r.tool_result()["ok"], true);
    s.assert_contains("because of reasons");

    // MCP stop from the modal answers the waiting long poll at once with closed (HTTP 200)
    let poll2 = s.http_start(Http::tool("next_question", json!({"wait_seconds": 120})));
    s.keys("M");
    s.assert_contains("│> ● on");
    s.keys("<Enter>");
    let r = s.http_await(&poll2);
    assert_eq!(r.status, 200);
    assert_eq!(r.tool_result(), json!({"status": "closed", "note": "xplain closed the session. Stop."}));
    let body = r.json();
    assert_eq!(body["jsonrpc"], "2.0");
    assert_eq!(
        body["result"]["content"][0]["text"],
        r#"{"status":"closed","note":"xplain closed the session. Stop."}"#
    );
    assert!(body["result"].get("isError").is_none());
    s.assert_row_contains(0, "[mcp: off]");

    s.keys("<Esc>");
    s.keys("q");
    s.assert_contains("Quit xplain? (y/n)");
    s.keys("y");
    assert_eq!(s.exit_code(), Some(0));
    // Same client polling twice and a dropped poll connection: f_mcpsrv_06_poll_session.
    // NOT COVERED: "delivered but client gone: requeued at front" needs the connection to drop between delivery
    // and the response write, a race no black-box step can force.
}

/// Two waiting pollers: queued questions dispatched in poller arrival order; each divider names its client.
#[test]
fn f_mcpsrv_06_arrival_order() {
    let mut s = autostart().build();
    s.assert_row_contains(0, "[mcp: on]");
    let pa = s.http_start(batch(json!([init_call(1, "firstbot", None), poll_call(2, 120)])));
    s.settle();
    let pb =
        s.http_start(batch(json!([init_call(1, "secondbot", None), poll_call(2, 120)])).remote_port(40002));
    s.keys("a");
    s.keys("for the first<Enter>");
    let r = s.http_await(&pa);
    let items = r.json().as_array().cloned().unwrap_or_default();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["id"], 1);
    assert_eq!(items[1]["id"], 2);
    let t = text_of(&items[1]);
    assert!(
        t.starts_with(
            r#"{"status":"question","thread_id":"q1","turn":1,"follow_up":false,"question":"for the first\n"#
        ),
        "{t}"
    );
    s.keys("j");
    s.keys("a");
    s.keys("for the second<Enter>");
    let r = s.http_await(&pb);
    let items = r.json().as_array().cloned().unwrap_or_default();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["id"], 1);
    assert_eq!(items[1]["id"], 2);
    let t = text_of(&items[1]);
    assert!(
        t.starts_with(
            r#"{"status":"question","thread_id":"q2","turn":1,"follow_up":false,"question":"for the second\n"#
        ),
        "{t}"
    );
    s.assert_matches(
        r"│ for the first +│\n│─ answer · firstbot · streaming… ─+│[\s\S]*│ for the second +│\n│─ answer · secondbot · streaming… ─+│",
    );
}

/// Question from a hunk header row (no line number): Lines line omitted (Side directly followed by blank line),
/// code = hunk row text.
#[test]
fn f_mcpsrv_06_context_hunk() {
    let mut s = autostart().build();
    s.assert_row_contains(0, "[mcp: on]");
    let poll = s.http_start(Http::tool("next_question", json!({"wait_seconds": 60})));
    s.keys("kk");
    s.assert_row_contains(0, "[cursor r1:C1] README.md");
    s.keys("a");
    s.keys("hunk?<Enter>");
    let r = s.http_await(&poll);
    let q = r.tool_result();
    assert_eq!(q["status"], "question");
    assert_eq!(q["thread_id"], "q1");
    re_match(
        r"^hunk\?\n\nFile: README\.md\nSide: new \(after the change\)\n\nCode at cursor line:\n```\n@@[^\n]*\n```\n\nSurrounding context:\n```\n# Title\n",
        q["question"].as_str().unwrap_or_default(),
    );
}

/// Question from the old (left) split pane: Side: old (before the change), old line number, old text.
#[test]
fn f_mcpsrv_06_context_old() {
    let mut s = autostart().args(["--split"]).size(120, 40).build();
    s.assert_row_contains(0, "[mcp: on]");
    let poll = s.http_start(Http::tool("next_question", json!({"wait_seconds": 60})));
    s.keys("p");
    s.assert_row_matches(0, r"\[cursor old L2:C1\] README\.md");
    s.keys("a");
    s.keys("old side?<Enter>");
    let r = s.http_await(&poll);
    let q = r.tool_result();
    assert_eq!(q["status"], "question");
    assert_eq!(q["thread_id"], "q1");
    re_match(
        r"^old side\?\n\nFile: README\.md\nSide: old \(before the change\)\nLines: 2\n\nCode at cursor line:\n```\nhello\n```\n\nSurrounding context:\n```\n",
        q["question"].as_str().unwrap_or_default(),
    );
}

/// Follow-up result text exactly as spec example: previous before question, follow_up true, turn 2, same thread.
#[test]
fn f_mcpsrv_06_followup() {
    let mut s = autostart().build();
    s.assert_row_contains(0, "[mcp: on]");
    let poll = s.http_start(Http::tool("next_question", json!({"wait_seconds": 60})));
    s.keys("a");
    s.keys("why?<Enter>");
    let r = s.http_await(&poll);
    expect_sub(
        &r.tool_result(),
        json!({"status": "question", "thread_id": "q1", "turn": 1, "follow_up": false}),
    );
    let r = s.mcp_call("answer", json!({"thread_id": "q1", "text": "because"}));
    expect_sub(&r.tool_result(), json!({"ok": true}));
    let poll2 = s.http_start(Http::tool("next_question", json!({"wait_seconds": 60})));
    s.keys("K");
    s.keys("a");
    s.keys("and?<Enter>");
    s.assert_row_contains(-1, "follow-up queued");
    let r = s.http_await(&poll2);
    assert_eq!(r.status, 200);
    assert_eq!(
        raw_text(&r),
        r#"{"status":"question","thread_id":"q1","turn":2,"follow_up":true,"previous":[{"turn":1,"question":"why?","answer":"because"}],"question":"Follow-up to your earlier answer (thread q1, turn 2): and?"}"#
    );
    // delivered follow-up turn is streaming in the UI
    s.assert_matches(r"follow-up: and\? +┃\n┃─ answer · agent · streaming… ─+┃");
}

/// next_question with a question already queued returns it at once (no wait); exact turn-1 result text and
/// context.
#[test]
fn f_mcpsrv_06_immediate() {
    let mut s = autostart().build();
    s.assert_row_contains(0, "[mcp: on]");
    s.keys("jj");
    s.assert_row_contains(0, "[cursor L3:C1] README.md");
    s.keys("a");
    s.keys("why more?<Enter>");
    s.assert_row_contains(-1, "question sent to agent");
    // queued before the poll: returns immediately even with wait_seconds 120
    let r = s.mcp_call("next_question", json!({"wait_seconds": 120}));
    assert_eq!(r.status, 200);
    no_error_flag(&r);
    re_match(
        r#"^\{"status":"question","thread_id":"q1","turn":1,"follow_up":false,"question":"why more\?\\n\\nFile: README\.md\\n[^"]*"\}$"#,
        &raw_text(&r),
    );
    let q = r.tool_result();
    expect_sub(&q, json!({"status": "question", "thread_id": "q1", "turn": 1, "follow_up": false}));
    assert!(q.get("previous").is_none(), "{q}");
    re_match(
        r"^why more\?\n\nFile: README\.md\nSide: new \(after the change\)\nLines: 3\n\nCode at cursor line:\n```\nmore\n```\n\nSurrounding context:\n```\n# Title\nhello\nhello world\nmore\n```$",
        q["question"].as_str().unwrap_or_default(),
    );
    // delivered - the UI thread moves from waiting to streaming
    s.assert_contains("─ answer · agent · streaming… ─");
    s.assert_not_contains("waiting…");
}

/// Reply to an annotate note: follow-up turn 2 on the note thread, previous = (note you added with annotate) +
/// note text, question names the note (#n only when numbered).
#[test]
fn f_mcpsrv_06_note_reply() {
    let mut s = autostart().build();
    s.assert_row_contains(0, "[mcp: on]");
    let r =
        s.mcp_call("annotate", json!({"file": "README.md", "line": 2, "number": 3, "text": "numbered note"}));
    expect_sub(&r.tool_result(), json!({"ok": true}));
    let r =
        s.mcp_call("annotate", json!({"file": "README.md", "line": 2, "side": "old", "text": "old note"}));
    expect_sub(&r.tool_result(), json!({"ok": true}));
    // annotate enqueues nothing
    let r = s.mcp_call("get_questions", json!({}));
    expect_sub(&r.tool_result(), json!({"questions": []}));
    // first box by row is the old-side note (del row above the add row)
    s.keys("J");
    s.assert_contains("▸ sent  agent note L2");
    s.keys("a");
    s.keys("reply old<Enter>");
    s.assert_row_contains(-1, "follow-up queued");
    let r = s.mcp_call("next_question", json!({"wait_seconds": 1}));
    expect_sub(
        &r.tool_result(),
        json!({
            "status": "question",
            "thread_id": "q2",
            "turn": 2,
            "follow_up": true,
            "previous": [{"turn": 1, "question": "(note you added with annotate)", "answer": "old note"}],
            "question": "Follow-up to your earlier answer (thread q2, turn 2): reply old\n\nReply to your annotate note:\nFile: README.md\nSide: old\nLine: 2\n\nYour note:\nold note",
        }),
    );
    s.keys("J");
    s.assert_contains("▸ sent  #3 agent note L2");
    s.keys("a");
    s.keys("reply numbered<Enter>");
    s.assert_row_contains(-1, "follow-up queued");
    let r = s.mcp_call("next_question", json!({"wait_seconds": 1}));
    assert_eq!(
        raw_text(&r),
        r#"{"status":"question","thread_id":"q1","turn":2,"follow_up":true,"previous":[{"turn":1,"question":"(note you added with annotate)","answer":"numbered note"}],"question":"Follow-up to your earlier answer (thread q1, turn 2): reply numbered\n\nReply to your annotate note #3:\nFile: README.md\nSide: new\nLine: 2\n\nYour note:\nnumbered note"}"#
    );
}

/// Same client polling again: the older waiting poll answers no_question_yet; a dropped poll connection cancels
/// the poll.
#[test]
fn f_mcpsrv_06_poll_session() {
    let mut s = autostart().build();
    s.assert_row_contains(0, "[mcp: on]");
    let r = s.mcp_rpc(
        "initialize",
        json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "twicebot"}}),
    );
    assert_eq!(r.status, 200);
    let sid = r.header("mcp-session-id").unwrap_or_default().to_string();
    let p1 = s.http_start(Http::tool("next_question", json!({"wait_seconds": 120})).session(&sid));
    s.settle();
    let p2 = s.http_start(
        Http::tool("next_question", json!({"wait_seconds": 120})).session(&sid).remote_port(40002),
    );
    // the 120 s wait did not run out - p2 replaced it
    let r = s.http_await(&p1);
    assert_eq!(r.status, 200);
    expect_sub(
        &r.tool_result(),
        json!({"status": "no_question_yet", "call_again": true, "note": "No question yet. Call next_question again immediately."}),
    );
    s.keys("M");
    all(&s, &[r"^ +│   twicebot ⟳ +│$", r"│ clients 1 pending 0 delivered 0 +│"]);
    s.keys("<Esc>");
    // dropping p2's connection cancels the poll - the next question stays queued instead of going to it
    s.http_abort(&p2);
    s.keys("M");
    all(&s, &[r"^ +│   twicebot +│$", r"│ clients 1 pending 0 delivered 0 +│"]);
    s.assert_not_contains("⟳");
    s.keys("<Esc>");
    s.keys("a");
    s.keys("after the drop<Enter>");
    s.keys("M");
    all(&s, &[r"^ +│   twicebot +│$", r"│ clients 1 pending 1 delivered 0 +│"]);
    s.assert_not_contains("⟳");
    s.keys("<Esc>");
    let r = s.http(Http::tool("next_question", json!({"wait_seconds": 1})).session(&sid));
    assert_eq!(r.status, 200);
    let q = r.tool_result();
    expect_sub(&q, json!({"status": "question", "thread_id": "q1", "turn": 1}));
    assert!(q["question"].as_str().is_some_and(|t| t.contains("after the drop")), "{q}");
    s.assert_contains("─ answer · twicebot · streaming… ─");
}

/// Previous: human turns carry message only, answers of a turn joined by blank line, each text capped at 4000, only
/// the last 5 turns.
#[test]
fn f_mcpsrv_06_previous() {
    let mut s = autostart().build();
    s.assert_row_contains(0, "[mcp: on]");
    s.keys("a");
    s.keys("t1<Enter>");
    let r = s.mcp_call("next_question", json!({"wait_seconds": 1}));
    let q = r.tool_result();
    expect_sub(&q, json!({"thread_id": "q1", "turn": 1}));
    re_match(r"^t1\n\nFile: ", q["question"].as_str().unwrap_or_default());
    let r = s.mcp_call("answer", json!({"thread_id": "q1", "text": "a1a"}));
    expect_sub(&r.tool_result(), json!({"ok": true}));
    // second answer on the same done turn
    let r = s.mcp_call("answer", json!({"thread_id": "q1", "text": "a1b"}));
    expect_sub(&r.tool_result(), json!({"ok": true}));
    s.keys("K");
    s.keys("a");
    s.keys("t2<Enter>");
    let r = s.mcp_call("next_question", json!({"wait_seconds": 1}));
    expect_sub(
        &r.tool_result(),
        json!({
            "thread_id": "q1",
            "turn": 2,
            "follow_up": true,
            "previous": [{"turn": 1, "question": "t1", "answer": "a1a\n\na1b"}],
            "question": "Follow-up to your earlier answer (thread q1, turn 2): t2",
        }),
    );
    // 4100-char answer, capped to 4000 in previous
    let r = s.mcp_call("answer", json!({"thread_id": "q1", "text": "x".repeat(4100)}));
    expect_sub(&r.tool_result(), json!({"ok": true}));
    for (turn, prev) in [(3, "a3"), (4, "a4"), (5, "a5"), (6, "a6")] {
        s.keys("K");
        s.keys("a");
        s.keys(&format!("t{turn}<Enter>"));
        let r = s.mcp_call("next_question", json!({"wait_seconds": 1}));
        expect_sub(
            &r.tool_result(),
            json!({
                "thread_id": "q1",
                "turn": turn,
                "follow_up": true,
                "question": format!("Follow-up to your earlier answer (thread q1, turn {turn}): t{turn}"),
            }),
        );
        let r = s.mcp_call("answer", json!({"thread_id": "q1", "text": prev}));
        expect_sub(&r.tool_result(), json!({"ok": true}));
    }
    s.keys("K");
    s.keys("a");
    s.keys("t7<Enter>");
    // turn 7 sees turns 2-6 only (last 5); turn 2 answer capped at 4000 chars
    let r = s.mcp_call("next_question", json!({"wait_seconds": 1}));
    expect_sub(
        &r.tool_result(),
        json!({
            "thread_id": "q1",
            "turn": 7,
            "follow_up": true,
            "previous": [
                {"turn": 2, "question": "t2", "answer": "x".repeat(4000)},
                {"turn": 3, "question": "t3", "answer": "a3"},
                {"turn": 4, "question": "t4", "answer": "a4"},
                {"turn": 5, "question": "t5", "answer": "a5"},
                {"turn": 6, "question": "t6", "answer": "a6"},
            ],
            "question": "Follow-up to your earlier answer (thread q1, turn 7): t7",
        }),
    );
    let r = s.mcp_call("get_questions", json!({}));
    expect_sub(&r.tool_result(), json!({"questions": []}));
}

/// Sticky follow-up: turn 2 goes to the client that got turn 1 while it polls, even though another client waited
/// longer; new questions go to any client.
#[test]
fn f_mcpsrv_06_sticky() {
    let mut s = autostart().build();
    s.assert_row_contains(0, "[mcp: on]");
    // pa is one batch: its second next_question starts right after the first one got q1, as the same session.
    let pa =
        s.http_start(batch(json!([init_call(1, "stickybot", None), poll_call(2, 120), poll_call(3, 120)])));
    s.settle();
    let pb =
        s.http_start(batch(json!([init_call(1, "otherbot", None), poll_call(2, 120)])).remote_port(40002));
    s.settle();
    s.keys("a");
    s.keys("why?<Enter>");
    // q1 went to stickybot (first poller); its batch now polls again as the same client
    s.assert_contains("─ answer · stickybot · streaming… ─");
    let r = s.mcp_call("answer", json!({"thread_id": "q1", "text": "because"}));
    expect_sub(&r.tool_result(), json!({"ok": true}));
    s.keys("K");
    s.keys("a");
    s.keys("and?<Enter>");
    s.assert_row_contains(-1, "follow-up queued");
    let r = s.http_await(&pa);
    let items = r.json().as_array().cloned().unwrap_or_default();
    assert_eq!(items.len(), 3);
    assert_eq!(items[0]["id"], 1);
    assert_eq!(items[1]["id"], 2);
    assert!(
        text_of(&items[1]).starts_with(r#"{"status":"question","thread_id":"q1","turn":1,"#),
        "{}",
        text_of(&items[1])
    );
    assert_eq!(items[2]["id"], 3);
    assert_eq!(
        text_of(&items[2]),
        r#"{"status":"question","thread_id":"q1","turn":2,"follow_up":true,"previous":[{"turn":1,"question":"why?","answer":"because"}],"question":"Follow-up to your earlier answer (thread q1, turn 2): and?"}"#
    );
    s.assert_matches(r"follow-up: and\? +┃\n┃─ answer · stickybot · streaming… ─+┃");
    // a new question is not sticky - the other waiting client gets it
    s.keys("<Esc>j");
    s.keys("a");
    s.keys("fresh<Enter>");
    let r = s.http_await(&pb);
    let items = r.json().as_array().cloned().unwrap_or_default();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["id"], 1);
    assert_eq!(items[1]["id"], 2);
    assert!(
        text_of(&items[1]).starts_with(
            r#"{"status":"question","thread_id":"q2","turn":1,"follow_up":false,"question":"fresh\n"#
        ),
        "{}",
        text_of(&items[1])
    );
}

/// Delivery sets the UI answer status streaming with the polling client name (initialize clientInfo) in the divider.
#[test]
fn f_mcpsrv_06_streaming() {
    let mut s = autostart().build();
    s.assert_row_contains(0, "[mcp: on]");
    // batch - initialize creates the session, the following next_question polls as that client
    let poll = s.http_start(batch(json!([init_call(1, "pollbot", Some("2.0")), poll_call(2, 60)])));
    s.keys("a");
    s.keys("stream me<Enter>");
    let r = s.http_await(&poll);
    assert_eq!(r.status, 200);
    let items = r.json().as_array().cloned().unwrap_or_default();
    assert_eq!(items.len(), 2);
    expect_sub(&items[0], json!({"id": 1, "result": {"serverInfo": {"name": "xplain"}}}));
    assert_eq!(items[1]["id"], 2);
    assert!(
        text_of(&items[1]).starts_with(
            r#"{"status":"question","thread_id":"q1","turn":1,"follow_up":false,"question":"stream me\n\n"#
        ),
        "{}",
        text_of(&items[1])
    );
    s.assert_contains("─ answer · pollbot · streaming… ─");
    s.assert_matches(r"│ [⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏] agent working… +│");
    none(&s, &["waiting…", "waiting for agent"]);
    let r = s.mcp_call("answer", json!({"thread_id": "q1", "text": "streamed reply"}));
    expect_sub(&r.tool_result(), json!({"ok": true}));
    s.assert_matches(r"│─ answer · pollbot · done ─+│\n│ streamed reply +│");
    none(&s, &["streaming…", "agent working"]);
}

/// next_question with an empty queue and wait_seconds 0 / negative / fractional (clamped to 1) returns
/// no_question_yet exactly.
#[test]
fn f_mcpsrv_06_timeout() {
    let mut s = autostart().build();
    s.assert_row_contains(0, "[mcp: on]");
    // wait_seconds 0 clamps up to 1; no question can arrive, so the outcome is deterministic. The poll parks until
    // the manual clock passes the 1 s wait.
    let expire = |s: &mut Sim, wait: Value| {
        let p = s.http_start(Http::tool("next_question", json!({"wait_seconds": wait})));
        assert!(s.http_reply(&p).is_none(), "poll must wait for its timeout");
        s.advance_clock(999);
        assert!(s.http_reply(&p).is_none(), "clamped to a 1 s wait, not less");
        s.advance_clock(1);
        s.http_await(&p)
    };
    let r = expire(&mut s, json!(0));
    assert_eq!(r.status, 200);
    no_error_flag(&r);
    assert_eq!(r.json()["result"]["content"].as_array().map(Vec::len), Some(1));
    assert_eq!(r.json()["result"]["content"][0]["type"], "text");
    assert_eq!(raw_text(&r), NO_QUESTION);
    let r = expire(&mut s, json!(-30));
    assert_eq!(raw_text(&r), NO_QUESTION);
    // 0.4 floors to 0, clamps to 1
    let r = expire(&mut s, json!(0.4));
    expect_sub(
        &r.tool_result(),
        json!({"status": "no_question_yet", "call_again": true, "note": "No question yet. Call next_question again immediately."}),
    );
    // the timed-out polls consumed nothing; a later question is still delivered as q1
    s.keys("a");
    s.keys("later<Enter>");
    let r = s.mcp_call("next_question", json!({"wait_seconds": 0}));
    let q = r.tool_result();
    expect_sub(&q, json!({"status": "question", "thread_id": "q1", "turn": 1}));
    re_match(r"^later\n\nFile: README\.md\n", q["question"].as_str().unwrap_or_default());
}

/// next_question wait_seconds non-number (default 45) and above max (clamped 120) are no errors: poll waits and
/// gets the next question.
#[test]
fn f_mcpsrv_06_wait_arg() {
    let mut s = autostart().build();
    s.assert_row_contains(0, "[mcp: on]");
    let p1 = s.http_start(Http::tool("next_question", json!({"wait_seconds": "abc"})));
    s.keys("a");
    s.keys("first<Enter>");
    let r = s.http_await(&p1);
    assert_eq!(r.status, 200);
    no_error_flag(&r);
    let q = r.tool_result();
    expect_sub(&q, json!({"status": "question", "thread_id": "q1", "turn": 1, "follow_up": false}));
    re_match(r"^first\n\n", q["question"].as_str().unwrap_or_default());
    let p2 = s.http_start(Http::tool("next_question", json!({"wait_seconds": 100_000})));
    s.keys("j");
    s.keys("a");
    s.keys("second<Enter>");
    let r = s.http_await(&p2);
    no_error_flag(&r);
    let q = r.tool_result();
    expect_sub(&q, json!({"status": "question", "thread_id": "q2", "turn": 1, "follow_up": false}));
    re_match(r"^second\n\n", q["question"].as_str().unwrap_or_default());
    // no wait_seconds at all
    let p3 = s.http_start(Http::tool("next_question", json!({})));
    s.keys("j");
    s.keys("a");
    s.keys("third<Enter>");
    let r = s.http_await(&p3);
    let q = r.tool_result();
    expect_sub(&q, json!({"status": "question", "thread_id": "q3"}));
    re_match(r"^third\n\n", q["question"].as_str().unwrap_or_default());
}

// ---------------------------------------------------------------------------------------------------------------
// F-MCPSRV-07: answer

/// Answer errors: missing / non-string thread_id or text; unknown thread (id cut at 100); queued but undelivered
/// thread; UI untouched.
#[test]
fn f_mcpsrv_07_answer_errors() {
    let mut s = autostart().build();
    s.assert_row_contains(0, "[mcp: on]");
    let need = "thread_id and text (strings) are required";
    for args in [
        json!({}),
        json!({"thread_id": "q1"}),
        json!({"text": "hi"}),
        json!({"thread_id": 1, "text": "hi"}),
        json!({"thread_id": "q1", "text": 7}),
    ] {
        let r = s.mcp_call("answer", args.clone());
        tool_error(&r, need);
    }
    let r = s.mcp_call("answer", json!({"thread_id": "q99", "text": "hi"}));
    tool_error(&r, "Unknown thread_id: q99");
    // id echoed max 100 chars
    let r = s.mcp_call("answer", json!({"thread_id": "z".repeat(150), "text": "hi"}));
    tool_error(&r, &format!("Unknown thread_id: {}", "z".repeat(100)));
    // q1 queued by the UI but not yet delivered to any agent
    s.keys("a");
    s.keys("pending one<Enter>");
    s.assert_contains(" ⠿ waiting for agent…");
    let r = s.mcp_call("answer", json!({"thread_id": "q1", "text": "too early"}));
    tool_error(&r, "Unknown thread_id: q1");
    s.assert_contains("─ answer · agent · waiting… ─");
    s.assert_contains(" ⠿ waiting for agent…");
    none(&s, &["too early", "done"]);
    let r = s.mcp_call("get_questions", json!({}));
    expect_sub(&r.tool_result(), json!({"questions": [{"thread_id": "q1", "turn": 1}]}));
    // agent note thread was never delivered either
    let r = s.mcp_call("annotate", json!({"file": "README.md", "line": 3, "text": "a note"}));
    expect_sub(&r.tool_result(), json!({"ok": true}));
    let r = s.mcp_call("answer", json!({"thread_id": "q2", "text": "self answer"}));
    tool_error(&r, "Unknown thread_id: q2");
    s.assert_contains("a note");
    s.assert_not_contains("self answer");
}

/// Answer ok: exact result text, UI turn done with text inline; second answer appended; answer text sanitized.
#[test]
fn f_mcpsrv_07_answer_ok() {
    let mut s = autostart().build();
    s.assert_row_contains(0, "[mcp: on]");
    s.keys("a");
    s.keys("explain<Enter>");
    let r = s.mcp_call("next_question", json!({"wait_seconds": 1}));
    expect_sub(&r.tool_result(), json!({"status": "question", "thread_id": "q1"}));
    s.assert_contains("─ answer · agent · streaming… ─");
    s.assert_not_contains("first answer");
    let r = s.mcp_call("answer", json!({"thread_id": "q1", "text": "first answer"}));
    assert_eq!(r.status, 200);
    no_error_flag(&r);
    assert_eq!(
        raw_text(&r),
        r#"{"ok":true,"note":"Answer delivered. Call next_question again immediately."}"#
    );
    s.assert_matches(r"│─ answer · agent · done ─+│\n│ first answer +│");
    none(&s, &["streaming…", "agent working"]);
    // second answer on the done turn, with SGR escapes and a BEL
    let r =
        s.mcp_call("answer", json!({"thread_id": "q1", "text": "second\u{1b}[31m red\u{1b}[0m\u{7} part"}));
    expect_sub(
        &r.tool_result(),
        json!({"ok": true, "note": "Answer delivered. Call next_question again immediately."}),
    );
    s.assert_matches(r"first answer +│\n[\s\S]*│ second red part +│");
    none(&s, &["[31m", "[0m"]);
    // follow-up shows the stored answers of turn 1 - both kept, joined, sanitized
    s.keys("K");
    s.keys("a");
    s.keys("more<Enter>");
    let r = s.mcp_call("next_question", json!({"wait_seconds": 1}));
    expect_sub(
        &r.tool_result(),
        json!({
            "thread_id": "q1",
            "turn": 2,
            "previous": [{"turn": 1, "question": "explain", "answer": "first answer\n\nsecond red part"}],
        }),
    );
    // answer goes to the latest delivered turn (turn 2)
    let r = s.mcp_call("answer", json!({"thread_id": "q1", "text": "turn two answer"}));
    expect_sub(&r.tool_result(), json!({"ok": true}));
    s.assert_matches(r"follow-up: more +┃\n┃─ answer · agent · done ─+┃\n┃ turn two answer +┃");
}

// ---------------------------------------------------------------------------------------------------------------
// F-MCPSRV-08: annotate

/// Annotate errors: file string, line number, text string required (exact isError text); nothing added.
#[test]
fn f_mcpsrv_08_annotate_errors() {
    let mut s = autostart().build();
    s.assert_row_contains(0, "[mcp: on]");
    let need = "file (string), line (number) and text (string) are required";
    for args in [
        json!({}),
        json!({"file": "README.md", "line": "2", "text": "string line"}),
        json!({"file": 5, "line": 2, "text": "number file"}),
        json!({"file": "README.md", "line": 2}),
        json!({"file": "README.md", "line": 2, "text": 42}),
        json!({"line": 2, "text": "no file"}),
    ] {
        let r = s.mcp_call("annotate", args.clone());
        tool_error(&r, need);
    }
    none(&s, &["agent note", "string line", "number file", "no file"]);
    s.keys("J");
    s.assert_row_contains(-1, "no comments");
}

/// Annotate ok: exact result, note box on file/line/side; line floored and min 1; unknown side = new; only in its
/// own file.
#[test]
fn f_mcpsrv_08_annotate() {
    let mut s = autostart().build();
    s.assert_row_contains(0, "[mcp: on]");
    s.assert_not_contains("agent note");
    let r = s.mcp_call("annotate", json!({"file": "README.md", "line": 3, "text": "on more"}));
    assert_eq!(r.status, 200);
    no_error_flag(&r);
    assert_eq!(raw_text(&r), r#"{"ok":true}"#);
    // 2.9 floors to 2; side not old/new falls back to new (add row hello world, not del row hello)
    let r = s.mcp_call(
        "annotate",
        json!({"file": "README.md", "line": 2.9, "side": "middle", "text": "floored new"}),
    );
    assert_eq!(raw_text(&r), r#"{"ok":true}"#);
    // line 0 -> 1 on the old side (context row old 1)
    let r = s.mcp_call("annotate", json!({"file": "README.md", "line": 0, "side": "old", "text": "min one"}));
    expect_sub(&r.tool_result(), json!({"ok": true}));
    let r = s.mcp_call("annotate", json!({"file": "README.md", "line": -7, "text": "negative"}));
    expect_sub(&r.tool_result(), json!({"ok": true}));
    let r = s.mcp_call("annotate", json!({"file": "src/c.tsx", "line": 1, "text": "other file note"}));
    expect_sub(&r.tool_result(), json!({"ok": true}));
    none(&s, &["other file note", "saved · not asked"]);
    s.assert_matches(
        r"   1    1   # Title\n╭─+╮\n│ sent  agent note L1 +│\n│ min one +│\n╰─+╯\n╭─+╮\n│ sent  agent note L1 +│\n│ negative +│\n╰─+╯\n   2      -[▶ ]hello *…?\n        2 \+[▶ ]hello world *…?\n╭─+╮\n│ sent  agent note L2 +│\n│ floored new +│\n╰─+╯\n        3 \+[▶ ]more *…?\n╭─+╮\n│ sent  agent note L3 +│\n│ on more +│\n╰─+╯",
    );
    // the other file's note shows when that file is shown
    s.keys("<S-Tab><S-Tab>");
    s.assert_row_matches(0, r"\] src/c\.tsx ");
    none(&s, &["on more", "floored new"]);
    s.assert_matches(r"╭─+╮\n│ sent  agent note L1 +│\n│ other file note +│\n╰─+╯");
    // annotate on the file currently open in browse shows there at once
    s.keys("Fjson<Enter>");
    s.assert_row_matches(0, r"^\[browse\] .*package\.json$");
    s.assert_not_contains("agent note");
    let r = s.mcp_call("annotate", json!({"file": "package.json", "line": 1, "text": "browse note"}));
    expect_sub(&r.tool_result(), json!({"ok": true}));
    s.assert_row_matches(0, r"^\[browse\] .*package\.json$");
    s.assert_matches(r"╭─+╮\n│ sent  agent note L1 +│\n│ browse note +│\n╰─+╯");
}

/// Annotate number: integer shown as #n and joins ) order (any file, browse for non-diff file); non-integer number
/// ignored.
#[test]
fn f_mcpsrv_08_number() {
    let mut s = autostart().build();
    s.assert_row_contains(0, "[mcp: on]");
    let r = s.mcp_call("annotate", json!({"file": "README.md", "line": 3, "number": 2, "text": "step two"}));
    expect_sub(&r.tool_result(), json!({"ok": true}));
    let r =
        s.mcp_call("annotate", json!({"file": "package.json", "line": 1, "number": 1, "text": "step one"}));
    expect_sub(&r.tool_result(), json!({"ok": true}));
    let r =
        s.mcp_call("annotate", json!({"file": "README.md", "line": 1, "number": 1.5, "text": "fractional"}));
    expect_sub(&r.tool_result(), json!({"ok": true}));
    let r = s.mcp_call("annotate", json!({"file": "README.md", "line": 2, "number": "3", "text": "stringy"}));
    expect_sub(&r.tool_result(), json!({"ok": true}));
    for t in ["│ sent  #2 agent note L3 ", "│ sent  agent note L1 ", "│ sent  agent note L2 "] {
        s.assert_contains(t);
    }
    none(&s, &["#1.5", "#3", "#1 agent note"]);
    // first numbered = #1 in package.json (not in diff) -> opened in browse, focused
    s.keys(")");
    s.assert_row_matches(0, r"^\[browse\] .*package\.json$");
    s.assert_contains("▸ sent  #1 agent note L1");
    s.assert_contains("step one");
    s.keys(")");
    s.assert_not_contains("[browse]");
    s.assert_row_matches(0, r"\[cursor L3:C1\] README\.md ");
    s.assert_contains("▸ sent  #2 agent note L3");
    // wraps back to #1 - fractional / string numbers are not in the numbered set
    s.keys(")");
    s.assert_row_matches(0, r"^\[browse\] .*package\.json$");
    s.assert_contains("▸ sent  #1 agent note L1");
}

// ---------------------------------------------------------------------------------------------------------------
// F-MCPSRV-09..11

/// get_questions lists queued undelivered questions (thread_id, turn, follow_up, preview = message first 200
/// chars) without consuming them.
#[test]
fn f_mcpsrv_09_get_questions() {
    let mut s = autostart().build();
    s.assert_row_contains(0, "[mcp: on]");
    let r = s.mcp_call("get_questions", json!({}));
    assert_eq!(r.status, 200);
    no_error_flag(&r);
    assert_eq!(raw_text(&r), r#"{"questions":[]}"#);
    // 250-char message
    let long = "abcdefghij".repeat(25);
    let head = "abcdefghij".repeat(20);
    s.keys("a");
    s.keys(&format!("{long}<Enter>"));
    s.keys("j");
    s.keys("a");
    s.keys("short one<Enter>");
    let r = s.mcp_call("get_questions", json!({}));
    assert_eq!(
        raw_text(&r),
        format!(
            r#"{{"questions":[{{"thread_id":"q1","turn":1,"follow_up":false,"preview":"{head}"}},{{"thread_id":"q2","turn":1,"follow_up":false,"preview":"short one"}}]}}"#
        )
    );
    // not consumed - same list again, nothing delivered in the UI
    let r = s.mcp_call("get_questions", json!({}));
    expect_sub(
        &r.tool_result(),
        json!({"questions": [{"thread_id": "q1", "preview": head}, {"thread_id": "q2", "preview": "short one"}]}),
    );
    s.assert_matches(r"waiting…[\s\S]*waiting…");
    s.assert_not_contains("streaming…");
    let r = s.mcp_call("next_question", json!({"wait_seconds": 1}));
    let q = r.tool_result();
    expect_sub(&q, json!({"status": "question", "thread_id": "q1"}));
    re_match(&format!("^{long}\n\nFile: "), q["question"].as_str().unwrap_or_default());
    // delivered q1 gone from the list
    let r = s.mcp_call("get_questions", json!({}));
    expect_sub(
        &r.tool_result(),
        json!({"questions": [{"thread_id": "q2", "turn": 1, "follow_up": false, "preview": "short one"}]}),
    );
    let r = s.mcp_call("answer", json!({"thread_id": "q1", "text": "done one"}));
    expect_sub(&r.tool_result(), json!({"ok": true}));
    // follow-up on q1 listed as turn 2, follow_up true, preview = follow-up message only
    s.keys("KK");
    s.assert_matches(r"▸ sent  line L2 +┃\n┃ abcdefghij");
    s.keys("a");
    s.keys("again please<Enter>");
    s.assert_row_contains(-1, "follow-up queued");
    let r = s.mcp_call("get_questions", json!({}));
    expect_sub(
        &r.tool_result(),
        json!({"questions": [
            {"thread_id": "q2", "turn": 1, "follow_up": false, "preview": "short one"},
            {"thread_id": "q1", "turn": 2, "follow_up": true, "preview": "again please"},
        ]}),
    );
}

/// files_changed: exact ok result, silent reload (new file appears, no note); non-string / too many / long paths,
/// non-array or omitted paths accepted; browsed file re-read.
#[test]
fn f_mcpsrv_10_files_changed() {
    let mut s = autostart().build();
    s.assert_row_matches(0, r"\[mcp: on\] \[1/4\] .*README\.md \+2 -1$");
    s.write_file("added.txt", "brand new file\n");
    s.write_file("README.md", "# Title\nhello world\nmore\nagent line\n");
    // not reloaded yet
    s.assert_row_contains(0, "[1/4]");
    s.assert_not_contains("agent line");
    // non-string entries are dropped, not an error
    let r = s.mcp_call("files_changed", json!({"paths": ["README.md", 5, null, {"a": 1}, "added.txt"]}));
    assert_eq!(r.status, 200);
    no_error_flag(&r);
    assert_eq!(raw_text(&r), r#"{"ok":true}"#);
    s.assert_row_matches(0, r"\[mcp: on\] \[1/5\] .*README\.md \+3 -1$");
    assert!(!s.row(-1).contains("reloaded"), "{}", s.row(-1));
    s.assert_contains("+ agent line");
    s.keys("<S-Tab><S-Tab>");
    s.assert_row_matches(0, r"\[\d/5\] .*added\.txt");
    s.assert_contains("brand new file");
    // more than 100 paths and a path over 500 chars - still ok
    s.write_file("added.txt", "second version\n");
    let many: Vec<String> = (0..150).map(|i| format!("f{i}.txt")).collect();
    let r = s.mcp_call("files_changed", json!({"paths": many}));
    no_error_flag(&r);
    assert_eq!(raw_text(&r), r#"{"ok":true}"#);
    s.assert_contains("second version");
    none(&s, &["brand new file", "reloaded"]);
    s.write_file("added.txt", "third version\n");
    let r = s.mcp_call("files_changed", json!({"paths": ["p".repeat(600)]}));
    assert_eq!(raw_text(&r), r#"{"ok":true}"#);
    s.assert_contains("third version");
    none(&s, &["second version", "reloaded"]);
    // paths not an array (a string) - ignored, still ok and reloads
    s.write_file("added.txt", "fourth version\n");
    let r = s.mcp_call("files_changed", json!({"paths": "added.txt"}));
    assert_eq!(r.status, 200);
    no_error_flag(&r);
    assert_eq!(raw_text(&r), r#"{"ok":true}"#);
    s.assert_contains("fourth version");
    none(&s, &["third version", "reloaded"]);
    // paths omitted - ok and reloads
    s.write_file("added.txt", "fifth version\n");
    let r = s.mcp_call("files_changed", json!({}));
    no_error_flag(&r);
    assert_eq!(raw_text(&r), r#"{"ok":true}"#);
    s.assert_contains("fifth version");
    none(&s, &["fourth version", "reloaded"]);
    // browse - the browsed file is re-read
    s.keys("Fjson<Enter>");
    s.assert_row_matches(0, r"^\[browse\] .*package\.json$");
    s.assert_contains("\"demo\"");
    s.write_file("package.json", "{\"name\":\"changed\"}\n");
    let r = s.mcp_call("files_changed", json!({"paths": ["package.json"]}));
    expect_sub(&r.tool_result(), json!({"ok": true}));
    s.assert_row_matches(0, r"^\[browse\] .*package\.json$");
    s.assert_contains("\"changed\"");
    none(&s, &["\"demo\"", "reloaded"]);
}

/// MCP modal: clients = initialize sessions (name/version); pending = queued undelivered; delivered = delivery
/// count incl. follow-ups.
#[test]
fn f_mcpsrv_11_counters() {
    let mut s = autostart().build();
    s.assert_row_contains(0, "[mcp: on]");
    s.keys("M");
    s.assert_matches(r"│ clients 0 pending 0 delivered 0 +│");
    s.keys("<Esc>");
    let r = s.mcp_rpc(
        "initialize",
        json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "alpha", "version": "1.0"}}),
    );
    assert_eq!(r.status, 200);
    let r = s.mcp_rpc(
        "initialize",
        json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "beta"}}),
    );
    assert_eq!(r.status, 200);
    s.keys("a");
    s.keys("one<Enter>");
    s.keys("j");
    s.keys("a");
    s.keys("two<Enter>");
    s.keys("M");
    all(&s, &[r"│ clients 2 pending 2 delivered 0 +│", r"│   alpha 1\.0 +│", r"│   beta +│"]);
    s.keys("<Esc>");
    // later tool calls carry no mcp-session-id: anonymous client entries are UNSPEC-12, so clients count not
    // asserted below
    // get_questions does not consume
    let r = s.mcp_call("get_questions", json!({}));
    assert_eq!(r.tool_result()["questions"].as_array().map(Vec::len), Some(2));
    s.keys("M");
    s.assert_matches(r"│ clients \d+ pending 2 delivered 0 +│");
    s.keys("<Esc>");
    let r = s.mcp_call("next_question", json!({"wait_seconds": 1}));
    expect_sub(&r.tool_result(), json!({"status": "question", "thread_id": "q1"}));
    s.keys("M");
    s.assert_matches(r"│ clients \d+ pending 1 delivered 1 +│");
    s.keys("<Esc>");
    let r = s.mcp_call("next_question", json!({"wait_seconds": 1}));
    expect_sub(&r.tool_result(), json!({"status": "question", "thread_id": "q2"}));
    s.keys("M");
    s.assert_matches(r"│ clients \d+ pending 0 delivered 2 +│");
    s.keys("<Esc>");
    // answers do not change the counters
    let r = s.mcp_call("answer", json!({"thread_id": "q1", "text": "ans"}));
    expect_sub(&r.tool_result(), json!({"ok": true}));
    s.keys("M");
    s.assert_matches(r"│ clients \d+ pending 0 delivered 2 +│");
    s.keys("<Esc>");
    // follow-up queued -> pending, delivered -> counted
    s.keys("KK");
    s.assert_matches(r"▸ sent  line L2 +┃\n┃ one ");
    s.keys("a");
    s.keys("more<Enter>");
    s.assert_row_contains(-1, "follow-up queued");
    s.keys("M");
    s.assert_matches(r"│ clients \d+ pending 1 delivered 2 +│");
    s.keys("<Esc>");
    let r = s.mcp_call("next_question", json!({"wait_seconds": 1}));
    expect_sub(
        &r.tool_result(),
        json!({"status": "question", "thread_id": "q1", "turn": 2, "follow_up": true}),
    );
    s.keys("M");
    s.assert_matches(r"│ clients \d+ pending 0 delivered 3 +│");
}

/// Client row shows the poll marker while its long poll waits, marker gone once the poll got a question; counters
/// follow.
#[test]
fn f_mcpsrv_11_polling() {
    let mut s = autostart().build();
    s.assert_row_contains(0, "[mcp: on]");
    let poll = s.http_start(batch(json!([init_call(1, "waitbot", Some("2.0")), poll_call(2, 120)])));
    // the 120 s wait never expires: a question ends the poll.
    s.keys("M");
    all(&s, &[r"^ +│   waitbot 2\.0 ⟳ +│$", r"│ clients \d+ pending 0 delivered 0 +│"]);
    s.keys("<Esc>");
    s.keys("a");
    s.keys("poll me<Enter>");
    let r = s.http_await(&poll);
    let items = r.json().as_array().cloned().unwrap_or_default();
    assert_eq!(items.len(), 2);
    expect_sub(&items[0], json!({"id": 1, "result": {"serverInfo": {"name": "xplain"}}}));
    assert_eq!(items[1]["id"], 2);
    assert!(
        text_of(&items[1]).starts_with(r#"{"status":"question","thread_id":"q1""#),
        "{}",
        text_of(&items[1])
    );
    s.keys("M");
    all(&s, &[r"^ +│   waitbot 2\.0 +│$", r"│ clients \d+ pending 0 delivered 1 +│"]);
    s.assert_not_contains("⟳");
    s.keys("<Esc>");
    // a new long poll waits (marker back); MCP stop answers it with closed (F-MCPSRV-06, F-MCPUI-03)
    let poll2 = s.http_start(
        batch(json!([init_call(3, "waitbot2", Some("3.0")), poll_call(4, 120)])).remote_port(40002),
    );
    s.keys("M");
    s.assert_matches(r"^ +│   waitbot2 3\.0 ⟳ +│$");
    s.keys("<Enter>");
    let r = s.http_await(&poll2);
    assert_eq!(r.status, 200);
    let items = r.json().as_array().cloned().unwrap_or_default();
    assert_eq!(items.len(), 2);
    expect_sub(&items[0], json!({"id": 3, "result": {"serverInfo": {"name": "xplain"}}}));
    assert_eq!(items[1]["id"], 4);
    assert_eq!(text_of(&items[1]), r#"{"status":"closed","note":"xplain closed the session. Stop."}"#);
    s.assert_matches(r"│ clients 0 pending 0 delivered \d+ +│");
    none(&s, &["⟳", "waitbot"]);
}
