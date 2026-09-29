//! Ask flow: sending comments to the agent, thread turns, applying MCP hub events to comments.
//!
//! Spec: F-ASK-01 (enqueue, ids), F-ASK-02 (`a` ask focused), F-ASK-03 (follow-up), F-ASK-04 (`A` ask all),
//! F-ASK-05 (status/spinner), F-ASK-09 (answer arrival, second answer on done turn), F-COMMENT-10 (agent
//! notes), F-MCPSRV-06 (question text incl. context), F-RELOAD-02 (`files_changed` reload trigger).
//! Oracle: `src/ask/{controller,prompt}.ts`, `src/useAsk.ts`, `src/mcp/bridge.ts`.
//! Owner: component `agent` (E). Must not: format thread boxes (thread_layout) or open sockets.

use crate::comments::Comment;
use crate::effect::Fx;
use crate::mcp::{McpOutput, OutQuestion};
use crate::state::State;

/// Ask bookkeeping (add fields here), e.g. spinner running flag.
#[derive(Debug, Clone, Default)]
pub struct AskState {
    pub spinner_running: bool,
}

/// `<q>` text sent to the agent: message plus code context (F-MCPSRV-06).
pub fn build_question(_comment: &Comment, _turn: u32) -> OutQuestion {
    todo!("F-MCPSRV-06, prompt.ts")
}

/// Send a comment (new thread or follow-up turn) to the agent queue: mark pending, `state.mcp.enqueue`,
/// merge effects (`apply_output`), arm spinner.
pub fn ask_comment(_state: &mut State, _id: &str, _fx: &mut Fx) {
    todo!("F-ASK-01/02/03")
}

/// `A`: ask every askable comment of all files (F-ASK-04).
pub fn ask_all(_state: &mut State, _fx: &mut Fx) {
    todo!("F-ASK-04")
}

/// Whether a comment may be asked now (F-ASK-02 preconditions).
pub fn askable(_state: &State, _id: &str) -> bool {
    todo!("F-ASK-02")
}

/// Fold an `McpOutput` into state and effects: hub events -> answers/annotations/clients/`files_changed`
/// (-> `reload::on_files_changed`), effects appended to `fx` in order.
pub fn apply_output(_state: &mut State, _out: McpOutput, _fx: &mut Fx) {
    todo!("F-ASK-09, F-COMMENT-10, F-RELOAD-02")
}

/// `TimerId::Spinner`: advance frame, re-arm background timer while any answer is live (F-ASK-05).
pub fn on_spinner(_state: &mut State, _fx: &mut Fx) {
    todo!("F-ASK-05")
}
