//! Ask flow: sending comments to the agent, thread turns, applying MCP hub events to comments.
//!
//! Spec: F-ASK-01 (enqueue, ids), F-ASK-02 (`a` ask focused), F-ASK-03 (follow-up), F-ASK-04 (`A` ask all),
//! F-ASK-05 (status/spinner), F-ASK-09 (answer arrival, second answer on done turn), F-COMMENT-10 (agent
//! notes), F-MCPSRV-06 (question text incl. context), F-RELOAD-02 (`files_changed` reload trigger).
//! Oracle: `src/ask/{controller,prompt}.ts`, `src/useAsk.ts`, `src/mcp/bridge.ts`.
//! Owner: component `agent` (E). Must not: format thread boxes (thread_layout) or open sockets.

use std::collections::HashMap;

use crate::comments::{self, Answer, AnswerStatus, Comment, Origin, PaneSide, Turn};
use crate::effect::{Effect, Fx};
use crate::event::TimerId;
use crate::mcp::tools::sanitize;
use crate::mcp::{HubEvent, McpOutput, OutQuestion};
use crate::messages::{CANT_FOLLOW_UP, FOLLOW_UP_QUEUED, MCP_OFF};
use crate::state::State;
use crate::thread::ThreadScroll;

/// Spinner frame period (F-ASK-05).
pub const SPINNER_MS: u64 = 80;
/// Number of spinner frames.
pub const SPINNER_FRAMES: usize = 10;

const NOTE_TURN: &str = "(note you added with annotate)";

/// Ask bookkeeping (add fields here), e.g. spinner running flag.
#[derive(Debug, Clone, Default)]
pub struct AskState {
    pub spinner_running: bool,
    /// `thread id -> agent name` of the last delivery (answers carry it, F-ASK-05).
    pub agents: HashMap<String, String>,
}

/// File / side / lines / code block + surrounding context (no question text), F-MCPSRV-06.
pub fn build_context(c: &Comment) -> String {
    let sel_start = c.selection.as_ref().and_then(|s| s.start_line);
    let sel_end = c.selection.as_ref().and_then(|s| s.end_line);
    let range = match (sel_start, sel_end) {
        (Some(a), Some(b)) if a != b => Some(format!("{a}-{b}")),
        _ => sel_start.or(c.line).map(|n| n.to_string()),
    };
    let mut l: Vec<String> = Vec::new();
    l.push(format!("File: {}", c.file));
    l.push(format!(
        "Side: {}",
        if c.side == PaneSide::Old { "old (before the change)" } else { "new (after the change)" }
    ));
    if let Some(r) = range {
        l.push(format!("Lines: {r}"));
    }
    l.push("".into());
    l.push((if sel_start.is_some() { "Selected code:" } else { "Code at cursor line:" }).into());
    l.push("```".into());
    l.push(c.text.clone());
    l.push("```".into());
    if !c.context.is_empty() {
        l.push("".into());
        l.push("Surrounding context:".into());
        l.push("```".into());
        l.extend(c.context.iter().cloned());
        l.push("```".into());
    }
    l.join("\n")
}

/// Context text sent with a reply to an agent note (F-MCPSRV-06).
fn note_context(c: &Comment) -> String {
    let mut l: Vec<String> = Vec::new();
    match c.number {
        Some(n) => l.push(format!("Reply to your annotate note #{n}:")),
        None => l.push("Reply to your annotate note:".into()),
    }
    l.push(format!("File: {}", c.file));
    l.push(format!("Side: {}", if c.side == PaneSide::Old { "old" } else { "new" }));
    if let Some(n) = c.line {
        l.push(format!("Line: {n}"));
    }
    l.push("".into());
    l.push("Your note:".into());
    l.push(c.turns.first().map_or(c.message.as_str(), |t| t.message.as_str()).into());
    l.join("\n")
}

fn turn_message(c: &Comment, turn: u32) -> &str {
    c.turns.get((turn as usize).saturating_sub(1)).map_or(c.message.as_str(), |t| t.message.as_str())
}

