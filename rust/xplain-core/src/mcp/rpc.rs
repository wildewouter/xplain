//! JSON-RPC 2.0 envelope and method dispatch.
//!
//! Spec: F-MCPSRV-03 (envelope: parse errors, batch, notifications -> 202, ids), F-MCPSRV-04 (initialize with
//! protocol negotiation and session id, ping, tools/list), F-MCPSRV-05 (tools/call routing, errors),
//! F-MCPSRV-11 (client registration). Oracle: `src/mcp/server.ts`. Owner: component `agent` (E).
//! Must not: implement tool bodies (tools.rs) or queue logic (hub.rs).

use serde_json::Value;

use super::http::raw_response;
use super::hub::{Cont, PollResult, Resolved, cap_chars};
use super::tools::{self, PROTOCOL_VERSIONS, SERVER_VERSION, TOOL_NAMES, ToolOutcome};
use super::{ConnId, HttpRequest, McpOutput, McpState};
use crate::effect::Effect;
use crate::event::TimerId;

fn quote(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| String::from("\"\""))
}

/// `{"jsonrpc":"2.0","id":<id>,"error":{"code":..,"message":..}}` with `id` as raw JSON.
pub fn error_item(id: &str, code: i32, message: &str) -> String {
    format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":{id},\"error\":{{\"code\":{code},\"message\":{}}}}}",
        quote(message)
    )
}

fn result_item(id: &str, result: &str) -> String {
    format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"result\":{result}}}")
}

/// Dispatch a request that passed `http::precheck`: parse body, handle each message, build the response
/// effect (`Effect::HttpReply`) or park a long poll (no reply yet).
pub fn dispatch(state: &mut McpState, req: HttpRequest, now_ms: i64) -> McpOutput {
    let mut out = McpOutput::default();
    match serde_json::from_slice::<Value>(&req.body) {
        Err(_) => {
            let body = error_item("null", -32700, "Parse error");
            out.effects
                .push(Effect::HttpReply { conn: req.conn, response: raw_response(400, Vec::new(), body) });
        }
        Ok(body) => {
            let header_session = req
                .headers
                .iter()
                .find(|(k, _)| k == "mcp-session-id")
                .map(|(_, v)| v.clone())
                .filter(|v| !v.is_empty());
            let (batch, rest) = match body {
                Value::Array(items) => (true, items.into_iter().collect()),
                other => (false, std::iter::once(other).collect()),
            };
            let cont = Cont {
                batch,
                rest,
                header_session,
                remote_port: req.remote_port,
                entropy: req.entropy,
                now_ms,
                ..Cont::default()
            };
            run(state, req.conn, cont, &mut out);
        }
    }
    settle(state, &mut out);
    out
}

/// Continue handling the items of a request until it is finished (reply pushed) or parks on a long poll.
fn run(state: &mut McpState, conn: ConnId, mut cont: Cont, out: &mut McpOutput) {
    while let Some(msg) = cont.rest.pop_front() {
        let obj = msg.as_object();
        let method = obj.and_then(|o| o.get("method")).and_then(Value::as_str);
        let Some(method) = method else {
            let id = obj.and_then(|o| o.get("id")).map_or_else(|| "null".to_string(), Value::to_string);
            cont.out.push(error_item(&id, -32600, "Invalid Request"));
            continue;
        };
        let Some(id) = obj.and_then(|o| o.get("id")).map(Value::to_string) else {
            continue; // notification
        };
        let empty = serde_json::Map::new();
        let params = obj.and_then(|o| o.get("params")).and_then(Value::as_object).unwrap_or(&empty);
        match handle(state, conn, &mut cont, method, params, out) {
            Handled::Item(result) => cont.out.push(result_or_error(&id, result)),
            Handled::Parked { wait_ms } => {
                if let Some(p) = state.inner.pollers.iter_mut().find(|p| p.conn == conn) {
                    p.rpc_id = id;
                    p.cont = cont;
                }
                out.effects.push(Effect::SetTimer {
                    id: TimerId::PollTimeout(conn),
                    after_ms: wait_ms,
                    background: true,
                });
                return;
            }
        }
    }
    finish_request(conn, cont, out);
}

