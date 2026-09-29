//! MCP modal (`M`), server start/stop flow, integrations flow, wiring of MCP events into state.
//!
//! Spec: F-MCPUI-01 (modal content, power row, integration rows, status, errors), F-MCPUI-02 (keys),
//! F-MCPUI-03 (start/stop effects, port busy), F-MCPUI-04 (autostart on `Event::Started`), F-INTEG-02..06
//! (check/register/unregister flow with `Integrations` trait, copy `c`, watch prompt `w`, masking, notes),
//! F-MCPSRV-11 (client list display data). Oracle: `src/components/McpModal.tsx`, `src/mcp/bridge.ts`,
//! `mcp*` code in `src/app.tsx`. Owner: component `shell` (G). Never names an agent: only the trait.
//! Must not render (`view::modals` draws from `state.overlay`, `state.mcp`, `state.integration_state`).

use crate::ask;
use crate::comments::AnswerStatus;
use crate::effect::{Effect, Fx};
use crate::errors::{IoReason, fail_msg};
use crate::event::{ReqId, TimerId};
use crate::integration::{CommandError, CommandResult, CommandSpec, RegStatus};
use crate::keys::{Key, KeyEvent};
use crate::mcp::{self, ConnId, HttpRequest, McpEndpoint};
use crate::state::{CommandPurpose, McpModal, Overlay, Pending, State};

const START_FIRST: &str = "start MCP first";
const COPY_ONLY: &str = "copy-paste only: c copies the snippet";

/// `M` in cursor context. True when consumed.
pub fn on_normal_key(state: &mut State, key: KeyEvent, _fx: &mut Fx) -> bool {
    if key.mods.ctrl || key.key != Key::Char('M') {
        return false;
    }
    state.overlay = Overlay::Mcp(McpModal::default());
    true
}

/// Key while `Overlay::Mcp`.
pub fn on_key(state: &mut State, key: KeyEvent, fx: &mut Fx) {
    if key.mods.ctrl {
        return;
    }
    let Overlay::Mcp(mut m) = std::mem::replace(&mut state.overlay, Overlay::None) else {
        return;
    };
    let keep_open = handle_key(state, &mut m, key, fx);
    if keep_open {
        state.overlay = Overlay::Mcp(m);
    }
}

/// Returns false when the modal closes.
fn handle_key(state: &mut State, m: &mut McpModal, key: KeyEvent, fx: &mut Fx) -> bool {
    if let Some((i, register)) = m.confirm {
        match key.key {
            Key::Char('y') | Key::Enter => {
                m.confirm = None;
                m.note = None;
                if register {
                    start_register(state, i, fx);
                } else {
                    start_unregister(state, i, fx);
                }
            }
            Key::Char('n') | Key::Esc => m.confirm = None,
            _ => {}
        }
        return true;
    }
    let count = state.integrations.len();
    let it = m.row.checked_sub(1).filter(|i| *i < count);
    match key.key {
        Key::Esc | Key::Char('q') | Key::Char('M') => return false,
        Key::Char('j') | Key::Down => pick(m, 1, count),
        Key::Char('k') | Key::Up => pick(m, -1, count),
        Key::Enter | Key::Char(' ') if m.row == 0 => {
            m.note = None;
            if state.mcp.running {
                stop_server(state, fx);
            } else {
                start_server(state, fx);
            }
        }
        Key::Enter | Key::Char('d') if it.is_some() => {
            if let Some(i) = it {
                on_enter_or_d(state, m, i, key.key == Key::Char('d'));
            }
        }
        Key::Char(c @ ('c' | 'w')) if it.is_some() => {
            if let Some(i) = it {
                copy_text(state, m, i, c == 'c', fx);
            }
        }
        Key::Char('R') => {
            if state.mcp.running {
                check_all(state, fx, None);
            } else {
                m.note = Some(START_FIRST.to_string());
            }
        }
        _ => {}
    }
    true
}

fn pick(m: &mut McpModal, d: isize, count: usize) {
    m.row = m.row.saturating_add_signed(d).min(count);
    m.preview = None;
    m.note = None;
}