/// `<q>` text sent to the agent: message plus code context (F-MCPSRV-06). `turn` is 1-based.
pub fn build_question(comment: &Comment, turn: u32) -> OutQuestion {
    let message = sanitize(turn_message(comment, turn));
    if turn <= 1 {
        let ctx = sanitize(&build_context(comment));
        let question = if ctx.is_empty() { message } else { format!("{message}\n\n{ctx}") };
        return OutQuestion {
            thread_id: comment.id.clone(),
            turn: 1,
            question,
            follow_up: false,
            previous: Vec::new(),
        };
    }
    let agent_note = comment.origin == Origin::Agent;
    let previous = comment
        .turns
        .iter()
        .take(turn as usize - 1)
        .enumerate()
        .map(|(i, t)| {
            let n = i as u32 + 1;
            if agent_note && i == 0 {
                (n, NOTE_TURN.to_string(), t.message.clone())
            } else {
                let answers: Vec<&str> =
                    t.prior.iter().chain(t.answer.iter()).map(|a| a.text.as_str()).collect();
                (n, t.message.clone(), answers.join("\n\n"))
            }
        })
        .collect();
    let mut question =
        format!("Follow-up to your earlier answer (thread {}, turn {turn}): {message}", comment.id);
    if agent_note {
        question.push_str("\n\n");
        question.push_str(&sanitize(&note_context(comment)));
    }
    OutQuestion { thread_id: comment.id.clone(), turn, question, follow_up: true, previous }
}

fn comment_mut<'a>(state: &'a mut State, id: &str) -> Option<&'a mut Comment> {
    state.comments.iter_mut().find(|c| c.id == id)
}

/// Mark turn `turn` (1-based) pending, queue it, fold the hub output.
fn send_turn(state: &mut State, id: &str, turn: u32, fx: &mut Fx) {
    let Some(c) = comment_mut(state, id) else { return };
    let Some(t) = c.turns.get_mut(turn as usize - 1) else { return };
    // before enqueue: delivery may be immediate
    t.answer = Some(Answer { status: AnswerStatus::Pending, text: String::new(), agent: None });
    let c = &*c;
    let q = build_question(c, turn);
    let preview = sanitize(turn_message(c, turn));
    let out = state.mcp.enqueue_with_preview(q, &preview);
    apply_output(state, out, fx);
}

/// Send a comment (new thread or follow-up turn) to the agent queue: mark pending, `state.mcp.enqueue`,
/// merge effects (`apply_output`), arm spinner. A comment whose last turn (of several) has no answer yet is a
/// follow-up turn just added by the editor; otherwise turn 1 of an askable comment is sent. No note on
/// success (the caller words it); MCP off: note `MCP is off (M to start)`.
pub fn ask_comment(state: &mut State, id: &str, fx: &mut Fx) {
    let Some(c) = comments::find(state, id) else { return };
    let turns = c.turns.len();
    let pending_follow_up = turns > 1 && c.turns.last().is_some_and(|t| t.answer.is_none());
    if !pending_follow_up && !askable(state, id) {
        return;
    }
    if !state.mcp.running {
        state.set_note(MCP_OFF);
        return;
    }
    let turn = if pending_follow_up { turns as u32 } else { 1 };
    send_turn(state, id, turn, fx);
}

/// `a` on a focused comment (F-ASK-02): notes, follow-up editor, or queue turn 1.
pub fn ask_focused(state: &mut State, id: &str, fx: &mut Fx) {
    let Some(c) = comments::find(state, id) else { return };
    let latest = comments::latest_answer(c);
    if latest.is_some_and(comments::is_live) {
        state.set_note("still waiting for the agent");
        return;
    }
    let agent = comments::is_agent_note(c);
    if comments::can_reply(c) {
        crate::editor::open_follow_up(state, id);
        return;
    }
    if agent {
        state.set_note("can't reply to this note yet");
        return;
    }
    if !askable(state, id) {
        state.set_note("can't retry a follow-up yet");
        return;
    }
    if !state.mcp.running {
        state.set_note(MCP_OFF);
        return;
    }
    send_turn(state, id, 1, fx);
    state.set_note("question queued");
}