enum Reply {
    Result(String),
    Error(i32, String),
}

fn result_or_error(id: &str, r: Reply) -> String {
    match r {
        Reply::Result(s) => result_item(id, &s),
        Reply::Error(code, msg) => error_item(id, code, &msg),
    }
}

enum Handled {
    Item(Reply),
    Parked { wait_ms: u64 },
}

fn handle(
    state: &mut McpState,
    conn: ConnId,
    cont: &mut Cont,
    method: &str,
    params: &serde_json::Map<String, Value>,
    out: &mut McpOutput,
) -> Handled {
    match method {
        "initialize" => {
            let want = params.get("protocolVersion").and_then(Value::as_str).unwrap_or("");
            let version = if PROTOCOL_VERSIONS.contains(&want) { want } else { PROTOCOL_VERSIONS[0] };
            let info = params.get("clientInfo").and_then(Value::as_object);
            let name = info.and_then(|o| o.get("name")).and_then(Value::as_str).unwrap_or("unknown");
            let ver = info.and_then(|o| o.get("version")).and_then(Value::as_str).unwrap_or("");
            let sid = uuid_v4(cont.entropy, cont.inits);
            cont.inits = cont.inits.wrapping_add(1);
            state.register_client(&sid, name, ver);
            cont.session = Some(sid);
            Handled::Item(Reply::Result(format!(
                "{{\"protocolVersion\":{},\"capabilities\":{{\"tools\":{{\"listChanged\":false}}}},\"serverInfo\":{{\"name\":\"xplain\",\"version\":\"{SERVER_VERSION}\"}},\"instructions\":{}}}",
                quote(version),
                quote(tools::instructions())
            )))
        }
        "ping" => Handled::Item(Reply::Result("{}".to_string())),
        "tools/list" => Handled::Item(Reply::Result(format!("{{\"tools\":{}}}", tools::tool_list()))),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            if !TOOL_NAMES.contains(&name) {
                return Handled::Item(Reply::Error(
                    -32602,
                    format!("Unknown tool: {}", cap_chars(name, 100)),
                ));
            }
            let client_id = cont
                .session
                .clone()
                .or_else(|| cont.header_session.clone())
                .unwrap_or_else(|| format!("anon:{}", cont.remote_port));
            let args = params.get("arguments").cloned().unwrap_or(Value::Null);
            match tools::call(state, &client_id, conn, name, &args, cont.now_ms) {
                ToolOutcome::Done { result, events } => {
                    out.events.extend(events);
                    Handled::Item(Reply::Result(result.to_string()))
                }
                ToolOutcome::Parked { wait_ms } => Handled::Parked { wait_ms },
            }
        }
        other => Handled::Item(Reply::Error(-32601, format!("Method not found: {}", cap_chars(other, 100)))),
    }
}

/// All items handled: push the HTTP reply.
fn finish_request(conn: ConnId, cont: Cont, out: &mut McpOutput) {
    let headers: Vec<(String, String)> =
        cont.session.iter().map(|s| ("mcp-session-id".to_string(), s.clone())).collect();
    let response = if cont.out.is_empty() {
        raw_response(202, headers, String::new())
    } else if cont.batch {
        raw_response(200, headers, format!("[{}]", cont.out.join(",")))
    } else {
        raw_response(200, headers, cont.out.join(","))
    };
    out.effects.push(Effect::HttpReply { conn, response });
}

/// A parked poll got its result: cancel its timer, add the item, continue the request.
pub(super) fn resume(state: &mut McpState, r: Resolved, out: &mut McpOutput) {
    let Resolved { poller, result, cancel_timer } = r;
    if cancel_timer {
        out.effects.push(Effect::CancelTimer(TimerId::PollTimeout(poller.conn)));
    }
    let text = tools::poll_text(&result);
    let item = result_item(&poller.rpc_id, &tools::text_result(&text, false).to_string());
    let mut cont = poller.cont;
    cont.out.push(item);
    run(state, poller.conn, cont, out);
}