fn on_enter_or_d(state: &State, m: &mut McpModal, i: usize, is_d: bool) {
    m.preview = None;
    let can = state.integrations[i].can_register();
    let status = state.integration_state.get(i).map(|s| s.status).unwrap_or_default();
    if !can {
        m.note = Some(COPY_ONLY.to_string());
    } else if is_d {
        if status == RegStatus::NotRegistered {
            m.note = Some("not registered".to_string());
        } else {
            m.note = None;
            m.confirm = Some((i, false));
        }
    } else if !state.mcp.running {
        m.note = Some(START_FIRST.to_string());
    } else if status == RegStatus::Registered {
        m.note = Some("already registered (d to remove)".to_string());
    } else {
        m.note = None;
        m.confirm = Some((i, true));
    }
}

fn copy_text(state: &State, m: &mut McpModal, i: usize, register: bool, fx: &mut Fx) {
    let Some(ep) = state.mcp.endpoint.clone().filter(|_| state.mcp.running) else {
        m.preview = None;
        m.note = Some(START_FIRST.to_string());
        return;
    };
    let a = &state.integrations[i];
    let (text, what) = if register {
        (a.register_text(&ep), "register command (copied)")
    } else {
        (a.watch_prompt(&ep), "watch prompt (copied)")
    };
    m.note = None;
    m.preview = Some((what.to_string(), mask(&text, &ep.token)));
    fx.push(Effect::Clipboard(text));
}

fn mask(text: &str, token: &str) -> String {
    if token.is_empty() { text.to_string() } else { text.replace(token, "***") }
}

fn token_of(state: &State) -> String {
    state.mcp.endpoint.as_ref().map(|e| e.token.clone()).unwrap_or_else(|| state.last_token.clone())
}

/// `Event::Started`: autostart when configured (F-MCPUI-04).
pub fn on_started(state: &mut State, fx: &mut Fx) {
    if !state.settings.mcp_autostart {
        return;
    }
    start_server(state, fx);
    if let Some(req) = state.mcp.starting {
        state.autostart_req = Some(req);
    } else if !state.mcp.running {
        if let Some(e) = state.mcp.start_error.clone() {
            state.note = Some(format!("mcp autostart failed: {e}"));
        }
    }
}

/// Emit `Effect::McpStart` (validates `env.mcp_port_raw` via `mcp::parse_port`).
pub fn start_server(state: &mut State, fx: &mut Fx) {
    if state.mcp.running || state.mcp.starting.is_some() {
        return;
    }
    state.mcp.start_error = None;
    let port = match mcp::parse_port(state.env.mcp_port_raw.as_deref()) {
        Ok(p) => p,
        Err(msg) => {
            state.mcp.start_error = Some(msg);
            return;
        }
    };
    let req = state.alloc_req();
    state.pending.insert(req, Pending::McpStart);
    state.mcp.starting = Some(req);
    fx.push(Effect::McpStart { req, port, state_dir: state.env.state_dir.clone() });
}

/// Stop: waiting polls answered `closed`, live answers cancelled, then `McpStop` (F-MCPUI-03).
pub fn stop_server(state: &mut State, fx: &mut Fx) {
    if !state.mcp.running && state.mcp.endpoint.is_none() {
        return;
    }
    let out = state.mcp.stop();
    ask::apply_output(state, out, fx);
    state.mcp.running = false;
    state.mcp.endpoint = None;
    state.mcp.clients.clear();
    state.mcp.queue.clear();
    cancel_live(state);
    if let Overlay::Editor(e) = &mut state.overlay {
        e.ask_mode = false;
    }
    let req = state.alloc_req();
    state.pending.insert(req, Pending::McpStop);
    fx.push(Effect::McpStop { req });
}

fn cancel_live(state: &mut State) {
    for c in &mut state.comments {
        if let Some(a) = c.turns.last_mut().and_then(|t| t.answer.as_mut()) {
            if matches!(a.status, AnswerStatus::Pending | AnswerStatus::Streaming) {
                a.status = AnswerStatus::Cancelled;
                a.text = "MCP stopped".to_string();
            }
        }
    }
}