/// Follow-up editor Enter (F-ASK-03): add turn `message`, queue it. `false` = refused (note set, editor stays).
pub fn follow_up(state: &mut State, id: &str, message: &str, fx: &mut Fx) -> bool {
    if !state.mcp.running {
        state.set_note(MCP_OFF);
        return false;
    }
    if !comments::find(state, id).is_some_and(comments::can_follow_up) {
        state.set_note(CANT_FOLLOW_UP);
        return false;
    }
    let Some(turn) = comments::append_turn(state, id, message) else { return false };
    state.thread.scrolls.insert(id.to_string(), ThreadScroll { off: 0, follow: true });
    send_turn(state, id, turn, fx);
    state.set_note(FOLLOW_UP_QUEUED);
    true
}

/// `A`: ask every askable comment of the current file (F-ASK-04).
pub fn ask_all(state: &mut State, fx: &mut Fx) {
    let todo: Vec<String> =
        comments::ids_in_file(state).into_iter().filter(|id| askable(state, id)).collect();
    if todo.is_empty() {
        state.set_note("nothing to ask");
        return;
    }
    if !state.mcp.running {
        state.set_note(MCP_OFF);
        return;
    }
    for id in &todo {
        send_turn(state, id, 1, fx);
    }
    let n = todo.len();
    state.set_note(format!("queued {n} question{}", if n == 1 { "" } else { "s" }));
}

/// Whether a comment may be asked now (F-ASK-02 preconditions).
pub fn askable(state: &State, id: &str) -> bool {
    comments::find(state, id).is_some_and(comments::can_ask)
}

fn any_live(state: &State) -> bool {
    state.comments.iter().any(|c| comments::latest_answer(c).is_some_and(comments::is_live))
}

fn arm_spinner(state: &mut State, fx: &mut Fx) {
    if !state.ask.spinner_running && any_live(state) {
        state.ask.spinner_running = true;
        fx.push(Effect::SetTimer { id: TimerId::Spinner, after_ms: SPINNER_MS, background: true });
    }
}

/// Live answers become `cancelled` with text `MCP stopped` (F-MCPUI-03).
pub fn cancel_live(state: &mut State) {
    for c in &mut state.comments {
        if let Some(a) = c.turns.last_mut().and_then(|t| t.answer.as_mut()) {
            if comments::is_live(a) {
                *a = Answer { status: AnswerStatus::Cancelled, text: "MCP stopped".to_string(), agent: None };
            }
        }
    }
}

fn on_delivered(state: &mut State, thread_id: &str, client_name: &str) {
    // the agent is the delivering client's name; an unnamed client gives none
    let agent = (!client_name.is_empty() && client_name != "unknown").then(|| client_name.to_string());
    if let Some(name) = &agent {
        state.ask.agents.insert(thread_id.to_string(), name.clone());
    }
    let Some(c) = comment_mut(state, thread_id) else { return };
    if let Some(a) = c.turns.last_mut().and_then(|t| t.answer.as_mut()) {
        if comments::is_live(a) {
            *a = Answer { status: AnswerStatus::Streaming, text: String::new(), agent };
        }
    }
}

fn on_answer(state: &mut State, thread_id: &str, turn: u32, text: &str) {
    let agent = state.ask.agents.get(thread_id).cloned();
    let Some(c) = comment_mut(state, thread_id) else { return };
    let Some(t) = (turn as usize).checked_sub(1).and_then(|i| c.turns.get_mut(i)) else { return };
    if let Some(old) = t.answer.take() {
        if old.status == AnswerStatus::Done {
            t.prior.push(old.clone());
        }
    }
    t.answer = Some(Answer { status: AnswerStatus::Done, text: text.to_string(), agent });
}

fn on_annotate(state: &mut State, file: &str, line: u32, text: &str, side: PaneSide, number: Option<u32>) {
    let comment = Comment {
        id: String::new(),
        file: file.to_string(),
        row: 0,
        side,
        line: Some(line),
        deleted: false,
        text: String::new(),
        message: text.to_string(),
        selection: None,
        context: Vec::new(),
        wide: Vec::new(),
        turns: vec![Turn { message: text.to_string(), answer: None, prior: Vec::new() }],
        origin: Origin::Agent,
        number,
        seq: 0,
    };
    comments::insert(state, comment);
}

