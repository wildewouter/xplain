//! Ported from `e2e/scenarios/u16-mcptools`.

use serde_json::json;
use xplain_sim::{Http, Sim};

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
}