pub fn on_mcp_started(state: &mut State, req: ReqId, result: Result<McpEndpoint, String>, fx: &mut Fx) {
    if state.pending.remove(&req).is_none() || state.mcp.starting != Some(req) {
        return;
    }
    state.mcp.starting = None;
    let auto = state.autostart_req.take() == Some(req);
    match result {
        Ok(ep) => {
            state.last_token = ep.token.clone();
            state.mcp.running = true;
            state.mcp.endpoint = Some(ep);
            state.mcp.start_error = None;
            state.mcp.delivered = 0;
            check_all(state, fx, None);
        }
        Err(e) => {
            if auto {
                state.note = Some(format!("mcp autostart failed: {e}"));
            }
            state.mcp.start_error = Some(e);
        }
    }
}

pub fn on_mcp_stopped(state: &mut State, req: ReqId, _fx: &mut Fx) {
    state.pending.remove(&req);
}

fn run(state: &mut State, fx: &mut Fx, integration: usize, purpose: CommandPurpose, cmd: CommandSpec) {
    let req = state.alloc_req();
    state.pending.insert(req, Pending::Command { integration, purpose });
    fx.push(Effect::RunCommand { req, cmd });
}

/// Registration check of every registrable integration (F-INTEG-02). `keep` keeps that row's note.
fn check_all(state: &mut State, fx: &mut Fx, keep: Option<usize>) {
    let Some(ep) = state.mcp.endpoint.clone() else {
        return;
    };
    for i in 0..state.integrations.len() {
        let Some(cmd) = state.integrations[i].check_command(&ep) else {
            continue;
        };
        if let Some(s) = state.integration_state.get_mut(i) {
            s.keep_note = keep == Some(i);
        }
        run(state, fx, i, CommandPurpose::Check, cmd);
    }
}

fn start_register(state: &mut State, i: usize, fx: &mut Fx) {
    let Some(ep) = state.mcp.endpoint.clone() else {
        return;
    };
    let cmds = state.integrations[i].register_commands(&ep);
    let Some(first) = cmds.into_iter().next() else {
        return;
    };
    set_busy(state, i, true);
    run(state, fx, i, CommandPurpose::Register(0), first);
}

fn start_unregister(state: &mut State, i: usize, fx: &mut Fx) {
    let Some(cmd) = state.integrations[i].unregister_command() else {
        return;
    };
    set_busy(state, i, true);
    run(state, fx, i, CommandPurpose::Unregister, cmd);
}

fn set_busy(state: &mut State, i: usize, busy: bool) {
    if let Some(s) = state.integration_state.get_mut(i) {
        s.busy = busy;
        if busy {
            s.message = None;
        }
    }
}

/// `None` = the command succeeded (exit 0); else the F-INTEG-03 failure text.
fn failure_text(label: &str, what: &str, result: &CommandResult) -> Option<String> {
    match result {
        Err(CommandError::NotFound) => Some(format!("{label} CLI not found")),
        Err(CommandError::Timeout) => Some(format!("{label} {what} failed: timed out")),
        Err(CommandError::Other(m)) => Some(format!("{label} {what} failed: {m}")),
        Ok(o) if o.code != 0 => {
            let s = if o.stderr.is_empty() { &o.stdout } else { &o.stderr };
            let out = s.trim().split('\n').take(3).collect::<Vec<_>>().join(" ");
            let tail = if out.is_empty() { String::new() } else { format!(": {out}") };
            Some(format!("{label} {what} failed (exit {}){tail}", o.code))
        }
        Ok(_) => None,
    }
}