/// Fold an `McpOutput` into state and effects: hub events -> answers/annotations/clients/`files_changed`
/// (-> `reload::on_files_changed`), effects appended to `fx` in order.
pub fn apply_output(state: &mut State, out: McpOutput, fx: &mut Fx) {
    fx.extend(out.effects);
    for ev in out.events {
        match ev {
            HubEvent::Delivered { thread_id, client_name, .. } => {
                on_delivered(state, &thread_id, &client_name)
            }
            HubEvent::Answer { thread_id, turn, text } => on_answer(state, &thread_id, turn, &text),
            HubEvent::Annotate { file, line, text, side, number } => {
                on_annotate(state, &file, line, &text, side, number);
            }
            HubEvent::FilesChanged { paths } => crate::reload::on_files_changed(state, &paths, fx),
            HubEvent::ClientsChanged => {}
        }
    }
    arm_spinner(state, fx);
}

/// `TimerId::Spinner`: advance frame, re-arm background timer while any answer is live (F-ASK-05).
pub fn on_spinner(state: &mut State, fx: &mut Fx) {
    state.spinner = (state.spinner + 1) % SPINNER_FRAMES;
    if any_live(state) {
        fx.push(Effect::SetTimer { id: TimerId::Spinner, after_ms: SPINNER_MS, background: true });
    } else {
        state.ask.spinner_running = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comments::SelectionInfo;
    use crate::comments::testutil::{answer, comment, state_with};
    use crate::mcp::McpEndpoint;

    fn running_state() -> State {
        let mut st = state_with(Vec::new());
        st.mcp.running = true;
        st.mcp.endpoint = Some(McpEndpoint { url: String::new(), token: "t".into(), port: 1 });
        st
    }

    fn human(id: &str, seq: u64) -> Comment {
        let mut c = comment(id, seq, 3, "why?");
        c.text = "let x = 1;".into();
        c.context = vec!["a".into(), "let x = 1;".into(), "b".into()];
        c
    }

    fn note_comment() -> Comment {
        let mut c = comment("q2", 2, 9, "check this");
        c.origin = Origin::Agent;
        c.number = Some(4);
        c
    }

    #[test]
    fn f_mcpsrv_06_build_question_turn1_context() {
        let q = build_question(&human("q1", 1), 1);
        assert_eq!(
            q.question,
            "why?\n\nFile: a.rs\nSide: new (after the change)\nLines: 3\n\nCode at cursor line:\n```\nlet x = 1;\n```\n\nSurrounding context:\n```\na\nlet x = 1;\nb\n```"
        );
        assert!(!q.follow_up && q.previous.is_empty() && q.turn == 1 && q.thread_id == "q1");
    }

    #[test]
    fn f_mcpsrv_06_build_question_selection_and_old_side_no_context() {
        let mut c = human("q1", 1);
        c.side = PaneSide::Old;
        c.line = None;
        c.context.clear();
        c.text = "l1\nl2".into();
        c.selection =
            Some(SelectionInfo { start_line: Some(4), end_line: Some(5), start_col: 1, end_col: 2 });
        assert_eq!(
            build_question(&c, 1).question,
            "why?\n\nFile: a.rs\nSide: old (before the change)\nLines: 4-5\n\nSelected code:\n```\nl1\nl2\n```"
        );
        c.selection =
            Some(SelectionInfo { start_line: Some(4), end_line: Some(4), start_col: 1, end_col: 2 });
        assert!(build_question(&c, 1).question.contains("Lines: 4\n"));
        c.selection = None;
        assert!(!build_question(&c, 1).question.contains("Lines:"));
    }

    #[test]
    fn f_mcpsrv_06_build_question_follow_up_previous() {
        let mut c = human("q1", 1);
        c.turns[0].answer = Some(answer(AnswerStatus::Done, "b2"));
        c.turns[0].prior = vec![answer(AnswerStatus::Done, "b1")];
        c.turns.push(Turn { message: "and?".into(), answer: None, prior: vec![] });
        let q = build_question(&c, 2);
        assert!(q.follow_up);
        assert_eq!(q.question, "Follow-up to your earlier answer (thread q1, turn 2): and?");
        assert_eq!(q.previous, vec![(1, "why?".to_string(), "b1\n\nb2".to_string())]);
    }

    #[test]
    fn f_mcpsrv_06_build_question_note_reply() {
        let mut c = note_comment();
        c.turns.push(Turn { message: "why?".into(), answer: None, prior: vec![] });
        let q = build_question(&c, 2);
        assert_eq!(
            q.question,
            "Follow-up to your earlier answer (thread q2, turn 2): why?\n\nReply to your annotate note #4:\nFile: a.rs\nSide: new\nLine: 9\n\nYour note:\ncheck this"
        );
        assert_eq!(
            q.previous,
            vec![(1, "(note you added with annotate)".to_string(), "check this".to_string())]
        );
        c.number = None;
        c.line = None;
        assert!(
            build_question(&c, 2)
                .question
                .contains("Reply to your annotate note:\nFile: a.rs\nSide: new\n\nYour note:")
        );
    }

    #[test]
    fn f_ask_01_ask_comment_marks_pending_and_queues() {
        let mut st = running_state();
        st.comments.push(human("q1", 1));
        let mut fx = Vec::new();
        ask_comment(&mut st, "q1", &mut fx);
        assert_eq!(st.mcp.queue.len(), 1);
        assert_eq!(st.mcp.queue[0].thread_id, "q1");
        let a = comments::latest_answer(&st.comments[0]).expect("answer");
        assert_eq!(a.status, AnswerStatus::Pending);
        assert!(!askable(&st, "q1"));
        assert!(st.note.is_none());
        assert!(fx.contains(&Effect::SetTimer { id: TimerId::Spinner, after_ms: 80, background: true }));
        assert!(st.ask.spinner_running);
    }

    #[test]
    fn f_ask_02_ask_focused_notes() {
        let mut st = running_state();
        st.comments.push(human("q1", 1));
        let mut fx = Vec::new();
        ask_focused(&mut st, "q1", &mut fx);
        assert_eq!(st.note.as_deref(), Some("question queued"));
        assert_eq!(st.mcp.queue.len(), 1);
        ask_focused(&mut st, "q1", &mut fx);
        assert_eq!(st.note.as_deref(), Some("still waiting for the agent"));
        // MCP off
        let mut st = state_with(Vec::new());
        st.comments.push(human("q1", 1));
        ask_focused(&mut st, "q1", &mut fx);
        assert_eq!(st.note.as_deref(), Some("MCP is off (M to start)"));
        // follow-ups: can't retry
        st.comments[0].turns.push(Turn {
            message: "x".into(),
            answer: Some(answer(AnswerStatus::Cancelled, "")),
            prior: vec![],
        });
        st.mcp.running = true;
        ask_focused(&mut st, "q1", &mut fx);
        assert_eq!(st.note.as_deref(), Some("can't retry a follow-up yet"));
        // note that cannot take a reply
        let mut n = note_comment();
        n.turns[0].answer = Some(answer(AnswerStatus::Pending, ""));
        st.comments.push(n);
        ask_focused(&mut st, "q2", &mut fx);
        assert_eq!(st.note.as_deref(), Some("still waiting for the agent"));
        st.comments[1].turns[0].answer = None;
        st.comments[1].turns.push(Turn { message: "r".into(), answer: None, prior: vec![] });
        ask_focused(&mut st, "q2", &mut fx);
        assert_eq!(st.note.as_deref(), Some("can't reply to this note yet"));
    }

    #[test]
    fn f_ask_02_askable_rules() {
        let mut st = running_state();
        let mut a = human("q1", 1);
        a.turns[0].answer = Some(answer(AnswerStatus::Cancelled, "MCP stopped"));
        let mut b = human("q3", 3);
        b.turns[0].answer = Some(answer(AnswerStatus::Done, "x"));
        st.comments = vec![a, note_comment(), b];
        assert!(askable(&st, "q1"));
        assert!(!askable(&st, "q2"));
        assert!(!askable(&st, "q3"));
        assert!(!askable(&st, "nope"));
    }

    #[test]
    fn f_ask_03_follow_up() {
        let mut st = running_state();
        let mut c = human("q1", 1);
        c.turns[0].answer = Some(answer(AnswerStatus::Done, "because"));
        st.comments.push(c);
        let mut fx = Vec::new();
        assert!(follow_up(&mut st, "q1", "and?", &mut fx));
        assert_eq!(st.note.as_deref(), Some("follow-up queued"));
        assert_eq!(st.comments[0].turns.len(), 2);
        assert_eq!(st.comments[0].turns[1].answer.as_ref().map(|a| a.status), Some(AnswerStatus::Pending));
        let q = &st.mcp.queue[0];
        assert_eq!((q.turn, q.follow_up), (2, true));
        assert_eq!(q.previous, vec![(1, "why?".to_string(), "because".to_string())]);
        assert_eq!(st.thread.scrolls.get("q1"), Some(&ThreadScroll { off: 0, follow: true }));
        // live thread refuses
        assert!(!follow_up(&mut st, "q1", "again", &mut fx));
        assert_eq!(st.note.as_deref(), Some("can't follow up yet"));
        // MCP off
        st.mcp.running = false;
        assert!(!follow_up(&mut st, "q1", "again", &mut fx));
        assert_eq!(st.note.as_deref(), Some("MCP is off (M to start)"));
    }

    #[test]
    fn f_ask_03_follow_up_turn_added_by_editor_via_ask_comment() {
        let mut st = running_state();
        let mut c = human("q1", 1);
        c.turns[0].answer = Some(answer(AnswerStatus::Done, "because"));
        c.turns.push(Turn { message: "and?".into(), answer: None, prior: vec![] });
        st.comments.push(c);
        let mut fx = Vec::new();
        ask_comment(&mut st, "q1", &mut fx);
        assert_eq!(st.mcp.queue[0].turn, 2);
    }

    #[test]
    fn f_ask_04_ask_all() {
        let mut st = running_state();
        st.comments = vec![human("q1", 1), human("q2", 2)];
        let mut fx = Vec::new();
        ask_all(&mut st, &mut fx);
        // ids_in_file needs anchored rows: none in this state -> nothing to ask
        assert_eq!(st.note.as_deref(), Some("nothing to ask"));
    }

    #[test]
    fn f_ask_09_delivery_and_answers() {
        let mut st = running_state();
        st.comments.push(human("q1", 1));
        let mut fx = Vec::new();
        ask_comment(&mut st, "q1", &mut fx);
        let ev = |e| McpOutput { effects: vec![], events: vec![e] };
        apply_output(
            &mut st,
            ev(HubEvent::Delivered { thread_id: "q1".into(), turn: 1, client_name: "bot".into() }),
            &mut fx,
        );
        let a = comments::latest_answer(&st.comments[0]).expect("a");
        assert_eq!((a.status, a.agent.as_deref()), (AnswerStatus::Streaming, Some("bot")));
        apply_output(
            &mut st,
            ev(HubEvent::Answer { thread_id: "q1".into(), turn: 1, text: "one".into() }),
            &mut fx,
        );
        let a = comments::latest_answer(&st.comments[0]).expect("a");
        assert_eq!((a.status, a.text.as_str(), a.agent.as_deref()), (AnswerStatus::Done, "one", Some("bot")));
        // second answer on the done turn: first kept
        apply_output(
            &mut st,
            ev(HubEvent::Answer { thread_id: "q1".into(), turn: 1, text: "two".into() }),
            &mut fx,
        );
        let t = &st.comments[0].turns[0];
        assert_eq!(t.prior.len(), 1);
        assert_eq!(t.prior[0].text, "one");
        assert_eq!(t.answer.as_ref().map(|a| a.text.as_str()), Some("two"));
        // unknown thread / turn: ignored
        apply_output(
            &mut st,
            ev(HubEvent::Answer { thread_id: "zz".into(), turn: 1, text: "x".into() }),
            &mut fx,
        );
        apply_output(
            &mut st,
            ev(HubEvent::Answer { thread_id: "q1".into(), turn: 5, text: "x".into() }),
            &mut fx,
        );
        assert_eq!(st.comments[0].turns[0].prior.len(), 1);
    }

    #[test]
    fn f_ask_09_unnamed_client_gives_no_agent() {
        let mut st = running_state();
        st.comments.push(human("q1", 1));
        let mut fx = Vec::new();
        ask_comment(&mut st, "q1", &mut fx);
        let out = McpOutput {
            effects: vec![],
            events: vec![HubEvent::Delivered {
                thread_id: "q1".into(),
                turn: 1,
                client_name: "unknown".into(),
            }],
        };
        apply_output(&mut st, out, &mut fx);
        let a = comments::latest_answer(&st.comments[0]).expect("a");
        assert_eq!((a.status, a.agent.clone()), (AnswerStatus::Streaming, None));
    }

    #[test]
    fn f_comment_10_annotate_creates_agent_note() {
        let mut st = running_state();
        let mut fx = Vec::new();
        let out = McpOutput {
            effects: vec![],
            events: vec![HubEvent::Annotate {
                file: "b.rs".into(),
                line: 7,
                text: "note".into(),
                side: PaneSide::Old,
                number: Some(2),
            }],
        };
        apply_output(&mut st, out, &mut fx);
        assert_eq!(st.comments.len(), 1);
        let c = &st.comments[0];
        assert_eq!(
            (c.id.as_str(), c.file.as_str(), c.line, c.side, c.number),
            ("q1", "b.rs", Some(7), PaneSide::Old, Some(2))
        );
        assert_eq!(c.origin, Origin::Agent);
        assert_eq!(c.turns[0].message, "note");
        assert_eq!(c.text, "");
    }

    #[test]
    fn f_mcpui_03_cancel_live() {
        let mut st = running_state();
        let mut c = human("q1", 1);
        c.turns[0].answer = Some(answer(AnswerStatus::Streaming, ""));
        let mut d = human("q2", 2);
        d.turns[0].answer = Some(answer(AnswerStatus::Done, "ok"));
        st.comments = vec![c, d];
        cancel_live(&mut st);
        let a = comments::latest_answer(&st.comments[0]).expect("a");
        assert_eq!((a.status, a.text.as_str()), (AnswerStatus::Cancelled, "MCP stopped"));
        assert_eq!(comments::latest_answer(&st.comments[1]).map(|a| a.status), Some(AnswerStatus::Done));
    }

    #[test]
    fn f_ask_05_spinner_rearms_while_live() {
        let mut st = running_state();
        let mut c = human("q1", 1);
        c.turns[0].answer = Some(answer(AnswerStatus::Streaming, ""));
        st.comments.push(c);
        st.ask.spinner_running = true;
        let mut fx = Vec::new();
        on_spinner(&mut st, &mut fx);
        assert_eq!(st.spinner, 1);
        assert_eq!(fx, vec![Effect::SetTimer { id: TimerId::Spinner, after_ms: 80, background: true }]);
        cancel_live(&mut st);
        let mut fx = Vec::new();
        on_spinner(&mut st, &mut fx);
        assert!(fx.is_empty() && !st.ask.spinner_running);
        st.spinner = 9;
        on_spinner(&mut st, &mut fx);
        assert_eq!(st.spinner, 0);
    }

    #[test]
    fn f_ask_01_end_to_end_with_mcp_state() {
        // ask -> parked poll wakes -> answer tool -> events fold into the comment
        use crate::mcp::{ConnId, HttpRequest};
        let mut st = running_state();
        st.comments.push(human("q1", 1));
        let req = |conn: u64, body: &str| HttpRequest {
            conn: ConnId(conn),
            method: "POST".into(),
            path: "/mcp".into(),
            headers: vec![("host".into(), "localhost".into()), ("authorization".into(), "Bearer t".into())],
            body: body.as_bytes().to_vec(),
            body_too_large: false,
            remote_port: 1,
            entropy: [3; 16],
        };
        let mut fx = Vec::new();
        let out = st.mcp.handle_http(
            req(1, "{\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"next_question\"}}"),
            0,
        );
        apply_output(&mut st, out, &mut fx);
        ask_comment(&mut st, "q1", &mut fx);
        assert!(fx.iter().any(|e| matches!(e, Effect::HttpReply { .. })));
        assert_eq!(comments::latest_answer(&st.comments[0]).map(|a| a.status), Some(AnswerStatus::Streaming));
        let out = st.mcp.handle_http(
            req(2, "{\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"answer\",\"arguments\":{\"thread_id\":\"q1\",\"text\":\"yes\"}}}"),
            0,
        );
        apply_output(&mut st, out, &mut fx);
        let a = comments::latest_answer(&st.comments[0]).expect("a");
        assert_eq!((a.status, a.text.as_str()), (AnswerStatus::Done, "yes"));
    }
}
