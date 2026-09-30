//! Hub bookkeeping: question queue, parked long polls, sticky thread->client map, sessions, client list,
//! counters.
//!
//! Spec: F-MCPSRV-06 (queue order, poll wake/close/timeout, sticky routing, `previous` trimming to last 5 /
//! 4000 chars), F-MCPSRV-09 (`get_questions`), F-MCPSRV-11 (clients, `polling`, counters `delivered`),
//! F-MCPUI-03 (stop answers polls `closed`). Oracle: `src/mcp/hub.ts`. Owner: component `agent` (E).
//! Must not: build HTTP responses beyond the effect helpers it is given by rpc.rs.

use std::collections::VecDeque;

use serde_json::Value;

use super::text::{cap_chars, sanitize};
use super::{ClientInfo, ConnId, HubEvent, McpState, OutQuestion};

/// Max entries of `previous` and max chars per entry text (F-MCPSRV-06).
pub const MAX_HISTORY: usize = 5;
pub const MAX_HISTORY_TEXT: usize = 4000;

/// Rest of a JSON-RPC request that is parked on a long poll: what to do once the poll resolves.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Cont {
    /// Request body was an array (response is an array).
    pub batch: bool,
    /// Response items produced so far (raw JSON), in order.
    pub out: Vec<String>,
    /// Batch items not handled yet.
    pub rest: VecDeque<Value>,
    /// Session id created by `initialize` in this request (response header, client id of later items).
    pub session: Option<String>,
    /// `mcp-session-id` request header (non-empty).
    pub header_session: Option<String>,
    pub remote_port: u16,
    pub entropy: [u8; 16],
    /// Number of `initialize` calls so far in this request (session id derivation).
    pub inits: u8,
}

/// One parked `next_question` long poll.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Poller {
    pub conn: ConnId,
    pub client_id: String,
    /// JSON-RPC id to answer with (raw JSON text).
    pub rpc_id: String,
    /// Remainder of the request this poll belongs to.
    pub cont: Cont,
}

/// Outcome of one long poll.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PollResult {
    Question(OutQuestion),
    NoQuestion,
    Closed,
}

/// A poller that got its result and whose request must continue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub poller: Poller,
    pub result: PollResult,
    /// Arm-timer must be cancelled (not needed when the timer fired itself).
    pub cancel_timer: bool,
}

/// Internal state of the hub (add fields as needed).
#[derive(Debug, Clone, Default)]
pub struct HubInner {
    pub pollers: Vec<Poller>,
    /// `thread_id -> client_id` stickiness.
    pub sticky: Vec<(String, String)>,
    /// `thread_id -> last delivered turn`.
    pub turns: Vec<(String, u32)>,
    /// Polls resolved by a tool call, waiting for the request to continue (drained by `settle`).
    pub backlog: Vec<Resolved>,
    /// Hub events emitted outside a tool call result.
    pub events: Vec<HubEvent>,
    /// `stop` in progress: new polls answer `closed` at once.
    pub closing: bool,
}

impl HubInner {
    /// Trim `previous` to the last 5 entries of at most 4000 chars each (F-MCPSRV-06).
    pub fn trim_history(q: &mut OutQuestion) {
        let skip = q.previous.len().saturating_sub(MAX_HISTORY);
        q.previous.drain(..skip);
        for (_, question, answer) in &mut q.previous {
            *question = cap_chars(&sanitize(question), MAX_HISTORY_TEXT);
            *answer = cap_chars(&sanitize(answer), MAX_HISTORY_TEXT);
        }
    }

    fn upsert<T: PartialEq + Clone>(list: &mut Vec<(String, T)>, key: &str, val: T) {
        match list.iter_mut().find(|(k, _)| k == key) {
            Some(e) => e.1 = val,
            None => list.push((key.to_string(), val)),
        }
    }
}

impl McpState {
    /// Client name for an id (`unknown` when not registered).
    pub fn client_name(&self, id: &str) -> String {
        self.clients.iter().find(|c| c.id == id).map_or_else(|| "unknown".to_string(), |c| c.name.clone())
    }

    /// `initialize`: add a client entry (name max 200, version max 100, sanitized).
    pub fn register_client(&mut self, id: &str, name: &str, version: &str) {
        let info = ClientInfo {
            id: id.to_string(),
            name: cap_chars(&sanitize(name), 200),
            version: cap_chars(&sanitize(version), 100),
            polling: false,
        };
        match self.clients.iter_mut().find(|c| c.id == id) {
            Some(c) => *c = info,
            None => self.clients.push(info),
        }
        self.inner.events.push(HubEvent::ClientsChanged);
    }

    /// Make sure a client entry exists (poll without `initialize`: name `unknown`).
    pub(super) fn touch(&mut self, id: &str) {
        if !self.clients.iter().any(|c| c.id == id) {
            self.clients.push(ClientInfo {
                id: id.to_string(),
                name: "unknown".to_string(),
                version: String::new(),
                polling: false,
            });
        }
    }

    /// Recompute `polling` flags.
    pub(super) fn sync_clients(&mut self) {
        let inner = &self.inner;
        for c in &mut self.clients {
            c.polling = inner.pollers.iter().any(|p| p.client_id == c.id);
        }
    }