/// Finish everything the hub resolved meanwhile (a resumed request may resolve or park more), sync the
/// client list and collect hub events.
pub(super) fn settle(state: &mut McpState, out: &mut McpOutput) {
    while !state.inner.backlog.is_empty() {
        let batch: Vec<Resolved> = std::mem::take(&mut state.inner.backlog);
        for r in batch {
            resume(state, r, out);
        }
    }
    state.sync_clients();
    out.events.extend(std::mem::take(&mut state.inner.events));
}

/// Result of a poll that ended by timeout.
pub(super) fn timeout_result() -> PollResult {
    PollResult::NoQuestion
}

/// UUID v4 text from 16 entropy bytes (`salt` keeps several ids of one request apart).
fn uuid_v4(mut b: [u8; 16], salt: u8) -> String {
    b[15] ^= salt;
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::{McpEndpoint, OutQuestion};

    const TOKEN: &str = "tok-0123456789abcdef";

    fn state() -> McpState {
        McpState {
            running: true,
            endpoint: Some(McpEndpoint {
                url: "http://127.0.0.1:1/mcp".into(),
                token: TOKEN.into(),
                port: 1,
            }),
            ..McpState::default()
        }
    }

    fn post(conn: u64, body: &str) -> HttpRequest {
        HttpRequest {
            conn: ConnId(conn),
            method: "POST".into(),
            path: "/mcp".into(),
            headers: vec![
                ("host".into(), "127.0.0.1:1".into()),
                ("authorization".into(), format!("Bearer {TOKEN}")),
            ],
            body: body.as_bytes().to_vec(),
            body_too_large: false,
            remote_port: 4000 + conn as u16,
            entropy: [conn as u8; 16],
        }
    }

    fn with_session(mut r: HttpRequest, sid: &str) -> HttpRequest {
        r.headers.push(("mcp-session-id".into(), sid.into()));
        r
    }

    /// The single reply of `out` for `conn`: (status, headers, body).
    type Reply3 = (u16, Vec<(String, String)>, String);

    fn reply(out: &McpOutput, conn: u64) -> Option<Reply3> {
        out.effects.iter().find_map(|e| match e {
            Effect::HttpReply { conn: c, response } if *c == ConnId(conn) => Some((
                response.status,
                response.headers.clone(),
                String::from_utf8_lossy(&response.body).to_string(),
            )),
            _ => None,
        })
    }

    fn body_json(out: &McpOutput, conn: u64) -> Value {
        let (_, _, b) = reply(out, conn).expect("reply");
        serde_json::from_str(&b).expect("json")
    }

    fn text_json(v: &Value) -> Value {
        serde_json::from_str(v["result"]["content"][0]["text"].as_str().expect("text")).expect("inner json")
    }

    fn call(s: &mut McpState, conn: u64, body: &str) -> McpOutput {
        s.handle_http(post(conn, body), 0)
    }

    #[test]
    fn f_mcpsrv_02_precheck_wired() {
        let mut s = state();
        let mut r = post(1, "{}");
        r.path = "/nope".into();
        let out = s.handle_http(r, 0);
        assert_eq!(reply(&out, 1).map(|x| x.0), Some(404));
        let mut r = post(2, "{}");
        r.headers.retain(|(k, _)| k != "authorization");
        assert_eq!(reply(&s.handle_http(r, 0), 2).map(|x| x.0), Some(401));
    }

    #[test]
    fn f_mcpsrv_02_parse_error() {
        let mut s = state();
        for b in ["", "{", "nope"] {
            let out = call(&mut s, 1, b);
            let (st, h, body) = reply(&out, 1).expect("reply");
            assert_eq!(st, 400);
            assert_eq!(
                body,
                "{\"jsonrpc\":\"2.0\",\"id\":null,\"error\":{\"code\":-32700,\"message\":\"Parse error\"}}"
            );
            assert!(h.contains(&("content-type".into(), "application/json".into())));
        }
    }

    #[test]
    fn f_mcpsrv_03_invalid_request_table() {
        let mut s = state();
        let cases = [
            ("5", "null"),
            ("\"x\"", "null"),
            ("null", "null"),
            ("{}", "null"),
            ("{\"id\":7}", "7"),
            ("{\"id\":\"a\",\"method\":5}", "\"a\""),
            ("{\"method\":5}", "null"),
        ];
        for (body, id) in cases {
            let out = call(&mut s, 1, body);
            let (st, _, b) = reply(&out, 1).expect("reply");
            assert_eq!(st, 200, "{body}");
            assert_eq!(
                b,
                format!(
                    "{{\"jsonrpc\":\"2.0\",\"id\":{id},\"error\":{{\"code\":-32600,\"message\":\"Invalid Request\"}}}}"
                ),
                "{body}"
            );
        }
    }

    #[test]
    fn f_mcpsrv_03_notifications_and_batches() {
        let mut s = state();
        // notification -> 202, empty body, no content-type
        let out = call(&mut s, 1, "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}");
        let (st, h, b) = reply(&out, 1).expect("reply");
        assert_eq!((st, b.as_str()), (202, ""));
        assert!(h.is_empty());
        // unknown method notification too
        assert_eq!(reply(&call(&mut s, 1, "{\"method\":\"zzz\"}"), 1).map(|x| x.0), Some(202));
        // empty batch
        let (st, _, b) = reply(&call(&mut s, 1, "[]"), 1).expect("reply");
        assert_eq!((st, b.as_str()), (202, ""));
        // batch of one -> array
        let (st, _, b) = reply(&call(&mut s, 1, "[{\"id\":1,\"method\":\"ping\"}]"), 1).expect("reply");
        assert_eq!(st, 200);
        assert_eq!(b, "[{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}]");
        // mixed batch keeps order, drops notifications
        let (_, _, b) = reply(
            &call(&mut s, 1, "[{\"id\":\"a\",\"method\":\"ping\"},{\"method\":\"ping\"},{\"id\":null,\"method\":\"ping\"},3]"),
            1,
        )
        .expect("reply");
        let v: Value = serde_json::from_str(&b).expect("json");
        let a = v.as_array().expect("array");
        assert_eq!(a.len(), 3);
        assert_eq!(a[0]["id"], "a");
        assert_eq!(a[1]["id"], Value::Null);
        assert_eq!(a[1]["result"], serde_json::json!({}));
        assert_eq!(a[2]["error"]["code"], -32600);
    }

    #[test]
    fn f_mcpsrv_03_id_echo_as_sent() {
        let mut s = state();
        let (_, _, b) =
            reply(&call(&mut s, 1, "{\"id\":{\"k\":[1,2]},\"method\":\"ping\"}"), 1).expect("reply");
        assert_eq!(b, "{\"jsonrpc\":\"2.0\",\"id\":{\"k\":[1,2]},\"result\":{}}");
        let (_, _, b) = reply(&call(&mut s, 1, "{\"id\":\"s\\\"x\",\"method\":\"ping\"}"), 1).expect("reply");
        assert_eq!(b, "{\"jsonrpc\":\"2.0\",\"id\":\"s\\\"x\",\"result\":{}}");
    }

    #[test]
    fn f_mcpsrv_03_unknown_method() {
        let mut s = state();
        let v = body_json(&call(&mut s, 1, "{\"id\":1,\"method\":\"nope/x\"}"), 1);
        assert_eq!(v["error"]["code"], -32601);
        assert_eq!(v["error"]["message"], "Method not found: nope/x");
        let long = "m".repeat(300);
        let v = body_json(&call(&mut s, 1, &format!("{{\"id\":1,\"method\":\"{long}\"}}")), 1);
        assert_eq!(v["error"]["message"], format!("Method not found: {}", "m".repeat(100)));
    }

    #[test]
    fn f_mcpsrv_04_initialize() {
        let mut s = state();
        let out = call(
            &mut s,
            1,
            "{\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-03-26\",\"clientInfo\":{\"name\":\"tester\",\"version\":\"9.1\"}}}",
        );
        let (st, h, b) = reply(&out, 1).expect("reply");
        assert_eq!(st, 200);
        let sid =
            h.iter().find(|(k, _)| k == "mcp-session-id").map(|(_, v)| v.clone()).expect("session header");
        assert_eq!(sid.len(), 36);
        assert_eq!(sid.as_bytes()[14], b'4');
        let v: Value = serde_json::from_str(&b).expect("json");
        assert_eq!(v["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(v["result"]["capabilities"], serde_json::json!({"tools": {"listChanged": false}}));
        assert_eq!(v["result"]["serverInfo"], serde_json::json!({"name": "xplain", "version": "0.1.0"}));
        assert_eq!(v["result"]["instructions"], tools::instructions());
        assert_eq!(s.clients.len(), 1);
        assert_eq!(
            (s.clients[0].name.as_str(), s.clients[0].version.as_str(), s.clients[0].id.as_str()),
            ("tester", "9.1", sid.as_str())
        );
        assert!(out.events.contains(&crate::mcp::HubEvent::ClientsChanged));
        // unknown protocol -> newest; non-string clientInfo -> unknown/empty
        let v = body_json(
            &call(
                &mut s,
                2,
                "{\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"1999\",\"clientInfo\":{\"name\":5,\"version\":[]}}}",
            ),
            2,
        );
        assert_eq!(v["result"]["protocolVersion"], "2025-06-18");
        assert_eq!((s.clients[1].name.as_str(), s.clients[1].version.as_str()), ("unknown", ""));
        // no params at all
        let v = body_json(&call(&mut s, 3, "{\"id\":1,\"method\":\"initialize\"}"), 3);
        assert_eq!(v["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(s.clients.len(), 3);
        assert_ne!(s.clients[0].id, s.clients[1].id);
    }

    #[test]
    fn f_mcpsrv_04_batch_initialize_then_call_uses_session() {
        let mut s = state();
        let out = call(
            &mut s,
            1,
            "[{\"id\":1,\"method\":\"initialize\",\"params\":{\"clientInfo\":{\"name\":\"n\"}}},{\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"next_question\",\"arguments\":{\"wait_seconds\":1}}}]",
        );
        // parked: no reply yet, timer armed, poller belongs to the new session
        assert!(reply(&out, 1).is_none());
        assert!(out.effects.contains(&Effect::SetTimer {
            id: TimerId::PollTimeout(ConnId(1)),
            after_ms: 1000,
            background: true
        }));
        assert_eq!(s.inner.pollers.len(), 1);
        assert_eq!(s.inner.pollers[0].client_id, s.clients[0].id);
        assert!(s.clients[0].polling);
        // timeout finishes the batch: [init result, no_question_yet]
        let out = s.poll_timeout(ConnId(1));
        let (st, h, b) = reply(&out, 1).expect("reply");
        assert_eq!(st, 200);
        assert!(h.iter().any(|(k, _)| k == "mcp-session-id"));
        let v: Value = serde_json::from_str(&b).expect("json");
        assert_eq!(v[0]["id"], 1);
        assert_eq!(v[1]["id"], 2);
        assert_eq!(text_json(&v[1])["status"], "no_question_yet");
        assert!(!s.clients[0].polling);
    }

    #[test]
    fn f_mcpsrv_04_ping_and_tools_list() {
        let mut s = state();
        let v = body_json(&call(&mut s, 1, "{\"id\":1,\"method\":\"ping\"}"), 1);
        assert_eq!(v["result"], serde_json::json!({}));
        let v = body_json(&call(&mut s, 1, "{\"id\":1,\"method\":\"tools/list\"}"), 1);
        let names: Vec<&str> = v["result"]["tools"]
            .as_array()
            .expect("tools")
            .iter()
            .map(|t| t["name"].as_str().expect("n"))
            .collect();
        assert_eq!(names, ["next_question", "answer", "get_questions", "annotate", "files_changed"]);
    }

    #[test]
    fn f_mcpsrv_05_tools_call_errors_and_shape() {
        let mut s = state();
        let v = body_json(
            &call(&mut s, 1, "{\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"zap\"}}"),
            1,
        );
        assert_eq!(
            (v["error"]["code"].clone(), v["error"]["message"].clone()),
            (serde_json::json!(-32602), serde_json::json!("Unknown tool: zap"))
        );
        let v =
            body_json(&call(&mut s, 1, "{\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":5}}"), 1);
        assert_eq!(v["error"]["message"], "Unknown tool: ");
        let v = body_json(&call(&mut s, 1, "{\"id\":1,\"method\":\"tools/call\"}"), 1);
        assert_eq!(v["error"]["message"], "Unknown tool: ");
        // tool error: isError
        let v = body_json(
            &call(
                &mut s,
                1,
                "{\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"answer\",\"arguments\":7}}",
            ),
            1,
        );
        assert_eq!(v["result"]["isError"], true);
        assert_eq!(v["result"]["content"][0]["type"], "text");
        assert_eq!(v["result"]["content"][0]["text"], "thread_id and text (strings) are required");
        // ok: no isError
        let v = body_json(
            &call(
                &mut s,
                1,
                "{\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"files_changed\",\"arguments\":{\"paths\":[\"a\"]}}}",
            ),
            1,
        );
        assert!(v["result"].get("isError").is_none());
        assert_eq!(v["result"]["content"][0]["text"], "{\"ok\":true}");
    }

    fn enqueue(s: &mut McpState, thread: &str, turn: u32, msg: &str) -> McpOutput {
        s.enqueue(OutQuestion {
            thread_id: thread.into(),
            turn,
            question: msg.into(),
            follow_up: turn > 1,
            previous: if turn > 1 { vec![(1, "q".into(), "a".into())] } else { vec![] },
        })
    }

    fn next_q(s: &mut McpState, conn: u64, sid: &str, wait: u32) -> McpOutput {
        s.handle_http(
            with_session(
                post(conn, &format!("{{\"id\":9,\"method\":\"tools/call\",\"params\":{{\"name\":\"next_question\",\"arguments\":{{\"wait_seconds\":{wait}}}}}}}")),
                sid,
            ),
            0,
        )
    }

    #[test]
    fn f_mcpsrv_06_immediate_when_queued() {
        let mut s = state();
        let out = enqueue(&mut s, "q1", 1, "hello\n\nFile: a");
        assert!(out.effects.is_empty());
        assert_eq!(s.queue.len(), 1);
        let out = next_q(&mut s, 1, "A", 120);
        let v = body_json(&out, 1);
        assert_eq!(
            v["result"]["content"][0]["text"],
            "{\"status\":\"question\",\"thread_id\":\"q1\",\"turn\":1,\"follow_up\":false,\"question\":\"hello\\n\\nFile: a\"}"
        );
        assert_eq!(s.delivered, 1);
        assert!(s.queue.is_empty());
        assert!(out.events.iter().any(
            |e| matches!(e, crate::mcp::HubEvent::Delivered { thread_id, turn: 1, .. } if thread_id == "q1")
        ));
        assert!(!out.effects.iter().any(|e| matches!(e, Effect::SetTimer { .. })));
    }

    #[test]
    fn f_mcpsrv_06_wake_parked_poll() {
        let mut s = state();
        let out = next_q(&mut s, 1, "A", 30);
        assert!(reply(&out, 1).is_none());
        assert_eq!(
            out.effects,
            vec![Effect::SetTimer {
                id: TimerId::PollTimeout(ConnId(1)),
                after_ms: 30_000,
                background: true
            }]
        );
        assert_eq!(s.clients[0].id, "A");
        assert_eq!(s.clients[0].name, "unknown");
        assert!(s.clients[0].polling);
        let out = enqueue(&mut s, "q1", 1, "hey");
        assert_eq!(out.effects[0], Effect::CancelTimer(TimerId::PollTimeout(ConnId(1))));
        let v = body_json(&out, 1);
        assert_eq!(text_json(&v)["question"], "hey");
        assert_eq!(v["id"], 9);
        assert!(!s.clients[0].polling);
        assert_eq!(s.delivered, 1);
    }

    #[test]
    fn f_mcpsrv_06_timeout_and_conn_closed() {
        let mut s = state();
        next_q(&mut s, 1, "A", 1);
        let out = s.poll_timeout(ConnId(1));
        assert_eq!(text_json(&body_json(&out, 1))["status"], "no_question_yet");
        assert!(s.inner.pollers.is_empty());
        // late timer: nothing
        assert!(s.poll_timeout(ConnId(1)).effects.is_empty());
        // dropped connection cancels the poll, question stays queued
        next_q(&mut s, 2, "B", 1);
        let out = s.conn_closed(ConnId(2));
        assert!(reply(&out, 2).is_none());
        assert_eq!(out.effects, vec![Effect::CancelTimer(TimerId::PollTimeout(ConnId(2)))]);
        enqueue(&mut s, "q1", 1, "x");
        assert_eq!(s.queue.len(), 1);
        assert_eq!(s.delivered, 0);
    }

    #[test]
    fn f_mcpsrv_06_same_client_new_poll_answers_old() {
        let mut s = state();
        next_q(&mut s, 1, "A", 45);
        let out = next_q(&mut s, 2, "A", 45);
        assert_eq!(text_json(&body_json(&out, 1))["status"], "no_question_yet");
        assert!(reply(&out, 2).is_none());
        assert_eq!(s.inner.pollers.len(), 1);
        assert_eq!(s.inner.pollers[0].conn, ConnId(2));
    }

    #[test]
    fn f_mcpsrv_06_poller_arrival_order() {
        let mut s = state();
        next_q(&mut s, 1, "A", 45);
        next_q(&mut s, 2, "B", 45);
        let out = enqueue(&mut s, "q1", 1, "one");
        assert!(reply(&out, 1).is_some() && reply(&out, 2).is_none());
        let out = enqueue(&mut s, "q2", 1, "two");
        assert!(reply(&out, 2).is_some());
    }

    #[test]
    fn f_mcpsrv_06_sticky_follow_up() {
        let mut s = state();
        // A gets q1
        next_q(&mut s, 1, "A", 45);
        enqueue(&mut s, "q1", 1, "one");
        // A and B poll (A first), follow-up on q1 must go to A even though B polls too
        next_q(&mut s, 2, "B", 45);
        next_q(&mut s, 3, "A", 45);
        let out = enqueue(&mut s, "q1", 2, "again");
        assert!(reply(&out, 3).is_some(), "A gets follow-up");
        assert!(reply(&out, 2).is_none());
        // A no longer polling: B may take a follow-up of q1
        let out = enqueue(&mut s, "q1", 3, "third");
        assert!(reply(&out, 2).is_some());
    }

    #[test]
    fn f_mcpsrv_06_follow_up_result_and_trim() {
        let mut s = state();
        s.enqueue(OutQuestion {
            thread_id: "q1".into(),
            turn: 8,
            question: "Follow-up to your earlier answer (thread q1, turn 8): z".into(),
            follow_up: true,
            previous: (1..8).map(|i| (i, format!("m{i}"), "x".repeat(4100))).collect(),
        });
        let v = body_json(&next_q(&mut s, 1, "A", 1), 1);
        let t = text_json(&v);
        let prev = t["previous"].as_array().expect("previous");
        assert_eq!(prev.len(), 5);
        assert_eq!(prev[0]["turn"], 3);
        assert_eq!(prev[0]["answer"].as_str().expect("a").len(), 4000);
        assert_eq!(t["follow_up"], true);
    }

    #[test]
    fn f_mcpsrv_07_answer_after_delivery() {
        let mut s = state();
        enqueue(&mut s, "q1", 1, "x");
        next_q(&mut s, 1, "A", 1);
        let out = call(
            &mut s,
            2,
            "{\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"answer\",\"arguments\":{\"thread_id\":\"q1\",\"text\":\"yes\"}}}",
        );
        assert_eq!(
            out.events,
            vec![crate::mcp::HubEvent::Answer { thread_id: "q1".into(), turn: 1, text: "yes".into() }]
        );
        assert_eq!(text_json(&body_json(&out, 2))["ok"], true);
    }

    #[test]
    fn f_mcpsrv_11_anon_client_per_connection() {
        let mut s = state();
        let out = s.handle_http(
            post(1, "{\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"next_question\",\"arguments\":{\"wait_seconds\":1}}}"),
            0,
        );
        assert!(reply(&out, 1).is_none());
        assert_eq!(s.clients[0].id, "anon:4001");
        assert_eq!(s.clients[0].name, "unknown");
    }

    #[test]
    fn f_mcpui_03_stop_answers_polls_closed() {
        let mut s = state();
        next_q(&mut s, 1, "A", 45);
        next_q(&mut s, 2, "B", 45);
        enqueue(&mut s, "q9", 1, "left over");
        // q9 already went to A; queue another undeliverable one for a third client
        s.queue.push(OutQuestion {
            thread_id: "qz".into(),
            turn: 1,
            question: "z".into(),
            follow_up: false,
            previous: vec![],
        });
        s.delivered = 5;
        let out = s.stop();
        let closed = "{\"status\":\"closed\",\"note\":\"xplain closed the session. Stop.\"}";
        assert_eq!(text_json(&body_json(&out, 2))["status"], "closed");
        assert_eq!(body_json(&out, 2)["result"]["content"][0]["text"], closed);
        let (st, _, _) = reply(&out, 2).expect("reply");
        assert_eq!(st, 200);
        assert!(s.clients.is_empty() && s.queue.is_empty() && s.inner.pollers.is_empty());
        assert_eq!(s.delivered, 5);
        assert!(out.events.contains(&crate::mcp::HubEvent::ClientsChanged));
        // stop reply comes before anything else the reducer appends; batch tail after closed poll
        let mut s = state();
        s.handle_http(
            post(3, "[{\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"next_question\"}},{\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"next_question\"}},{\"id\":3,\"method\":\"ping\"}]"),
            0,
        );
        let out = s.stop();
        let v = body_json(&out, 3);
        let a = v.as_array().expect("array");
        assert_eq!(a.len(), 3);
        assert_eq!(text_json(&a[0])["status"], "closed");
        assert_eq!(text_json(&a[1])["status"], "closed");
        assert_eq!(a[2]["result"], serde_json::json!({}));
    }

    #[test]
    fn f_mcpsrv_11_client_list_polling_flag() {
        let mut s = state();
        let out = call(
            &mut s,
            1,
            "{\"id\":1,\"method\":\"initialize\",\"params\":{\"clientInfo\":{\"name\":\"a\"}}}",
        );
        let sid = out
            .effects
            .iter()
            .find_map(|e| match e {
                Effect::HttpReply { response, .. } => {
                    response.headers.iter().find(|(k, _)| k == "mcp-session-id").map(|(_, v)| v.clone())
                }
                _ => None,
            })
            .expect("sid");
        assert!(!s.clients[0].polling);
        next_q(&mut s, 2, &sid, 5);
        assert!(s.clients[0].polling);
        s.conn_closed(ConnId(2));
        assert!(!s.clients[0].polling);
    }
}