/// `Event::CommandDone` for integration checks/registrations (F-INTEG-02..04).
pub fn on_command_done(state: &mut State, req: ReqId, result: CommandResult, fx: &mut Fx) {
    let Some(Pending::Command { integration: i, purpose }) = state.pending.remove(&req) else {
        return;
    };
    if i >= state.integrations.len() || i >= state.integration_state.len() {
        return;
    }
    let agent = state.integrations[i].clone();
    let token = token_of(state);
    match purpose {
        CommandPurpose::Check => {
            let keep = std::mem::take(&mut state.integration_state[i].keep_note);
            let Some(ep) = state.mcp.endpoint.clone() else {
                return;
            };
            let st = &mut state.integration_state[i];
            st.status = agent.parse_check(&ep, &result);
            if !keep {
                st.message = None;
            }
        }
        CommandPurpose::Register(step) => {
            let Some(ep) = state.mcp.endpoint.clone() else {
                set_busy(state, i, false);
                return;
            };
            let cmds = agent.register_commands(&ep);
            if let Some(next) = cmds.get(step + 1) {
                run(state, fx, i, CommandPurpose::Register(step + 1), next.clone());
                return;
            }
            let msg =
                failure_text(agent.label(), "register", &result).unwrap_or_else(|| agent.register_hint(&ep));
            finish(state, i, msg, &token, fx);
        }
        CommandPurpose::Unregister => {
            let msg = failure_text(agent.label(), "unregister", &result)
                .unwrap_or_else(|| format!("Removed xplain from {}", agent.label()));
            finish(state, i, msg, &token, fx);
        }
    }
}

fn finish(state: &mut State, i: usize, msg: String, token: &str, fx: &mut Fx) {
    let st = &mut state.integration_state[i];
    st.busy = false;
    st.message = Some(mask(&msg, token));
    check_all(state, fx, Some(i));
}

/// `Event::McpHttp`: `state.mcp.handle_http`, then `ask::apply_output`.
pub fn on_http(state: &mut State, req: HttpRequest, fx: &mut Fx) {
    let out = state.mcp.handle_http(req, state.clock.unix_ms);
    ask::apply_output(state, out, fx);
}

pub fn on_conn_closed(state: &mut State, conn: ConnId, fx: &mut Fx) {
    let out = state.mcp.conn_closed(conn);
    ask::apply_output(state, out, fx);
}

/// `Event::Timer(PollTimeout)` / `Spinner` routing for MCP-owned timers.
pub fn on_timer(state: &mut State, id: TimerId, fx: &mut Fx) {
    match id {
        TimerId::PollTimeout(conn) => {
            let out = state.mcp.poll_timeout(conn);
            ask::apply_output(state, out, fx);
        }
        TimerId::Spinner => ask::on_spinner(state, fx),
    }
}