    /// Index of the first queued question `client_id` may take (sticky rule), if any.
    pub(super) fn pick(&self, client_id: &str) -> Option<usize> {
        self.queue.iter().position(|q| match self.inner.sticky.iter().find(|(t, _)| *t == q.thread_id) {
            None => true,
            Some((_, s)) => s == client_id || !self.inner.pollers.iter().any(|p| p.client_id == *s),
        })
    }

    /// Take queue item `i` for `client_id`: counters, stickiness, events. Returns the question.
    pub(super) fn take(&mut self, i: usize, client_id: &str) -> OutQuestion {
        let q = self.queue.remove(i);
        self.delivered += 1;
        HubInner::upsert(&mut self.inner.sticky, &q.thread_id, client_id.to_string());
        HubInner::upsert(&mut self.inner.turns, &q.thread_id, q.turn);
        let client_name = self.client_name(client_id);
        self.inner.events.push(HubEvent::Delivered {
            thread_id: q.thread_id.clone(),
            turn: q.turn,
            client_name,
        });
        self.inner.events.push(HubEvent::ClientsChanged);
        q
    }

    /// Hand queued questions to waiting pollers (arrival order) until nothing more matches.
    pub(super) fn dispatch(&mut self) {
        loop {
            let mut progressed = false;
            for pi in 0..self.inner.pollers.len() {
                let cid = self.inner.pollers[pi].client_id.clone();
                if let Some(qi) = self.pick(&cid) {
                    let q = self.take(qi, &cid);
                    let poller = self.inner.pollers.remove(pi);
                    self.inner.backlog.push(Resolved {
                        poller,
                        result: PollResult::Question(q),
                        cancel_timer: true,
                    });
                    progressed = true;
                    break;
                }
            }
            if !progressed || self.queue.is_empty() {
                break;
            }
        }
    }

    /// Start a poll for `client_id` on `conn`. `Some(result)` = answered at once; `None` = parked (caller
    /// fills `rpc_id` / `cont` of the new poller and arms the timer).
    pub(super) fn poll(&mut self, client_id: &str, conn: ConnId) -> Option<PollResult> {
        if self.inner.closing {
            return Some(PollResult::Closed);
        }
        self.touch(client_id);
        if let Some(pos) = self.inner.pollers.iter().position(|p| p.client_id == client_id) {
            let old = self.inner.pollers.remove(pos);
            self.inner.backlog.push(Resolved {
                poller: old,
                result: PollResult::NoQuestion,
                cancel_timer: true,
            });
        }
        if let Some(i) = self.pick(client_id) {
            return Some(PollResult::Question(self.take(i, client_id)));
        }
        self.inner.pollers.push(Poller {
            conn,
            client_id: client_id.to_string(),
            rpc_id: String::new(),
            cont: Cont::default(),
        });
        self.inner.events.push(HubEvent::ClientsChanged);
        None
    }

    /// Turn of the last delivery of `thread_id`, `None` when never delivered.
    pub(super) fn delivered_turn(&self, thread_id: &str) -> Option<u32> {
        self.inner.turns.iter().find(|(t, _)| t == thread_id).map(|(_, n)| *n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(thread: &str, turn: u32) -> OutQuestion {
        OutQuestion {
            thread_id: thread.into(),
            turn,
            question: format!("{thread}/{turn}"),
            preview: format!("{thread}/{turn}"),
            follow_up: turn > 1,
            previous: Vec::new(),
        }
    }

    #[test]
    fn f_mcpsrv_06_trim_history_last_five_4000() {
        let mut oq = q("q1", 8);
        oq.previous = (1..=7).map(|i| (i, "a".repeat(5000), format!("ans{i}"))).collect();
        HubInner::trim_history(&mut oq);
        assert_eq!(oq.previous.len(), 5);
        assert_eq!(oq.previous[0].0, 3);
        assert_eq!(oq.previous[4].0, 7);
        assert_eq!(oq.previous[0].1.chars().count(), 4000);
        assert_eq!(oq.previous[0].2, "ans3");
    }

    #[test]
    fn f_mcpsrv_06_sticky_pick() {
        let mut s = McpState::default();
        s.queue.push(q("q1", 2));
        s.queue.push(q("q2", 1));
        s.inner.sticky.push(("q1".into(), "A".into()));
        // A is polling: B may not take q1, takes q2
        s.inner.pollers.push(Poller {
            conn: ConnId(1),
            client_id: "A".into(),
            rpc_id: "1".into(),
            cont: Cont::default(),
        });
        assert_eq!(s.pick("B"), Some(1));
        assert_eq!(s.pick("A"), Some(0));
        // A gone: anyone takes q1
        s.inner.pollers.clear();
        assert_eq!(s.pick("B"), Some(0));
    }

    #[test]
    fn f_mcpsrv_11_take_counts_and_events() {
        let mut s = McpState::default();
        s.register_client("A", "agent-x", "1");
        s.queue.push(q("q1", 1));
        let got = s.take(0, "A");
        assert_eq!(got.thread_id, "q1");
        assert_eq!(s.delivered, 1);
        assert_eq!(s.delivered_turn("q1"), Some(1));
        assert!(
            s.inner
                .events
                .iter()
                .any(|e| matches!(e, HubEvent::Delivered { client_name, .. } if client_name == "agent-x"))
        );
    }
}