/// Textual reason helper for start failures shown in the modal.
pub fn start_error_text(port: u16, reason: IoReason) -> String {
    fail_msg(&format!("cannot listen on 127.0.0.1:{port}"), reason)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::testutil::{ep, fake_state, k, ok, running_state};

    fn open(s: &mut State) {
        let mut fx = Vec::new();
        assert!(on_normal_key(s, k('M'), &mut fx));
    }

    fn press(s: &mut State, key: KeyEvent) -> Fx {
        let mut fx = Vec::new();
        on_key(s, key, &mut fx);
        fx
    }

    fn modal(s: &State) -> &McpModal {
        match &s.overlay {
            Overlay::Mcp(m) => m,
            o => panic!("not mcp: {o:?}"),
        }
    }

    fn done_last(s: &mut State, res: CommandResult, fx: &mut Fx) {
        let req = ReqId(s.next_req);
        on_command_done(s, req, res, fx);
    }

    fn req_of(fx: &Fx) -> ReqId {
        match fx.last() {
            Some(Effect::RunCommand { req, .. }) => *req,
            e => panic!("no run: {e:?}"),
        }
    }

    #[test]
    fn f_mcpui_01_opens_on_power_row() {
        let mut s = fake_state();
        open(&mut s);
        assert_eq!(*modal(&s), McpModal::default());
    }

    #[test]
    fn f_mcpui_02_rows_clamped_and_notes_cleared() {
        let mut s = fake_state();
        open(&mut s);
        for _ in 0..5 {
            press(&mut s, k('j'));
        }
        assert_eq!(modal(&s).row, 2);
        press(&mut s, KeyEvent::plain(Key::Up));
        assert_eq!(modal(&s).row, 1);
        press(&mut s, k('k'));
        press(&mut s, k('k'));
        assert_eq!(modal(&s).row, 0);
        press(&mut s, k('j'));
        press(&mut s, KeyEvent::plain(Key::Enter));
        assert_eq!(modal(&s).note.as_deref(), Some("start MCP first"));
        press(&mut s, k('j'));
        assert_eq!(modal(&s).note, None);
    }

    #[test]
    fn f_mcpui_02_close_keys() {
        for c in [KeyEvent::plain(Key::Esc), k('q'), k('M')] {
            let mut s = fake_state();
            open(&mut s);
            press(&mut s, c);
            assert_eq!(s.overlay, Overlay::None);
        }
    }

    #[test]
    fn f_mcpui_02_power_row_starts_and_stops() {
        let mut s = fake_state();
        open(&mut s);
        let fx = press(&mut s, KeyEvent::plain(Key::Enter));
        assert!(
            matches!(fx.as_slice(), [Effect::McpStart { port: 47615, state_dir, .. }] if state_dir == "/state")
        );
        assert!(s.mcp.starting.is_some());
        // second Enter while starting: nothing
        assert!(press(&mut s, k(' ')).is_empty());
    }

    #[test]
    fn f_mcpui_03_invalid_port_sets_error_no_effect() {
        let mut s = fake_state();
        s.env.mcp_port_raw = Some("70000".into());
        let mut fx = Vec::new();
        start_server(&mut s, &mut fx);
        assert!(fx.is_empty());
        assert_eq!(s.mcp.start_error.as_deref(), Some("invalid XPLAIN_MCP_PORT \"70000\" (0-65535)"));
        assert!(!s.mcp.running);
    }

    #[test]
    fn f_mcpui_03_started_runs_checks_and_clears_error() {
        let mut s = fake_state();
        s.mcp.start_error = Some("x".into());
        let mut fx = Vec::new();
        start_server(&mut s, &mut fx);
        let Effect::McpStart { req, .. } = fx[0] else { panic!() };
        let mut fx2 = Vec::new();
        on_mcp_started(&mut s, req, Ok(ep()), &mut fx2);
        assert!(s.mcp.running);
        assert_eq!(s.mcp.start_error, None);
        assert_eq!(s.last_token, "secrettoken");
        // only the registrable integration is checked
        assert_eq!(fx2.len(), 1);
        assert!(matches!(&fx2[0], Effect::RunCommand { cmd, .. } if cmd.args == ["get"]));
    }

    #[test]
    fn f_mcpui_03_start_failure_keeps_error_row() {
        let mut s = fake_state();
        let mut fx = Vec::new();
        start_server(&mut s, &mut fx);
        let Effect::McpStart { req, .. } = fx[0] else { panic!() };
        on_mcp_started(&mut s, req, Err(mcp::port_busy_message(47615)), &mut Vec::new());
        assert!(!s.mcp.running);
        assert!(
            s.mcp.start_error.as_deref().is_some_and(|e| e.starts_with("MCP port 47615 is already in use"))
        );
        assert_eq!(s.note, None);
    }

    #[test]
    fn stale_start_result_ignored() {
        let mut s = fake_state();
        on_mcp_started(&mut s, ReqId(999), Ok(ep()), &mut Vec::new());
        assert!(!s.mcp.running);
    }

    #[test]
    fn f_mcpui_04_autostart_off_does_nothing() {
        let mut s = fake_state();
        let mut fx = Vec::new();
        on_started(&mut s, &mut fx);
        assert!(fx.is_empty());
    }

    #[test]
    fn f_mcpui_04_autostart_starts_and_failure_notes() {
        let mut s = fake_state();
        s.settings.mcp_autostart = true;
        let mut fx = Vec::new();
        on_started(&mut s, &mut fx);
        let Effect::McpStart { req, .. } = fx[0] else { panic!() };
        on_mcp_started(&mut s, req, Err("boom".into()), &mut Vec::new());
        assert_eq!(s.note.as_deref(), Some("mcp autostart failed: boom"));
    }

    #[test]
    fn f_mcpui_04_autostart_invalid_port_notes() {
        let mut s = fake_state();
        s.settings.mcp_autostart = true;
        s.env.mcp_port_raw = Some("abc".into());
        let mut fx = Vec::new();
        on_started(&mut s, &mut fx);
        assert!(fx.is_empty());
        assert_eq!(
            s.note.as_deref(),
            Some("mcp autostart failed: invalid XPLAIN_MCP_PORT \"abc\" (0-65535)")
        );
    }

    #[test]
    fn f_mcpui_02_integration_enter_rules() {
        let mut s = fake_state();
        open(&mut s);
        press(&mut s, k('j'));
        press(&mut s, KeyEvent::plain(Key::Enter));
        assert_eq!(modal(&s).note.as_deref(), Some("start MCP first"));
        // copy-paste only row
        press(&mut s, k('j'));
        press(&mut s, KeyEvent::plain(Key::Enter));
        assert_eq!(modal(&s).note.as_deref(), Some("copy-paste only: c copies the snippet"));
        press(&mut s, k('d'));
        assert_eq!(modal(&s).note.as_deref(), Some("copy-paste only: c copies the snippet"));
    }

    #[test]
    fn f_mcpui_02_register_confirm_flow() {
        let mut s = running_state();
        open(&mut s);
        press(&mut s, k('j'));
        press(&mut s, KeyEvent::plain(Key::Enter));
        assert_eq!(modal(&s).confirm, Some((0, true)));
        // other keys ignored, n cancels
        assert!(press(&mut s, k('x')).is_empty());
        assert_eq!(modal(&s).confirm, Some((0, true)));
        press(&mut s, k('n'));
        assert_eq!(modal(&s).confirm, None);
        press(&mut s, KeyEvent::plain(Key::Enter));
        let fx = press(&mut s, k('y'));
        assert_eq!(modal(&s).confirm, None);
        assert!(s.integration_state[0].busy);
        let Effect::RunCommand { cmd, .. } = &fx[0] else { panic!() };
        assert_eq!(cmd.args, ["remove"]);
    }

    #[test]
    fn f_mcpui_02_already_registered_and_not_registered() {
        let mut s = running_state();
        s.integration_state[0].status = RegStatus::Registered;
        open(&mut s);
        press(&mut s, k('j'));
        press(&mut s, KeyEvent::plain(Key::Enter));
        assert_eq!(modal(&s).note.as_deref(), Some("already registered (d to remove)"));
        press(&mut s, k('d'));
        assert_eq!(modal(&s).confirm, Some((0, false)));
        press(&mut s, KeyEvent::plain(Key::Esc));
        s.integration_state[0].status = RegStatus::NotRegistered;
        press(&mut s, k('d'));
        assert_eq!(modal(&s).note.as_deref(), Some("not registered"));
        s.integration_state[0].status = RegStatus::Stale;
        press(&mut s, KeyEvent::plain(Key::Enter));
        assert_eq!(modal(&s).confirm, Some((0, true)));
    }

    #[test]
    fn f_integ_03_register_success_sequence() {
        let mut s = running_state();
        let mut fx = Vec::new();
        start_register(&mut s, 0, &mut fx);
        let r1 = req_of(&fx);
        let mut fx = Vec::new();
        // remove result ignored (fails)
        on_command_done(&mut s, r1, ok(1, "", "nope"), &mut fx);
        assert!(s.integration_state[0].busy);
        let Effect::RunCommand { cmd, .. } = &fx[0] else { panic!() };
        assert_eq!(cmd.args, ["add"]);
        let r2 = req_of(&fx);
        let mut fx = Vec::new();
        on_command_done(&mut s, r2, ok(0, "", ""), &mut fx);
        let st = &s.integration_state[0];
        assert!(!st.busy);
        assert_eq!(
            st.message.as_deref(),
            Some("Registered ***; restart the agent session, then paste the watch prompt")
        );
        // re-check keeps the note
        let r3 = req_of(&fx);
        assert!(matches!(&fx[0], Effect::RunCommand { cmd, .. } if cmd.args == ["get"]));
        on_command_done(&mut s, r3, ok(0, "", ""), &mut Vec::new());
        let st = &s.integration_state[0];
        assert_eq!(st.status, RegStatus::Registered);
        assert!(st.message.is_some());
        // a later plain check clears it
        let mut fx = Vec::new();
        check_all(&mut s, &mut fx, None);
        on_command_done(&mut s, req_of(&fx), ok(0, "", ""), &mut Vec::new());
        assert_eq!(s.integration_state[0].message, None);
    }

    #[test]
    fn f_integ_03_register_failures() {
        let cases: Vec<(CommandResult, &str)> = vec![
            (Err(CommandError::NotFound), "Alpha CLI not found"),
            (ok(2, "out", ""), "Alpha register failed (exit 2): out"),
            (ok(2, "out", "\n  bad\n\nx\ny\n"), "Alpha register failed (exit 2): bad  x"),
            (ok(3, "", ""), "Alpha register failed (exit 3)"),
            (ok(3, "out", "  "), "Alpha register failed (exit 3)"),
            (Err(CommandError::Other("boom".into())), "Alpha register failed: boom"),
        ];
        for (res, want) in cases {
            let mut s = running_state();
            let mut fx = Vec::new();
            start_register(&mut s, 0, &mut fx);
            let mut fx = Vec::new();
            done_last(&mut s, ok(0, "", ""), &mut fx);
            done_last(&mut s, res, &mut fx);
            assert_eq!(s.integration_state[0].message.as_deref(), Some(want));
        }
    }

    #[test]
    fn f_integ_03_token_masked_in_failure() {
        let mut s = running_state();
        let mut fx = Vec::new();
        start_register(&mut s, 0, &mut fx);
        let mut fx = Vec::new();
        done_last(&mut s, ok(0, "", ""), &mut fx);
        done_last(&mut s, ok(1, "", "bad secrettoken here"), &mut fx);
        assert_eq!(
            s.integration_state[0].message.as_deref(),
            Some("Alpha register failed (exit 1): bad *** here")
        );
    }

    #[test]
    fn f_integ_04_unregister() {
        let mut s = running_state();
        let mut fx = Vec::new();
        start_unregister(&mut s, 0, &mut fx);
        assert!(s.integration_state[0].busy);
        let mut fx = Vec::new();
        done_last(&mut s, ok(0, "", ""), &mut fx);
        assert_eq!(s.integration_state[0].message.as_deref(), Some("Removed xplain from Alpha"));
        let mut s = running_state();
        let mut fx = Vec::new();
        start_unregister(&mut s, 0, &mut fx);
        done_last(&mut s, ok(4, "", "no"), &mut Vec::new());
        assert_eq!(s.integration_state[0].message.as_deref(), Some("Alpha unregister failed (exit 4): no"));
    }

    #[test]
    fn f_integ_04_unregister_with_mcp_off_no_recheck() {
        let mut s = fake_state();
        let mut fx = Vec::new();
        start_unregister(&mut s, 0, &mut fx);
        let mut fx = Vec::new();
        done_last(&mut s, ok(0, "", ""), &mut fx);
        assert!(fx.is_empty());
        assert!(!s.integration_state[0].busy);
    }

    #[test]
    fn f_integ_02_check_statuses() {
        let mut s = running_state();
        for (res, want) in [
            (ok(0, "", ""), RegStatus::Registered),
            (ok(0, "stale", ""), RegStatus::Stale),
            (ok(1, "", ""), RegStatus::NotRegistered),
            (Err(CommandError::Timeout), RegStatus::NotRegistered),
        ] {
            let mut fx = Vec::new();
            check_all(&mut s, &mut fx, None);
            s.integration_state[0].message = Some("old".into());
            on_command_done(&mut s, req_of(&fx), res, &mut Vec::new());
            assert_eq!(s.integration_state[0].status, want);
            assert_eq!(s.integration_state[0].message, None);
        }
    }

    #[test]
    fn stale_command_ignored() {
        let mut s = running_state();
        on_command_done(&mut s, ReqId(77), ok(0, "", ""), &mut Vec::new());
        assert_eq!(s.integration_state[0].status, RegStatus::NotRegistered);
    }

    #[test]
    fn f_integ_05_copy_and_preview_masked() {
        let mut s = running_state();
        open(&mut s);
        press(&mut s, k('j'));
        let fx = press(&mut s, k('c'));
        assert_eq!(fx, vec![Effect::Clipboard("register http://127.0.0.1:47615/mcp secrettoken".into())]);
        assert_eq!(
            modal(&s).preview,
            Some(("register command (copied)".into(), "register http://127.0.0.1:47615/mcp ***".into()))
        );
        let fx = press(&mut s, k('w'));
        assert_eq!(fx, vec![Effect::Clipboard("watch secrettoken".into())]);
        assert_eq!(modal(&s).preview, Some(("watch prompt (copied)".into(), "watch ***".into())));
    }

    #[test]
    fn f_integ_05_copy_needs_running_and_power_row_ignores() {
        let mut s = fake_state();
        open(&mut s);
        assert!(press(&mut s, k('c')).is_empty());
        assert_eq!(modal(&s).note, None);
        press(&mut s, k('j'));
        assert!(press(&mut s, k('c')).is_empty());
        assert_eq!(modal(&s).note.as_deref(), Some("start MCP first"));
    }

    #[test]
    fn f_mcpui_02_refresh() {
        let mut s = fake_state();
        open(&mut s);
        press(&mut s, k('R'));
        assert_eq!(modal(&s).note.as_deref(), Some("start MCP first"));
        let mut s = running_state();
        open(&mut s);
        let fx = press(&mut s, k('R'));
        assert_eq!(fx.len(), 1);
    }

    #[test]
    fn f_mcpui_02_ctrl_keys_ignored_in_modal() {
        let mut s = fake_state();
        open(&mut s);
        press(&mut s, KeyEvent::ctrl('j'));
        assert_eq!(modal(&s).row, 0);
    }

    #[test]
    fn start_error_text_form() {
        assert_eq!(
            start_error_text(1, IoReason::PermissionDenied),
            "cannot listen on 127.0.0.1:1: permission denied"
        );
    }

    #[test]
    fn f_mcpui_03_stop_cancels_live_answers_and_orders_effects() {
        use crate::comments::{Answer, Turn};
        let mut s = running_state();
        let mut c = crate::comments::from_cursor(&mut s, "m");
        c.turns = vec![Turn {
            message: "m".into(),
            answer: Some(Answer { status: AnswerStatus::Streaming, text: String::new(), agent: None }),
            prior: vec![],
        }];
        s.comments.push(c);
        s.overlay = Overlay::Editor(crate::state::EditorState {
            kind: crate::state::EditorKind::New,
            text: String::new(),
            caret: 0,
            ask_mode: true,
            ext: Default::default(),
        });
        let mut fx = Vec::new();
        stop_server(&mut s, &mut fx);
        assert!(matches!(fx.last(), Some(Effect::McpStop { .. })));
        assert!(fx[..fx.len() - 1].iter().all(|e| !matches!(e, Effect::McpStop { .. })));
        let a = s.comments[0].turns[0].answer.as_ref().map(|a| (a.status, a.text.clone()));
        assert_eq!(a, Some((AnswerStatus::Cancelled, "MCP stopped".to_string())));
        assert!(matches!(&s.overlay, Overlay::Editor(e) if !e.ask_mode));
        assert!(!s.mcp.running && s.mcp.endpoint.is_none());
        // stopping again is a no-op
        let mut fx = Vec::new();
        stop_server(&mut s, &mut fx);
        assert!(fx.is_empty());
    }
}
